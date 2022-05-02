use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use aho_corasick::AhoCorasick;
use anyhow::{Context, Error};
use fancy_regex::{CaptureMatches, Captures, Match, Regex};
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
    args: HashMap<&'a str, &'a str>,
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
                        let key = split_n.next().unwrap().trim();
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

                let contents = fs::read_to_string(&target).with_context(|| {
                    format!(
                        "Could not read template file {} ({})",
                        self.link_text,
                        target.display(),
                    )
                })?;

                Ok(Args::replace(contents.as_str(), &self.args))
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
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        for cap in &mut self.0 {
            let mut split_args = cap.unwrap().get(0).unwrap().as_str().splitn(2, '=');
            let key = split_args.next().unwrap().trim();
            let value = split_args.next().unwrap();
            return Some((key, value));
        }
        None
    }
}

fn extract_template_args(contents: &str) -> TemplateArgsIter<'_> {
    TemplateArgsIter(TEMPLATE_ARGS.captures_iter(contents))
}

#[derive(PartialEq, Debug, Clone)]
struct Args<'a> {
    start_index: usize,
    end_index: usize,
    args_type: ArgsType<'a>,
    args_text: &'a str,
}

impl<'a> Args<'a> {
    fn replace(contents: &str, all_args: &HashMap<&str, &str>) -> String {
        // Must keep track of indices as they will not correspond after string substitution
        let mut previous_end_index = 0;
        let mut replaced = String::with_capacity(contents.len());

        for captured_arg in extract_args(contents) {
            replaced.push_str(&contents[previous_end_index..captured_arg.start_index]);

            match captured_arg.args_type {
                ArgsType::Escaped => replaced.push_str(&captured_arg.args_text[1..]),
                ArgsType::Plain(argument) => match all_args.get(argument) {
                    None => {}
                    Some(value) => replaced.push_str(value),
                },
                ArgsType::Default(argument, default_value) => {
                    // [TEM #2]
                    // check if captured_arg exists within hashmap
                    // if so, replace arg with corresponding value and push to replaced string
                    // if not, replace arg with default value and push to replaced string
                }
            }

            previous_end_index = captured_arg.end_index;
        }

        replaced.push_str(&contents[previous_end_index..]);
        replaced
    }

    fn from_capture(cap: Captures<'a>) -> Option<Args<'a>> {
        let arg_type = match (cap.get(0), cap.get(1), cap.get(2)) {
            (_, Some(argument), None) => {
                println!("Argument -> {:?}", argument);
                Some(ArgsType::Plain(argument.as_str()))
            }
            (_, Some(argument), Some(default_value)) => {
                println!("Argument -> {:?}", argument);
                println!("Default Value -> {:?}", default_value);
                Some(ArgsType::Default(argument.as_str(), default_value.as_str()))
            }
            (Some(mat), _, _) if mat.as_str().starts_with(ESCAPE_CHAR) => {
                println!("Escaped -> {}", mat.as_str());
                Some(ArgsType::Escaped)
            }
            _ => None,
        };

        arg_type.and_then(|arg_type| {
            cap.get(0).map(|capt| Args {
                start_index: capt.start(),
                end_index: capt.end(),
                args_type: arg_type,
                args_text: capt.as_str(),
            })
        })
    }
}

#[derive(PartialEq, Debug, Clone)]
enum ArgsType<'a> {
    Escaped,
    Plain(&'a str),
    Default(&'a str, &'a str),
}

struct ArgsIter<'a>(CaptureMatches<'a, 'a>);

impl<'a> Iterator for ArgsIter<'a> {
    type Item = Args<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        for cap in &mut self.0 {
            if let Some(inc) = Args::from_capture(cap.unwrap()) {
                return Some(inc);
            }
        }
        None
    }
}

fn extract_args(contents: &str) -> ArgsIter<'_> {
    ArgsIter(ARGS.captures_iter(contents))
}

#[cfg(test)]
mod link_tests {
    use std::any::Any;
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::links::{extract_args, extract_template_links, Args, ArgsType, Link, LinkType};
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
                args: HashMap::from([("lang", "rust")])
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
                args: HashMap::from([("lang", "rust"), ("math", "2+2=4")]),
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
                args: HashMap::from([("lang", "rust"), ("authors", "Goudham & Hazel")]),
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
                args: HashMap::from([("lang", "rust"), ("authors", "Goudham & Hazel")]),
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
                args: HashMap::from([("path", "images")]),
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
                args: HashMap::from([("lang", "rust"), ("authors", "Goudham & Hazel"), ("year", "2022")]),
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
                args: HashMap::from([("lang", "rust"), ("authors", "Goudham & Hazel"), ("year", "2022")]),
            },]
        );
    }

    #[test]
    fn test_extract_zero_args() {
        let s = "This is some text without any template links";
        assert_eq!(extract_args(s).collect::<Vec<_>>(), vec![])
    }

    #[test]
    fn test_extract_args_partial_match() {
        let s = "Some random text with {{#height...";
        assert_eq!(extract_args(s).collect::<Vec<_>>(), vec![]);
        let s = "Some random text with {{#image ferris.png...";
        assert_eq!(extract_args(s).collect::<Vec<_>>(), vec![]);
        let s = "Some random text with {{#width 550...";
        assert_eq!(extract_args(s).collect::<Vec<_>>(), vec![]);
        let s = "Some random text with \\{{#title...";
        assert_eq!(extract_args(s).collect::<Vec<_>>(), vec![]);
    }

    #[test]
    fn test_extract_args_empty() {
        let s = "Some random text with {{}} {{#}}...";
        assert_eq!(extract_args(s).collect::<Vec<_>>(), vec![]);
    }

    #[test]
    fn test_extract_args_simple() {
        let s = "This is some random text with {{#path}} and then some more random text";

        let res = extract_args(s).collect::<Vec<_>>();

        assert_eq!(
            res,
            vec![Args {
                start_index: 30,
                end_index: 39,
                args_type: ArgsType::Plain("path"),
                args_text: "{{#path}}"
            }]
        );
    }

    #[test]
    fn test_extract_args_escaped() {
        let start = r"
        Example Text
        \{{#height 200px}} << an escaped argument!
        ";
        let end = r"
        Example Text
        {{#height 200px}} << an escaped argument!
        ";
        assert_eq!(Args::replace(start, &HashMap::<&str, &str>::new()), end);
    }

    #[test]
    fn test_replace_args_simple() {
        let start = r"
        Example Text
        {{#height}} << an argument!
        ";
        let end = r"
        Example Text
        200px << an argument!
        ";
        assert_eq!(
            Args::replace(start, &HashMap::from([("height", "200px")])),
            end
        );
    }

    #[test]
    fn test_extract_args_with_spaces() {
        let s1 = "This is some random text with {{     #path       }}";
        let s2 = "This is some random text with {{#path       }}";
        let s3 = "This is some random text with {{     #path}}";

        let res1 = extract_args(s1).collect::<Vec<_>>();
        let res2 = extract_args(s2).collect::<Vec<_>>();
        let res3 = extract_args(s3).collect::<Vec<_>>();

        assert_eq!(
            res1,
            vec![Args {
                start_index: 30,
                end_index: 51,
                args_type: ArgsType::Plain("path"),
                args_text: "{{     #path       }}"
            }]
        );

        assert_eq!(
            res2,
            vec![Args {
                start_index: 30,
                end_index: 46,
                args_type: ArgsType::Plain("path"),
                args_text: "{{#path       }}"
            }]
        );

        assert_eq!(
            res3,
            vec![Args {
                start_index: 30,
                end_index: 44,
                args_type: ArgsType::Plain("path"),
                args_text: "{{     #path}}"
            }]
        );
    }

    // #[test]
    fn test_extract_args_with_default_value() {}

    // #[test]
    fn test_extract_args_with_default_value_and_spaces() {}
}