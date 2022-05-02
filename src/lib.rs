use std::path::Path;

use log::{error, warn};
use mdbook::book::Book;
use mdbook::errors::Result;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::BookItem;

mod links;

const MAX_LINK_NESTED_DEPTH: usize = 10;

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

                    let content = replace_template(&chapter.content, base, source, 0);
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

fn replace_template<P1, P2>(chapter_content: &str, base: P1, source: P2, depth: usize) -> String
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let path = base.as_ref();
    let source = source.as_ref();
    // Must keep track of indices as they will not correspond after string substitution
    let mut previous_end_index = 0;
    let mut replaced = String::with_capacity(chapter_content.len());

    for link in links::extract_template_links(chapter_content) {
        replaced.push_str(&chapter_content[previous_end_index..link.start_index]);

        match link.substitute_args_in_template(&path) {
            Ok(new_content) => {
                if depth < MAX_LINK_NESTED_DEPTH {
                    if let Some(rel_path) = link.link_type.relative_path(path) {
                        replaced.push_str(&replace_template(
                            &new_content,
                            rel_path,
                            source,
                            depth + 1,
                        ));
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