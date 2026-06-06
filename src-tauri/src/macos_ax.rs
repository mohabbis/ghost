use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop, CFRunLoopRef};
use core_foundation::string::CFString;
use core_graphics::event::{
    CallbackResult, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType,
};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

use accessibility_sys::{kAXTrustedCheckOptionPrompt, AXIsProcessTrustedWithOptions};

/// Handle to the run loop driving the active event tap. The tap lives on its
/// own thread and is disabled when that thread's run loop stops, so to stop
/// recording we signal this run loop. `CFRunLoopStop` is thread-safe, which is
/// why we can hold the raw ref here and call it from another thread.
struct RunLoopHandle(CFRunLoopRef);
// SAFETY: the ref is only ever used to call the thread-safe CFRunLoopStop.
unsafe impl Send for RunLoopHandle {}

static RUN_LOOP: Mutex<Option<RunLoopHandle>> = Mutex::new(None);

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
    if RUN_LOOP.lock().unwrap().is_some() {
        return Err("Recording already in progress".into());
    }

    // A CGEventTap only delivers callbacks while its mach port is attached to a
    // *running* run loop. Tauri command handlers don't run on a persistent
    // run-loop thread, so spin up a dedicated thread, install the tap on its
    // run loop, and block it there until stop_event_tap() signals it.
    std::thread::spawn(move || {
        let tap = match CGEventTap::new(
            CGEventTapLocation::HID,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            vec![CGEventType::LeftMouseDown],
            move |_proxy, event_type, event| {
                if matches!(event_type, CGEventType::LeftMouseDown) {
                    let loc = event.location();
                    let _ = handle.emit("ghost:click-captured", (loc.x, loc.y));
                }
                CallbackResult::Keep
            },
        ) {
            Ok(tap) => tap,
            // Tap creation fails when Accessibility permission is missing; the
            // frontend gates on check_accessibility before calling this.
            Err(_) => return,
        };

        let current = CFRunLoop::get_current();
        let source = match tap.mach_port().create_runloop_source(0) {
            Ok(source) => source,
            Err(_) => return,
        };
        current.add_source(&source, unsafe { kCFRunLoopCommonModes });
        tap.enable();

        *RUN_LOOP.lock().unwrap() = Some(RunLoopHandle(current.as_concrete_TypeRef()));
        CFRunLoop::run_current();
        // Run loop stopped: drop the handle and let `tap` drop, disabling it.
        *RUN_LOOP.lock().unwrap() = None;
    });

    Ok(())
}

pub fn stop_event_tap() {
    if let Some(handle) = RUN_LOOP.lock().unwrap().take() {
        // Reconstruct a CFRunLoop solely to issue the thread-safe stop.
        let run_loop = unsafe { CFRunLoop::wrap_under_get_rule(handle.0) };
        run_loop.stop();
    }
}

pub fn click_at(x: f64, y: f64) -> Result<(), String> {
    use enigo::{Button, Coordinate, Enigo, Mouse, Settings};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    enigo
        .move_mouse(x as i32, y as i32, Coordinate::Abs)
        .map_err(|e| e.to_string())?;
    enigo
        .button(Button::Left, enigo::Direction::Click)
        .map_err(|e| e.to_string())?;
    Ok(())
}
