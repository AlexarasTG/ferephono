use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, mpsc};

use core_foundation::base::TCFType;
use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
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
                let event_tap_handle = Arc::new(AtomicPtr::<c_void>::new(std::ptr::null_mut()));
                let callback_event_tap_handle = event_tap_handle.clone();
                let event_tap = CGEventTap::new(
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
                                let event_tap_handle =
                                    callback_event_tap_handle.load(Ordering::Acquire);
                                if event_tap_handle.is_null() {
                                    error!(
                                        "macOS hotkey event tap was disabled before its handle was available: {event_type:?}"
                                    );
                                } else {
                                    unsafe { CGEventTapEnable(event_tap_handle, true) };
                                    info!(
                                        "re-enabled macOS hotkey event tap after: {event_type:?}"
                                    );
                                }
                            }
                            _ => {}
                        }
                        CallbackResult::Keep
                    },
                );
                let Ok(event_tap) = event_tap else {
                    error!("unable to create the macOS hotkey event tap");
                    return;
                };
                event_tap_handle.store(
                    event_tap.mach_port().as_concrete_TypeRef().cast(),
                    Ordering::Release,
                );
                let Ok(run_loop_source) = event_tap.mach_port().create_runloop_source(0) else {
                    error!("unable to create the macOS hotkey run-loop source");
                    return;
                };
                CFRunLoop::get_current()
                    .add_source(&run_loop_source, unsafe { kCFRunLoopCommonModes });
                event_tap.enable();
                info!("native macOS hotkey event tap started");
                CFRunLoop::run_current();
            })
            .map(|_| ())
            .map_err(|error| format!("Unable to start global hotkey listener: {error}"))
    }
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
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
