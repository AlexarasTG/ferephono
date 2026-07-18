use async_trait::async_trait;
use enigo::{Enigo, Keyboard, Settings};

use crate::TextInserter;

pub struct DirectTextInserter;

#[async_trait]
impl TextInserter for DirectTextInserter {
    async fn insert(&self, text: &str) -> Result<(), String> {
        let text = text.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut enigo = Enigo::new(&Settings::default())
                .map_err(|error| format!("Unable to initialize text insertion: {error}"))?;
            enigo
                .text(&text)
                .map_err(|error| format!("Unable to insert dictated text: {error}"))
        })
        .await
        .map_err(|error| format!("Text insertion task stopped unexpectedly: {error}"))?
    }
}
