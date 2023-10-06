use std::path::Path;

use log::{error, warn};
use mdbook::book::Book;
use mdbook::errors::Result;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::BookItem;

use crate::utils::{FileReader, SystemFileReader};

mod links;
pub mod utils;

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

                    let content =
                        replace_template(&chapter.content, &SystemFileReader, base, source, 0);
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

pub fn replace_template<P1, P2, FR>(
    chapter_content: &str,
    file_reader: &FR,
    base: P1,
    source: P2,
    depth: usize,
) -> String
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
    FR: FileReader,
{
    let path = base.as_ref();
    let source = source.as_ref();
    // Must keep track of indices as they will not correspond after string substitution
    let mut previous_end_index = 0;
    let mut replaced = String::with_capacity(chapter_content.len());

    for link in links::extract_template_links(chapter_content) {
        replaced.push_str(&chapter_content[previous_end_index..link.start_index]);

        match link.replace_args(path, file_reader) {
            Ok(new_content) => {
                if depth < MAX_LINK_NESTED_DEPTH {
                    if let Some(rel_path) = link.link_type.relative_path(path) {
                        replaced.push_str(&replace_template(
                            &new_content,
                            file_reader,
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

#[cfg(test)]
mod lib_tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::replace_template;
    use crate::utils::TestFileReader;

    #[test]
    fn test_happy_path_escaped() {
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

        assert_eq!(
            replace_template(start, &TestFileReader::default(), "", "", 0),
            end
        );
    }

    #[test]
    fn test_happy_path_simple() {
        let start_chapter_content = "{{#template footer.md}}";
        let end_chapter_content = "Designed & Created With Love From - Goudham & Hazel";
        let file_name = PathBuf::from("footer.md");
        let template_file_contents =
            "Designed & Created With Love From - Goudham & Hazel".to_string();
        let map = HashMap::from([(file_name, template_file_contents)]);
        let file_reader = &TestFileReader::from(map);

        let actual_chapter_content =
            replace_template(start_chapter_content, file_reader, "", "", 0);

        assert_eq!(actual_chapter_content, end_chapter_content);
    }

    #[test]
    fn test_happy_path_with_args() {
        let start_chapter_content = "{{#template footer.md authors=Goudham & Hazel}}";
        let end_chapter_content = "Designed & Created With Love From - Goudham & Hazel";
        let file_name = PathBuf::from("footer.md");
        let template_file_contents = "Designed & Created With Love From - [[#authors]]".to_string();
        let map = HashMap::from([(file_name, template_file_contents)]);
        let file_reader = &TestFileReader::from(map);

        let actual_chapter_content =
            replace_template(start_chapter_content, file_reader, "", "", 0);

        assert_eq!(actual_chapter_content, end_chapter_content);
    }

    #[test]
    fn test_happy_path_new_lines() {
        let start_chapter_content = r"
        Some content...
        {{#template footer.md authors=Goudham & Hazel}}";
        let end_chapter_content = r"
        Some content...
        - - - -
        Designed & Created With Love From Goudham & Hazel";
        let file_name = PathBuf::from("footer.md");
        let template_file_contents = r"- - - -
        Designed & Created With Love From [[#authors]]"
            .to_string();
        let map = HashMap::from([(file_name, template_file_contents)]);
        let file_reader = &TestFileReader::from(map);

        let actual_chapter_content =
            replace_template(start_chapter_content, file_reader, "", "", 0);

        assert_eq!(actual_chapter_content, end_chapter_content);
    }

    #[test]
    fn test_happy_path_multiple() {
        let start_chapter_content = r"
        {{#template header.md title=Example Title}}
        Some content...
        {{#template
            footer.md
        authors=Goudham & Hazel}}";
        let end_chapter_content = r"
        # Example Title
        Some content...
        - - - -
        Designed & Created With Love From Goudham & Hazel";
        let header_file_name = PathBuf::from("header.md");
        let header_contents = r"# [[#title]]".to_string();
        let footer_file_name = PathBuf::from("footer.md");
        let footer_contents = r"- - - -
        Designed & Created With Love From [[#authors]]"
            .to_string();
        let map = HashMap::from([
            (footer_file_name, footer_contents),
            (header_file_name, header_contents),
        ]);
        let file_reader = &TestFileReader::from(map);

        let actual_chapter_content =
            replace_template(start_chapter_content, file_reader, "", "", 0);

        assert_eq!(actual_chapter_content, end_chapter_content);
    }

    #[test]
    fn test_happy_path_with_default_values() {
        let start_chapter_content = "{{#template footer.md}}";
        let end_chapter_content = "Designed By - Goudham";
        let file_name = PathBuf::from("footer.md");
        let template_file_contents = "Designed By - [[#authors Goudham]]".to_string();
        let map = HashMap::from([(file_name, template_file_contents)]);
        let file_reader = &TestFileReader::from(map);

        let actual_chapter_content =
            replace_template(start_chapter_content, file_reader, "", "", 0);

        assert_eq!(actual_chapter_content, end_chapter_content);
    }

    #[test]
    fn test_happy_path_with_overridden_default_values() {
        let start_chapter_content = "{{#template footer.md authors=Hazel}}";
        let end_chapter_content = "Designed By - Hazel";
        let file_name = PathBuf::from("footer.md");
        let template_file_contents = "Designed By - [[#authors Goudham]]".to_string();
        let map = HashMap::from([(file_name, template_file_contents)]);
        let file_reader = &TestFileReader::from(map);

        let actual_chapter_content =
            replace_template(start_chapter_content, file_reader, "", "", 0);

        assert_eq!(actual_chapter_content, end_chapter_content);
    }

    #[test]
    fn test_happy_path_nested() {
        let start_chapter_content = r"
        {{#template header.md title=Example Title}}
        Some content...";
        let end_chapter_content = r"
        # Example Title
        <img src='example.png' alt='Example Title'>
        Some content...";
        let header_file_name = PathBuf::from("header.md");
        let header_contents = r"# [[#title]]
        {{#template image.md title=[[#title]]}}"
            .to_string();
        let image_file_name = PathBuf::from("image.md");
        let image_contents = r"<img src='example.png' alt='[[#title]]'>".to_string();
        let map = HashMap::from([
            (image_file_name, image_contents),
            (header_file_name, header_contents),
        ]);
        let file_reader = &TestFileReader::from(map);

        let actual_chapter_content =
            replace_template(start_chapter_content, file_reader, "", "", 0);

        assert_eq!(actual_chapter_content, end_chapter_content);
    }

    #[test]
    fn test_sad_path_invalid_file() {
        env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

        let start_chapter_content = "{{#template footer.md}}";

        let actual_chapter_content =
            replace_template(start_chapter_content, &TestFileReader::default(), "", "", 0);

        assert_eq!(actual_chapter_content, start_chapter_content);
    }
}
