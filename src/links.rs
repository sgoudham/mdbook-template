use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::SplitN;

use aho_corasick::AhoCorasick;
use anyhow::Context;
use fancy_regex::{CaptureMatches, Captures, Regex};
use lazy_static::lazy_static;
use mdbook::errors::Result;

const ESCAPE_CHAR: char = '\\';
const LINE_BREAKS: &[char] = &['\n', '\r'];

lazy_static! {
    // r"(?x)\\\{\{\#.*\}\}|\{\{\s*\#(template)\s+([a-zA-Z0-9_^'<>().:*+|\\\/?-]+)\s+([^}]+)\}\}")
    static ref WHOLE_TEMPLATE: Regex = Regex::new(
            r"(?x)                              # insignificant whitespace mode
            \\\{\{\#.*\}\}                      # match escaped link
            |                                   # or
            \{\{\s*                             # link opening parens and whitespace
            \#(template)                        # link type - template
            \s+                                 # separating whitespace
            ([a-zA-Z0-9_^'<>().:*+|\\\/?-]+)    # relative path to template file 
            \s+                                 # separating whitespace
            ([^}]+)                             # get all template arguments
            \}\}                                # link closing parens"
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
            (Some(mat), _, _, _) if mat.as_str().contains(LINE_BREAKS) => {
                let mut args = mat
                    .as_str()
                    .lines()
                    .map(|line| {
                        let end_trimmed = line.trim_end_matches(LINE_BREAKS);
                        end_trimmed.trim_start_matches(LINE_BREAKS)
                    })
                    .collect::<VecDeque<&str>>();

                // Remove {{#template
                args.pop_front();
                // Remove ending }}
                args.pop_back();
                // Store relative path of template file
                let file = args.pop_front().unwrap();

                for arg in args {
                    let capture = arg.splitn(2, '=');
                    populate_key_values(&mut keys, &mut values, capture);
                }

                Some(LinkType::Template(PathBuf::from(file.trim())))
            }
            (_, _, Some(file), Some(args)) => {
                let matches = ARGS.captures_iter(args.as_str());
                for mat in matches {
                    let capture = mat.unwrap().get(0).unwrap().as_str().splitn(2, '=');
                    populate_key_values(&mut keys, &mut values, capture);
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
                            "Could not read template file {} ({})",
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

fn populate_key_values<'a>(
    keys: &mut Vec<String>,
    values: &mut Vec<String>,
    split_str: SplitN<'a, char>,
) {
    for (i, capt) in split_str.enumerate() {
        if i % 2 == 0 {
            keys.push(format!("{{{}}}", capt.trim()));
        } else {
            values.push(capt.to_string());
        }
    }
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
                args: VecPair(vec!["{lang}".to_string()], vec!["rust".to_string()])
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
                args: VecPair(
                    vec!["{lang}".to_string(), "{math}".to_string()],
                    vec!["rust".to_string(), "2+2=4".to_string()],
                )
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
                args: VecPair(
                    vec!["{lang}".to_string(), "{authors}".to_string()],
                    vec!["rust".to_string(), "Goudham & Hazel".to_string()]
                )
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
                args: VecPair(
                    vec!["{lang}".to_string(), "{authors}".to_string()],
                    vec!["rust".to_string(), "Goudham & Hazel".to_string()]
                )
            },]
        );
    }

    #[test]
    fn test_extract_template_links_with_special_characters() {
        let s = "Some random text with {{#template foo-bar\\-baz/_c++.rs path=images}}...";

        let res = extract_template_links(s).collect::<Vec<_>>();

        assert_eq!(
            res,
            vec![Link {
                start_index: 22,
                end_index: 68,
                link_type: LinkType::Template(PathBuf::from("foo-bar\\-baz/_c++.rs")),
                link_text: "{{#template foo-bar\\-baz/_c++.rs path=images}}",
                args: VecPair(vec!["{path}".to_string()], vec!["images".to_string()])
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
                args: VecPair(
                    vec![
                        "{lang}".to_string(),
                        "{authors}".to_string(),
                        "{year}".to_string()
                    ],
                    vec![
                        "rust".to_string(),
                        "Goudham & Hazel".to_string(),
                        "2022".to_string()
                    ]
                )
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
                args: VecPair(
                    vec![
                        "{lang}".to_string(),
                        "{authors}".to_string(),
                        "{year}".to_string()
                    ],
                    vec![
                        "rust".to_string(),
                        "Goudham & Hazel".to_string(),
                        "2022".to_string()
                    ]
                )
            },]
        );
    }
}