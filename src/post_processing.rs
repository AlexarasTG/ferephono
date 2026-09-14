pub mod file_name_formatter;
mod file_name_index;
mod file_name_matcher;
mod supported_file_types;

pub use file_name_index::{FileNameIndex, IndexStatus};
use std::sync::Arc;

#[derive(Default)]
pub struct PostProcessor {
    index: Option<Arc<FileNameIndex>>,
}

impl PostProcessor {
    pub fn new(index: Arc<FileNameIndex>) -> Self {
        Self { index: Some(index) }
    }

    pub fn process(&self, transcript: &str, diagnostics: bool) -> String {
        let snapshot = self.index.as_ref().and_then(|index| index.snapshot());
        let mut has_reference = false;
        let processed = file_name_formatter::format_with_resolver(transcript, |name| {
            has_reference = true;
            snapshot
                .as_ref()
                .map_or_else(|| name.to_owned(), |names| names.resolve(name, diagnostics))
        });
        if has_reference && snapshot.is_none() {
            if let Some(index) = &self.index {
                index.retry_after_error();
            }
        }
        processed
    }
}
