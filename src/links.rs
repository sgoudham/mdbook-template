use std::fs;
use std::path::{Path, PathBuf};

use aho_corasick::AhoCorasick;
use anyhow::Context;
use fancy_regex::{CaptureMatches, Captures, Regex};
use lazy_static::lazy_static;
use mdbook::errors::Result;

const ESCAPE_CHAR: char = '\\';

lazy_static! {
    // r"(?x)\\\{\{\#.*\}\}|\{\{\s*\#(template)\s+([a-zA-Z0-9_.\/-]+)\s*([^}]+)\}\}")
    static ref WHOLE_TEMPLATE: Regex = Regex::new(
            r"(?x)              # insignificant whitespace mode
            \\\{\{\#.*\}\}      # match escaped link
            |                   # or
            \{\{\s*             # link opening parens and whitespace
            \#(template)        # link type - template
            \s+                 # separating whitespace
            ([a-zA-Z0-9_.\/-]+) # relative path to template file 
            \s+                 # separating whitespace
            ([^}]+)             # get all template arguments
            \}\}                # link closing parens"
        )
        .unwrap();
    // https://stackoverflow.com/questions/22871602/optimizing-regex-to-fine-key-value-pairs-space-delimited
    static ref ARGS: Regex = Regex::new(r"(?<=\s|\A)([^\s=]+)=(.*?)(?=(?:\s[^\s=]+=|$))").unwrap();
}

#[derive(PartialEq, Debug, Clone)]
struct VecPair(Vec<String>, Vec<String>);

#[derive(PartialEq, Debug, Clone)]
pub(crate) struct Link<'a> {
    pub(crate) start_index: usize,
    pub(crate) end_index: usize,
    pub(crate) link_type: LinkType,
    pub(crate) link_text: &'a str,
    args: VecPair,
}

impl<'a> Link<'a> {
    fn from_capture(cap: Captures<'a>) -> Option<Link<'a>> {
        let mut keys: Vec<String> = vec![];
        let mut values: Vec<String> = vec![];

        let link_type = match (cap.get(0), cap.get(1), cap.get(2), cap.get(3)) {
            (_, _, Some(file), Some(args)) => {
                let matches = ARGS.captures_iter(args.as_str());
                for mat in matches {
                    let capture = mat.unwrap().get(0).unwrap().as_str().splitn(2, '=');
                    for (i, capt) in capture.enumerate() {
                        if i % 2 == 0 {
                            keys.push(format!("{{{}}}", capt));
                        } else {
                            values.push(capt.to_string());
                        }
                    }
                }
                Some(LinkType::Template(PathBuf::from(file.as_str())))
            }
            (Some(mat), _, _, _) if mat.as_str().starts_with(ESCAPE_CHAR) => {
                Some(LinkType::Escaped)
            }
            _ => None,
        };

        link_type.and_then(|lnk_type| {
            cap.get(0).map(|mat| Link {
                start_index: mat.start(),
                end_index: mat.end(),
                link_type: lnk_type,
                link_text: mat.as_str(),
                args: VecPair(keys, values),
            })
        })
    }

    pub(crate) fn substitute_args_in_template<P: AsRef<Path>>(&self, base: P) -> Result<String> {
        match self.link_type {
            LinkType::Escaped => Ok((&self.link_text[1..]).to_owned()),
            LinkType::Template(ref pat) => {
                let target = base.as_ref().join(pat);

                fs::read_to_string(&target)
                    .with_context(|| {
                        format!(
                            "Could not read file for link {} ({})",
                            self.link_text,
                            target.display(),
                        )
                    })
                    .map(|hay| {
                        let pair = &self.args;
                        let ac = AhoCorasick::new_auto_configured(pair.0.as_slice());
                        ac.replace_all(hay.as_str(), pair.1.as_slice())
                    })
            }
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub(crate) enum LinkType {
    Escaped,
    Template(PathBuf),
}

impl LinkType {
    pub(crate) fn relative_path<P: AsRef<Path>>(self, base: P) -> Option<PathBuf> {
        match self {
            LinkType::Escaped => None,
            LinkType::Template(path) => Some(
                base.as_ref()
                    .join(path)
                    .parent()
                    .expect("Included file should not be /")
                    .to_path_buf(),
            ),
        }
    }
}

pub(crate) struct LinkIter<'a>(CaptureMatches<'a, 'a>);

impl<'a> Iterator for LinkIter<'a> {
    type Item = Link<'a>;
    fn next(&mut self) -> Option<Link<'a>> {
        for cap in &mut self.0 {
            if let Some(inc) = Link::from_capture(cap.unwrap()) {
                return Some(inc);
            }
        }
        None
    }
}

pub(crate) fn extract_template_links(contents: &str) -> LinkIter<'_> {
    LinkIter(WHOLE_TEMPLATE.captures_iter(contents))
}

#[cfg(test)]
mod link_tests {
    use std::path::PathBuf;

    use crate::links::{extract_template_links, Link, LinkType, VecPair};
    use crate::replace;

    #[test]
    fn test_escaped_template_link() {
        let start = r"
        Example Text
        ```hbs
        \{{#template template.md}} << an escaped link!
        ```";
        let end = r"
        Example Text
        ```hbs
        {{#template template.md}} << an escaped link!
        ```";
        assert_eq!(replace(start, "", "", 0), end);
    }

    #[test]
    fn test_extract_zero_template_links() {
        let string = "This is some text without any template links";
        assert_eq!(extract_template_links(string).collect::<Vec<_>>(), vec![])
    }

    #[test]
    fn test_extract_zero_template_links_without_args() {
        let string = "{{#template templates/footer.md}}";
        assert_eq!(extract_template_links(string).collect::<Vec<_>>(), vec![])
    }

    #[test]
    fn test_extract_template_links_simple_link() {
        let s =
            "Some random text with {{#template file.rs}} and {{#template test.rs test=nice}}...";

        let res = extract_template_links(s).collect::<Vec<_>>();

        assert_eq!(
            res,
            vec![Link {
                start_index: 48,
                end_index: 79,
                link_type: LinkType::Template(PathBuf::from("test.rs")),
                link_text: "{{#template test.rs test=nice}}",
                args: VecPair(vec!["{test}".to_string()], vec!["nice".to_string()])
            },]
        );
    }
}