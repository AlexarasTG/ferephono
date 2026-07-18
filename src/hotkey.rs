use std::sync::mpsc;

use log::{debug, error};
use rdev::{EventType, Key};

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
                let mut state = RightOptionState::default();
                if let Err(error) = rdev::listen(move |event| {
                    if let Some(hotkey_event) = state.handle(event.event_type) {
                        debug!("right option transition: {hotkey_event:?}");
                        let _ = sender.send(hotkey_event);
                    }
                }) {
                    error!("global hotkey listener stopped: {error:?}");
                }
            })
            .map(|_| ())
            .map_err(|error| format!("Unable to start global hotkey listener: {error}"))
    }
}

#[derive(Default)]
struct RightOptionState {
    pressed: bool,
}

impl RightOptionState {
    fn handle(&mut self, event: EventType) -> Option<HotkeyEvent> {
        match event {
            EventType::KeyPress(Key::AltGr) if !self.pressed => {
                self.pressed = true;
                Some(HotkeyEvent::Pressed)
            }
            EventType::KeyRelease(Key::AltGr) if self.pressed => {
                self.pressed = false;
                Some(HotkeyEvent::Released)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_one_event_for_each_right_option_transition() {
        let mut state = RightOptionState::default();

        assert_eq!(
            state.handle(EventType::KeyPress(Key::AltGr)),
            Some(HotkeyEvent::Pressed)
        );
        assert_eq!(state.handle(EventType::KeyPress(Key::AltGr)), None);
        assert_eq!(
            state.handle(EventType::KeyRelease(Key::AltGr)),
            Some(HotkeyEvent::Released)
        );
        assert_eq!(state.handle(EventType::KeyRelease(Key::AltGr)), None);
    }

    #[test]
    fn ignores_left_option_and_unrelated_keys() {
        let mut state = RightOptionState::default();
        assert_eq!(state.handle(EventType::KeyPress(Key::Alt)), None);
        assert_eq!(state.handle(EventType::KeyPress(Key::KeyA)), None);
    }
}
