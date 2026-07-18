use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use core_foundation::runloop::CFRunLoop;
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CallbackResult, EventField, KeyCode,
};
use log::{debug, error, info};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotkeyEvent {
    Pressed,
    Released,
}

pub struct RightOptionHotkeyManager;

impl RightOptionHotkeyManager {
    pub fn start(sender: mpsc::Sender<HotkeyEvent>) -> Result<(), String> {
        std::thread::Builder::new()
            .name("floating-whisprer-hotkey".to_owned())
            .spawn(move || {
                let state = RightOptionState::default();
                let result = CGEventTap::with_enabled(
                    CGEventTapLocation::HID,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::ListenOnly,
                    vec![CGEventType::FlagsChanged],
                    move |_, event_type, event| {
                        match event_type {
                            CGEventType::FlagsChanged => {
                                let keycode = event
                                    .get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                                if let Some(hotkey_event) = state.handle(keycode) {
                                    debug!("right option transition: {hotkey_event:?}");
                                    let _ = sender.send(hotkey_event);
                                }
                            }
                            CGEventType::TapDisabledByTimeout
                            | CGEventType::TapDisabledByUserInput => {
                                error!("macOS hotkey event tap was disabled: {event_type:?}");
                            }
                            _ => {}
                        }
                        CallbackResult::Keep
                    },
                    || {
                        info!("native macOS hotkey event tap started");
                        CFRunLoop::run_current();
                    },
                );
                if result.is_err() {
                    error!("unable to create the macOS hotkey event tap");
                }
            })
            .map(|_| ())
            .map_err(|error| format!("Unable to start global hotkey listener: {error}"))
    }
}

#[derive(Default)]
struct RightOptionState {
    pressed: AtomicBool,
}

impl RightOptionState {
    fn handle(&self, keycode: i64) -> Option<HotkeyEvent> {
        if keycode != i64::from(KeyCode::RIGHT_OPTION) {
            return None;
        }

        if self.pressed.fetch_xor(true, Ordering::Relaxed) {
            Some(HotkeyEvent::Released)
        } else {
            Some(HotkeyEvent::Pressed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_one_event_for_each_right_option_transition() {
        let state = RightOptionState::default();

        assert_eq!(
            state.handle(i64::from(KeyCode::RIGHT_OPTION)),
            Some(HotkeyEvent::Pressed)
        );
        assert_eq!(
            state.handle(i64::from(KeyCode::RIGHT_OPTION)),
            Some(HotkeyEvent::Released)
        );
    }

    #[test]
    fn ignores_left_option_and_unrelated_keys() {
        let state = RightOptionState::default();
        assert_eq!(state.handle(0x3a), None);
        assert_eq!(state.handle(0x00), None);
    }
}
