use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Error, Result};

pub trait FileReader {
    fn read_to_string(&self, file_name: &Path, template_text: &str) -> Result<String>;
}

#[derive(PartialEq, Eq, Debug, Clone, Default)]
pub struct SystemFileReader;

#[derive(PartialEq, Eq, Debug, Clone, Default)]
pub struct TestFileReader {
    pub captured_contents: HashMap<PathBuf, String>,
}

impl FileReader for SystemFileReader {
    fn read_to_string(&self, file_name: &Path, template_text: &str) -> Result<String> {
        fs::read_to_string(file_name).with_context(|| {
            format!(
                "Could not read template file {} ({})",
                template_text,
                file_name.display(),
            )
        })
    }
}

impl From<HashMap<PathBuf, String>> for TestFileReader {
    fn from(map: HashMap<PathBuf, String>) -> Self {
        TestFileReader {
            captured_contents: map,
        }
    }
}

impl FileReader for TestFileReader {
    fn read_to_string(&self, file_name: &Path, template_text: &str) -> Result<String> {
        match self.captured_contents.get(file_name) {
            Some(file_contents) => Ok(file_contents.to_string()),
            None => Err(Error::msg(format!(
                "Could not read template file {} ({})",
                template_text,
                file_name.display(),
            ))),
        }
    }
}
