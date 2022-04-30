use std::fs;
use std::path::{Path, PathBuf};

use aho_corasick::AhoCorasick;
use anyhow::Context;
use fancy_regex::{CaptureMatches, Captures, Regex};
use lazy_static::lazy_static;
use log::{error, warn};
use mdbook::book::Book;
use mdbook::errors::Result;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::BookItem;

const ESCAPE_CHAR: char = '\\';
const MAX_LINK_NESTED_DEPTH: usize = 10;

lazy_static! {
    // https://stackoverflow.com/questions/22871602/optimizing-regex-to-fine-key-value-pairs-space-delimited
    static ref ARGS: Regex = Regex::new(r"(?<=\s|\A)([^\s=]+)=(.*?)(?=(?:\s[^\s=]+=|$))").unwrap();
    // TODO: Explain This Horrible Mess
    static ref WHOLE_TEMPLATE: Regex = Regex::new(r"\\\{\{\#.*\}\}|\{\{\s*\#(template)\s+([a-zA-Z0-9_.\/-]+)\s*([^}]+)\}\}").unwrap();
}

#[derive(Default)]
pub struct Template;

impl Template {
    pub fn new() -> Self {
        Template
    }
}

impl Preprocessor for Template {
    fn name(&self) -> &str {
        "template"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
        let src_dir = ctx.root.join(&ctx.config.book.src);

        book.for_each_mut(|section| {
            if let BookItem::Chapter(ref mut chapter) = section {
                if let Some(ref source) = chapter.path {
                    let base = source
                        .parent()
                        .map(|dir| src_dir.join(dir))
                        .expect("All book items have a parent");

                    let content = replace(&chapter.content, base, source, 0);
                    chapter.content = content;
                }
            }
        });

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

fn replace<P1, P2>(chapter_content: &str, base: P1, source: P2, depth: usize) -> String
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let path = base.as_ref();
    let source = source.as_ref();
    // Must keep track of indices as they will not correspond after string substitution
    let mut previous_end_index = 0;
    let mut replaced = String::with_capacity(chapter_content.len());

    for link in extract_template_links(chapter_content) {
        replaced.push_str(&chapter_content[previous_end_index..link.start_index]);

        match link.substitute_args_in_template(&path) {
            Ok(new_content) => {
                if depth < MAX_LINK_NESTED_DEPTH {
                    if let Some(rel_path) = link.link_type.relative_path(path) {
                        replaced.push_str(&replace(&new_content, rel_path, source, depth + 1));
                    } else {
                        replaced.push_str(&new_content);
                    }
                } else {
                    error!(
                        "Stack Overflow! {}. Check For Cyclic Templates",
                        source.display()
                    );
                }
                previous_end_index = link.end_index;
            }
            Err(err) => {
                error!("Error updating \"{}\", {}", link.link_text, err);
                for cause in err.chain().skip(1) {
                    warn!("Caused By: {}", cause);
                }

                // Include `{{# ... }}` snippet when errors occur
                previous_end_index = link.start_index;
            }
        }
    }

    replaced.push_str(&chapter_content[previous_end_index..]);
    replaced
}

#[derive(PartialEq, Debug, Clone)]
enum LinkType {
    Escaped,
    Template(PathBuf),
}

impl LinkType {
    fn relative_path<P: AsRef<Path>>(self, base: P) -> Option<PathBuf> {
        match self {
            LinkType::Escaped => None,
            LinkType::Template(p) => Some(return_relative_path(base.as_ref(), &p)),
        }
    }
}

fn return_relative_path<P: AsRef<Path>>(base: P, relative: P) -> PathBuf {
    base.as_ref()
        .join(relative)
        .parent()
        .expect("Included file should not be /")
        .to_path_buf()
}

#[derive(PartialEq, Debug, Clone)]
struct VecPair(Vec<String>, Vec<String>);

#[derive(PartialEq, Debug, Clone)]
struct Link<'a> {
    start_index: usize,
    end_index: usize,
    args: VecPair,
    link_type: LinkType,
    link_text: &'a str,
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
                args: VecPair(keys, values),
                link_type: lnk_type,
                link_text: mat.as_str(),
            })
        })
    }

    fn substitute_args_in_template<P: AsRef<Path>>(&self, base: P) -> Result<String> {
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

struct LinkIter<'a>(CaptureMatches<'a, 'a>);

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

fn extract_template_links(contents: &str) -> LinkIter<'_> {
    LinkIter(WHOLE_TEMPLATE.captures_iter(contents))
}