pub mod file_name_formatter;
mod file_name_index;
mod file_name_matcher;
mod supported_file_types;

pub use file_name_index::{FileNameIndex, IndexStatus};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

pub const DEFAULT_MINIMUM_SIMILARITY_TO_REPLACE: u8 = 80;

#[derive(Default)]
pub struct PostProcessor {
    index: Option<Arc<FileNameIndex>>,
    minimum_similarity_percent: Option<Arc<AtomicU8>>,
}

impl PostProcessor {
    pub fn new(index: Arc<FileNameIndex>, minimum_similarity_percent: Arc<AtomicU8>) -> Self {
        Self {
            index: Some(index),
            minimum_similarity_percent: Some(minimum_similarity_percent),
        }
    }

    pub fn process(&self, transcript: &str, diagnostics: bool) -> String {
        let minimum_similarity = f64::from(
            self.minimum_similarity_percent
                .as_ref()
                .map_or(DEFAULT_MINIMUM_SIMILARITY_TO_REPLACE, |value| {
                    value.load(Ordering::Relaxed)
                }),
        ) / 100.0;
        let snapshot = self.index.as_ref().and_then(|index| index.snapshot());
        let mut has_reference = false;
        let processed = file_name_formatter::format_with_resolver(transcript, |name| {
            has_reference = true;
            snapshot.as_ref().map_or_else(
                || name.to_owned(),
                |names| names.resolve(name, minimum_similarity, diagnostics),
            )
        });
        if has_reference && snapshot.is_none() {
            if let Some(index) = &self.index {
                index.retry_after_error();
            }
        }
        processed
    }
}
