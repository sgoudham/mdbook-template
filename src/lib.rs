use fancy_regex::{CaptureMatches, Captures, Regex};
use lazy_static::lazy_static;
use log::info;
use mdbook::book::Book;
use mdbook::errors::Result;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::BookItem;
use toml::Value;

lazy_static! {
    static ref ARGS: Regex = Regex::new(r"(?<=\s|\A)([^\s=]+)=(.*?)(?=(?:\s[^\s=]+=|$))").unwrap();
    static ref WHOLE_TEMPLATE: Regex = Regex::new(r"\\\{\{\#.*\}\}|\{\{\s*\#([a-zA-Z0-9_]+)\s+([a-zA-Z0-9_.-/]+)\s*([^}]+)\}\}").unwrap();
    // static ref RE: Regex = Regex::new(r"\\\{\{\#.*\}\}|\{\{\s*\#([a-zA-Z0-9_]+)\s+([a-zA-Z0-9-_.]+)\s*((?<=\s|\A)([^\s=]+)=(.*?)(?=(?:\s[^\s=]+=|$)))*\}\}").unwrap();
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
        /*
        TODO - 29/04/2022
         Store All Files in Key/Value Pairs
         Start iterating through each chapter in the book
            1. Get template string
            2. Remove }} at the end of template string
            3. Store template string arguments in Key/Value Pairs
            4. Get the template and dynamically find/replace
            5. Set the chapter content to the new one
         Return the book to mdbook
         */

        env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

        if let Some(config) = ctx.config.get_preprocessor(self.name()) {
            let default = Value::String(String::from("templates/"));
            let template_dir = config.get("templates-dir").unwrap_or(&default);
            info!("Reading from directory {}", template_dir);
        }

        book.for_each_mut(|book_item| {
            if let BookItem::Chapter(ref mut chapter) = book_item {
                chapter.content = String::from("All content is now replaced");
            }
        });

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct Link<'a> {
    start_index: usize,
    end_index: usize,
    pub link_text: &'a str,
}

impl<'a> Link<'a> {
    fn from_capture(cap: Captures<'a>) -> Option<Link<'a>> {
        cap.get(0).map(|mat| Link {
            start_index: mat.start(),
            end_index: mat.end(),
            link_text: mat.as_str(),
        })
    }
}

pub struct LinkIter<'a>(CaptureMatches<'a, 'a>);

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

pub fn extract_template_links(contents: &str) -> LinkIter<'_> {
    LinkIter(WHOLE_TEMPLATE.captures_iter(contents))
}

pub fn extract_template_arguments(contents: &str) -> LinkIter<'_> {
    LinkIter(ARGS.captures_iter(contents))
}