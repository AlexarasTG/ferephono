use async_trait::async_trait;

use crate::TextInserter;

pub struct DirectTextInserter;

#[async_trait]
impl TextInserter for DirectTextInserter {
    async fn insert(&self, text: &str) -> Result<(), String> {
        let text = text.to_owned();
        tokio::task::spawn_blocking(move || platform::insert(&text))
            .await
            .map_err(|error| format!("Text insertion task stopped unexpectedly: {error}"))?
    }
}

// Pasting instead of typing: Herdr intermittently drops bursts of synthetic keystrokes.
#[cfg(target_os = "macos")]
mod platform {
    use std::thread::sleep;
    use std::time::Duration;

    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    use objc2::Message;
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardType, NSPasteboardTypeString};
    use objc2_foundation::{NSArray, NSData, NSString};

    const RESTORE_DELAY: Duration = Duration::from_millis(500);
    const TRANSIENT_TYPE: &str = "org.nspasteboard.TransientType";

    type Item = Vec<(Retained<NSPasteboardType>, Retained<NSData>)>;

    pub(super) fn insert(text: &str) -> Result<(), String> {
        let pasteboard = NSPasteboard::generalPasteboard();
        let saved = snapshot(&pasteboard);
        let transcript = vec![
            (
                unsafe { NSPasteboardTypeString }.retain(),
                NSData::with_bytes(text.as_bytes()),
            ),
            (NSString::from_str(TRANSIENT_TYPE), NSData::new()),
        ];
        write(&pasteboard, &[transcript])?;
        let transcript_change = pasteboard.changeCount();
        let pasted = press_paste();
        sleep(RESTORE_DELAY);
        if pasteboard.changeCount() == transcript_change {
            write(&pasteboard, &saved)?;
        }
        pasted
    }

    fn snapshot(pasteboard: &NSPasteboard) -> Vec<Item> {
        let Some(items) = pasteboard.pasteboardItems() else {
            return Vec::new();
        };
        items
            .iter()
            .map(|item| {
                item.types()
                    .iter()
                    .filter_map(|data_type| item.dataForType(&data_type).map(|d| (data_type, d)))
                    .collect()
            })
            .collect()
    }

    fn write(pasteboard: &NSPasteboard, items: &[Item]) -> Result<(), String> {
        pasteboard.clearContents();
        if items.is_empty() {
            return Ok(());
        }
        let items: Vec<_> = items
            .iter()
            .map(|entries| {
                let item = NSPasteboardItem::new();
                for (data_type, data) in entries {
                    item.setData_forType(data, data_type);
                }
                ProtocolObject::from_retained(item)
            })
            .collect();
        if pasteboard.writeObjects(&NSArray::from_retained_slice(&items)) {
            Ok(())
        } else {
            Err("Unable to write to the clipboard".to_owned())
        }
    }

    fn press_paste() -> Result<(), String> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|error| format!("Unable to initialize text insertion: {error}"))?;
        let error = |error: enigo::InputError| format!("Unable to paste dictated text: {error}");
        enigo.key(Key::Meta, Direction::Press).map_err(error)?;
        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(error)?;
        enigo.key(Key::Meta, Direction::Release).map_err(error)
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use enigo::{Enigo, Keyboard, Settings};

    pub(super) fn insert(text: &str) -> Result<(), String> {
        Enigo::new(&Settings::default())
            .map_err(|error| format!("Unable to initialize text insertion: {error}"))?
            .text(text)
            .map_err(|error| format!("Unable to insert dictated text: {error}"))
    }
}
