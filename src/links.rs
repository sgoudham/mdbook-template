use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use aho_corasick::AhoCorasick;
use anyhow::Context;
use fancy_regex::{CaptureMatches, Captures, Regex};
use lazy_static::lazy_static;
use mdbook::errors::Result;

const ESCAPE_CHAR: char = '\\';
const LINE_BREAKS: &[char] = &['\n', '\r'];

lazy_static! {
    // https://stackoverflow.com/questions/22871602/optimizing-regex-to-fine-key-value-pairs-space-delimited
    static ref TEMPLATE_ARGS: Regex = Regex::new(r"(?<=\s|\A)([^\s=]+)=(.*?)(?=(?:\s[^\s=]+=|$))").unwrap();

    // r"(?x)\\\{\{\#.*\}\}|\{\{\s*\#(template)\s+([a-zA-Z0-9_^'<>().:*+|\\\/?-]+)\s+([^}]+)\}\}"
    static ref TEMPLATE: Regex = Regex::new(
        r"(?x)                              # enable insignificant whitespace mode
         
        \\\{\{                              # escaped link opening parens
        \#.*                                # match any character
        \}\}                                # escaped link closing parens
         
        |                                   # or
         
        \{\{\s*                             # link opening parens and whitespace(s)
        \#(template)                        # link type - template
        \s+                                 # separating whitespace
        ([\w'<>.:^\-\(\)\*\+\|\\\/\?]+)     # relative path to template file 
        \s+                                 # separating whitespace(s)
        ([^}]+)                             # get all template arguments
        \}\}                                # link closing parens"
    )
    .unwrap();

    // r"(?x)\\\{\{\#.*\}\}|\{\{\s*\#([\w'<>.:^\-\(\)\*\+\|\\\/\?]+)\s*\}\}|\{\{\s*\#([\w'<>.:^\-\(\)\*\+\|\\\/\?]+)\s+([^}]+)\}\}"
    static ref ARGS: Regex = Regex::new(
        r"(?x)                                  # enable insignificant whitespace mode
         
        \\\{\{                                  # escaped link opening parens  
        \#.*                                    # match any character          
        \}\}                                    # escaped link closing parens  
         
        |                                       # or
         
        \{\{\s*                                 # link opening parens and whitespace(s)
        \#([\w'<>.:^\-\(\)\*\+\|\\\/\?]+)       # arg name 
        \s*                                     # optional separating whitespace(s)
        \}\}                                    # link closing parens
         
        |                                       # or
         
        \{\{\s*                                 # link opening parens and whitespace
        \#([\w'<>.:^\-\(\)\*\+\|\\\/\?]+)       # arg name
        \s+                                     # separating whitespace(s)
        ([^}]+)                                 # get default value for argument
        \}\}                                    # link closing parens"
    )
    .unwrap();
}

#[derive(PartialEq, Debug, Clone)]
pub(crate) struct Link<'a> {
    pub(crate) start_index: usize,
    pub(crate) end_index: usize,
    pub(crate) link_type: LinkType,
    pub(crate) link_text: &'a str,
    args: HashMap<String, &'a str>,
}

impl<'a> Link<'a> {
    fn from_capture(cap: Captures<'a>) -> Option<Link<'a>> {
        let mut all_args = HashMap::with_capacity(20);

        let link_type = match (cap.get(0), cap.get(1), cap.get(2), cap.get(3)) {
            (Some(mat), _, _, _) if mat.as_str().contains(LINE_BREAKS) => {
                /*
                Given a template string that looks like:
                {{#template
                    footer.md
                    path=../images
                    author=Hazel
                }}

                The resulting args: <VecDeque<&str> will look like:
                ["{{#template", "footer.md", "path=../images", "author=Hazel", "}}"]
                 */
                let mut args = mat
                    .as_str()
                    .lines()
                    .map(|line| {
                        line.trim_end_matches(LINE_BREAKS)
                            .trim_start_matches(LINE_BREAKS)
                    })
                    .collect::<VecDeque<_>>();

                // Remove {{#template
                args.pop_front();
                // Remove ending }}
                args.pop_back();
                // Store relative path of template file
                let file = args.pop_front().unwrap();

                let split_args = args
                    .into_iter()
                    .map(|arg| {
                        let mut split_n = arg.splitn(2, '=');
                        let key = format!("{{{}}}", split_n.next().unwrap().trim());
                        let value = split_n.next().unwrap();
                        (key, value)
                    })
                    .collect::<Vec<_>>();
                all_args.extend(split_args);

                Some(LinkType::Template(PathBuf::from(file.trim())))
            }
            (_, _, Some(file), Some(args)) => {
                all_args.extend(extract_template_args(args.as_str()).collect::<Vec<_>>());
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
                args: all_args,
            })
        })
    }

    pub(crate) fn replace_args<P: AsRef<Path>>(&self, base: P) -> Result<String> {
        match self.link_type {
            LinkType::Escaped => Ok((&self.link_text[1..]).to_owned()),
            LinkType::Template(ref pat) => {
                let target = base.as_ref().join(pat);

                fs::read_to_string(&target)
                    .with_context(|| {
                        format!(
                            "Could not read template file {} ({})",
                            self.link_text,
                            target.display(),
                        )
                    })
                    .map(|hay| {
                        let pair = &self.args;
                        let ac = AhoCorasick::new_auto_configured(
                            pair.keys().collect::<Vec<_>>().as_slice(),
                        );
                        ac.replace_all(hay.as_str(), pair.values().collect::<Vec<_>>().as_slice())
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

    fn next(&mut self) -> Option<Self::Item> {
        for cap in &mut self.0 {
            if let Some(inc) = Link::from_capture(cap.unwrap()) {
                return Some(inc);
            }
        }
        None
    }
}

pub(crate) fn extract_template_links(contents: &str) -> LinkIter<'_> {
    LinkIter(TEMPLATE.captures_iter(contents))
}

struct TemplateArgsIter<'a>(CaptureMatches<'a, 'a>);

impl<'a> Iterator for TemplateArgsIter<'a> {
    type Item = (String, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        for mat in &mut self.0 {
            let mut split_capt = mat.unwrap().get(0).unwrap().as_str().splitn(2, '=');
            let key = format!("{{{}}}", split_capt.next().unwrap().trim());
            let value = split_capt.next().unwrap();
            return Some((key, value));
        }
        None
    }
}

fn extract_template_args(contents: &str) -> TemplateArgsIter<'_> {
    TemplateArgsIter(TEMPLATE_ARGS.captures_iter(contents))
}

#[cfg(test)]
mod link_tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::links::{extract_template_links, Link, LinkType};
    use crate::replace_template;

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
        assert_eq!(replace_template(start, "", "", 0), end);
    }

    #[test]
    fn test_extract_zero_template_links() {
        let s = "This is some text without any template links";
        assert_eq!(extract_template_links(s).collect::<Vec<_>>(), vec![])
    }

    #[test]
    fn test_extract_zero_template_links_without_args() {
        let s = "{{#template templates/footer.md}}";
        assert_eq!(extract_template_links(s).collect::<Vec<_>>(), vec![])
    }

    #[test]
    fn test_extract_template_links_partial_match() {
        let s = "Some random text with {{#template...";
        assert_eq!(extract_template_links(s).collect::<Vec<_>>(), vec![]);
        let s = "Some random text with {{#template footer.md...";
        assert_eq!(extract_template_links(s).collect::<Vec<_>>(), vec![]);
        let s = "Some random text with {{#template footer.md path=../images...";
        assert_eq!(extract_template_links(s).collect::<Vec<_>>(), vec![]);
        let s = "Some random text with \\{{#template...";
        assert_eq!(extract_template_links(s).collect::<Vec<_>>(), vec![]);
    }

    #[test]
    fn test_extract_template_links_empty() {
        let s = "Some random text with {{#template}} and {{#template  }} {{}} {{#}}...";
        assert_eq!(extract_template_links(s).collect::<Vec<_>>(), vec![]);
    }

    #[test]
    fn test_extract_template_links_unknown() {
        let s = "Some random text with {{#templatee file.rs}} and {{#include}} {{#playground}} {{#tempate}}...";
        assert!(extract_template_links(s).collect::<Vec<_>>() == vec![]);
    }

    #[test]
    fn test_extract_template_links_simple() {
        let s =
            "Some random text with {{#template file.rs}} and {{#template test.rs lang=rust}}...";

        let res = extract_template_links(s).collect::<Vec<_>>();

        assert_eq!(
            res,
            vec![Link {
                start_index: 48,
                end_index: 79,
                link_type: LinkType::Template(PathBuf::from("test.rs")),
                link_text: "{{#template test.rs lang=rust}}",
                args: HashMap::from([("{lang}".to_string(), "rust")])
            },]
        );
    }

    #[test]
    fn test_extract_template_links_simple_with_equals_sign() {
        let s = "Some random text with{{#template test.rs lang=rust math=2+2=4}}...";

        let res = extract_template_links(s).collect::<Vec<_>>();

        assert_eq!(
            res,
            vec![Link {
                start_index: 21,
                end_index: 63,
                link_type: LinkType::Template(PathBuf::from("test.rs")),
                link_text: "{{#template test.rs lang=rust math=2+2=4}}",
                args: HashMap::from([
                    ("{lang}".to_string(), "rust"),
                    ("{math}".to_string(), "2+2=4")
                ]),
            },]
        );
    }

    #[test]
    fn test_extract_template_links_simple_with_whitespace() {
        let s = "Some random text with {{#template test.rs lang=rust authors=Goudham & Hazel}}...";

        let res = extract_template_links(s).collect::<Vec<_>>();

        assert_eq!(
            res,
            vec![Link {
                start_index: 22,
                end_index: 77,
                link_type: LinkType::Template(PathBuf::from("test.rs")),
                link_text: "{{#template test.rs lang=rust authors=Goudham & Hazel}}",
                args: HashMap::from([
                    ("{lang}".to_string(), "rust"),
                    ("{authors}".to_string(), "Goudham & Hazel")
                ]),
            },]
        );
    }

    #[test]
    fn test_extract_template_links_simple_with_tabs() {
        let s = "Some random text with {{#template      test.rs      lang=rust authors=Goudham & Hazel}}...";

        let res = extract_template_links(s).collect::<Vec<_>>();

        assert_eq!(
            res,
            vec![Link {
                start_index: 22,
                end_index: 87,
                link_type: LinkType::Template(PathBuf::from("test.rs")),
                link_text: "{{#template      test.rs      lang=rust authors=Goudham & Hazel}}",
                args: HashMap::from([
                    ("{lang}".to_string(), "rust"),
                    ("{authors}".to_string(), "Goudham & Hazel")
                ]),
            },]
        );
    }

    #[test]
    fn test_extract_template_links_with_special_characters() {
        let s = "Some random text with {{#template foo-bar\\-baz/_c++.'.rs path=images}}...";

        let res = extract_template_links(s).collect::<Vec<_>>();

        assert_eq!(
            res,
            vec![Link {
                start_index: 22,
                end_index: 70,
                link_type: LinkType::Template(PathBuf::from("foo-bar\\-baz/_c++.'.rs")),
                link_text: "{{#template foo-bar\\-baz/_c++.'.rs path=images}}",
                args: HashMap::from([("{path}".to_string(), "images")]),
            },]
        );
    }

    #[test]
    fn test_extract_template_links_newlines() {
        let s = "{{#template
            test.rs
            lang=rust
            authors=Goudham & Hazel
            year=2022
        }}";

        let res = extract_template_links(s).collect::<Vec<_>>();

        assert_eq!(
            res,
            vec![Link {
                start_index: 0,
                end_index: 122,
                link_type: LinkType::Template(PathBuf::from("test.rs")),
                link_text: "{{#template\n            test.rs\n            lang=rust\n            authors=Goudham & Hazel\n            year=2022\n        }}",
                args: HashMap::from([("{lang}".to_string(), "rust"), ("{authors}".to_string(), "Goudham & Hazel"), ("{year}".to_string(), "2022")]),
            },]
        );
    }

    #[test]
    fn test_extract_template_links_with_newlines_tabs() {
        let s = "{{#template
    test.rs
lang=rust
        authors=Goudham & Hazel
year=2022
}}";

        let res = extract_template_links(s).collect::<Vec<_>>();

        assert_eq!(
            res,
            vec![Link {
                start_index: 0,
                end_index: 78,
                link_type: LinkType::Template(PathBuf::from("test.rs")),
                link_text: "{{#template\n    test.rs\nlang=rust\n        authors=Goudham & Hazel\nyear=2022\n}}",
                args: HashMap::from([("{lang}".to_string(), "rust"), ("{authors}".to_string(), "Goudham & Hazel"), ("{year}".to_string(), "2022")]),
            },]
        );
    }
}