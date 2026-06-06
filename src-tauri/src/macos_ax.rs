use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_graphics::event::{
    CallbackResult, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

use accessibility_sys::{kAXTrustedCheckOptionPrompt, AXIsProcessTrustedWithOptions};

struct TapHolder(Option<CGEventTap<'static>>);
unsafe impl Send for TapHolder {}
unsafe impl Sync for TapHolder {}

static EVENT_TAP: Mutex<TapHolder> = Mutex::new(TapHolder(None));
static mut APP_HANDLE: Option<AppHandle> = None;

pub fn check_permissions() -> bool {
    unsafe {
        let options = CFDictionary::from_CFType_pairs(&[(
            CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt).as_CFType(),
            CFBoolean::false_value().as_CFType(),
        )]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef())
    }
}

pub fn request_permissions() -> bool {
    unsafe {
        let options = CFDictionary::from_CFType_pairs(&[(
            CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt).as_CFType(),
            CFBoolean::true_value().as_CFType(),
        )]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef())
    }
}

pub fn start_event_tap(handle: AppHandle) -> Result<(), String> {
    unsafe { APP_HANDLE = Some(handle); }

    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::LeftMouseDown],
        |_proxy, event_type, event| {
            match event_type {
                CGEventType::LeftMouseDown => {
                    let loc = event.location();
                    unsafe {
                        if let Some(h) = &APP_HANDLE {
                            let _ = h.emit("ghost:click-captured", (loc.x, loc.y));
                        }
                    }
                }
                _ => {}
            }
            CallbackResult::Keep
        },
    )
    .map_err(|_| "Failed to create CGEventTap. Grant Accessibility permissions first.".to_string())?;

    tap.enable();

    let mut holder = EVENT_TAP.lock().unwrap();
    holder.0 = Some(tap);
    Ok(())
}

pub fn stop_event_tap() {
    let mut holder = EVENT_TAP.lock().unwrap();
    holder.0 = None;
}

pub fn click_at(x: f64, y: f64) -> Result<(), String> {
    use enigo::{Button, Coordinate, Enigo, Mouse, Settings};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo.move_mouse(x as i32, y as i32, Coordinate::Abs).map_err(|e| e.to_string())?;
    enigo.button(Button::Left, enigo::Direction::Click).map_err(|e| e.to_string())?;
    Ok(())
}
