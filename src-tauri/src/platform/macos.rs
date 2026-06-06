//! macOS backend implementation using CGEventTap, AXUIElement, and enigo.

use crate::core::events::{ElementInfo, InputEvent, KeyAction};
use crate::core::traits::{ElementLocator, InputRecorder, ReplayEngine};
use enigo::{Enigo, MouseButton, MouseControllable};
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::thread;

// Core Foundation types
type CFMachPortRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CGEventRef = *mut c_void;
type CGEventTapId = u32;
type CGEventType = u32;
type CGKeyCode = u16;
type AXUIElementRef = *mut c_void;
type AXError = i32;
type AXValueRef = *mut c_void;

// CGEventType constants
const kCGMouseEventLeftMouseDown: CGEventType = 1;
const kCGMouseEventLeftMouseUp: CGEventType = 2;
const kCGMouseEventRightMouseDown: CGEventType = 3;
const kCGMouseEventRightMouseUp: CGEventType = 4;
const kCGKeyDown: CGEventType = 10;
const kCGKeyUp: CGEventType = 11;
const kCGScrollWheelEvent: CGEventType = 22;

// AX constants
const kAXErrorSuccess: AXError = 0;
const kAXRoleAttribute: &str = "AXRole";
const kAXTitleAttribute: &str = "AXTitle";
const kAXValueAttribute: &str = "AXValue";
const kAXApplicationAttribute: &str = "AXApplication";

// External C functions (Core Graphics)
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap_place: CGEventTapId,
        events_of_interest: u64,
        handler: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CFMachPortGetRunLoopSource(port: CFMachPortRef) -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopRef, mode: CFStringRef);
    fn CFRunLoopRun();
    fn CFRunLoopStop(rl: CFRunLoopRef);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: CGEventField) -> i64;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: CGEventField) -> i64;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: CGEventField) -> i64;
}

// We need to define the callback type properly
type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapId,
    etype: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

type CGEventField = u32;
type CFStringRef = *const c_void;

// Accessibility external functions
#[link(name = "HIServices", kind = "framework")]
extern "C" {
    fn AXUIElementCopyElementAtPosition(
        application: AXUIElementRef,
        x: f32,
        y: f32,
        element: *mut AXUIElementRef,
    ) -> AXError;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut AXValueRef,
    ) -> AXError;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXValueGetValue(value: AXValueRef, ty: u32, ptr: *mut c_void) -> bool;
    fn CFGetTypeID(cf: *const c_void) -> usize;
    fn CFStringGetCStringPtr(theString: CFStringRef, encoding: u32) -> *const c_char;
    fn CFRelease(cf: *const c_void);
}

/// macOS-specific backend providing recorder, locator, and replayer implementations.
pub struct MacosBackend;

// Thread-local storage for the run loop reference
struct TapState {
    run_loop: Option<CFRunLoopRef>,
    is_running: Arc<AtomicBool>,
}

impl MacosBackend {
    pub fn new() -> Self {
        MacosBackend
    }

    /// Returns a boxed input recorder for macOS.
    pub fn recorder() -> Box<dyn InputRecorder> {
        Box::new(MacosRecorder::new())
    }

    /// Returns a boxed element locator for macOS.
    pub fn locator() -> Box<dyn ElementLocator> {
        Box::new(MacosLocator)
    }

    /// Returns a boxed replay engine for macOS.
    pub fn replayer() -> Box<dyn ReplayEngine> {
        Box::new(MacosReplayer)
    }
}

/// macOS event recorder using CGEventTap.
struct MacosRecorder {
    state: Arc<Mutex<Option<TapState>>>,
}

impl MacosRecorder {
    fn new() -> Self {
        MacosRecorder {
            state: Arc::new(Mutex::new(None)),
        }
    }
}

unsafe impl Send for MacosRecorder {}
unsafe impl Sync for MacosRecorder {}

impl InputRecorder for MacosRecorder {
    fn start(&self, tx: mpsc::Sender<InputEvent>) -> anyhow::Result<()> {
        let state_clone = self.state.clone();
        let is_running = Arc::new(AtomicBool::new(true));
        
        thread::spawn(move || {
            // Event mask for mouse and keyboard events
            let event_mask: u64 = (1 << kCGMouseEventLeftMouseDown)
                | (1 << kCGMouseEventLeftMouseUp)
                | (1 << kCGMouseEventRightMouseDown)
                | (1 << kCGMouseEventRightMouseUp)
                | (1 << kCGKeyDown)
                | (1 << kCGKeyUp)
                | (1 << kCGScrollWheelEvent);

            // Create the event tap
            unsafe {
                let tap = CGEventTapCreate(
                    0, // kCGSessionEventTap
                    event_mask,
                    kCGEventTapOptionDefault,
                    cg_event_callback,
                    Box::into_raw(Box::new(tx)) as *mut c_void,
                );

                if tap.is_null() {
                    eprintln!("Failed to create CGEventTap");
                    return;
                }

                let run_loop_source = CFMachPortGetRunLoopSource(tap);
                let current_run_loop = CFRunLoopGetCurrent();
                CFRunLoopAddSource(
                    current_run_loop,
                    run_loop_source,
                    kCFRunLoopCommonModes,
                );

                *state_clone.lock().unwrap() = Some(TapState {
                    run_loop: Some(current_run_loop),
                    is_running: is_running.clone(),
                });

                // Run the run loop
                CFRunLoopRun();
            }
        });

        Ok(())
    }

    fn stop(&self) {
        if let Some(mut state) = self.state.lock().unwrap().take() {
            state.is_running.store(false, Ordering::Relaxed);
            if let Some(rl) = state.run_loop {
                unsafe {
                    CFRunLoopStop(rl);
                }
            }
        }
    }
}

// Callback constants
const kCGEventTapOptionDefault: u32 = 0;
const kCFRunLoopCommonModes: CFStringRef = std::ptr::null();

// External function for getting current run loop
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
}

unsafe extern "C" fn cg_event_callback(
    _proxy: CGEventTapId,
    etype: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    let tx_ptr = user_info as *mut mpsc::Sender<InputEvent>;
    let tx = &*tx_ptr;

    let input_event = match etype {
        kCGMouseEventLeftMouseDown | kCGMouseEventLeftMouseUp => {
            let x = CGEventGetIntegerValueField(event, 0) as i32; // kCGMouseEventDeltaX
            let y = CGEventGetIntegerValueField(event, 1) as i32; // kCGMouseEventDeltaY
            let button = if etype == kCGMouseEventLeftMouseDown { 0 } else { 1 };
            InputEvent::MouseClick {
                x,
                y,
                button,
                element: None, // Will be populated by element locator if needed
            }
        }
        kCGMouseEventRightMouseDown | kCGMouseEventRightMouseUp => {
            let x = CGEventGetIntegerValueField(event, 0) as i32;
            let y = CGEventGetIntegerValueField(event, 1) as i32;
            let button = if etype == kCGMouseEventRightMouseDown { 2 } else { 3 };
            InputEvent::MouseClick {
                x,
                y,
                button,
                element: None,
            }
        }
        kCGKeyDown | kCGKeyUp => {
            let code = CGEventGetIntegerValueField(event, 0) as u16; // kCGKeyboardEventKeycode
            let action = if etype == kCGKeyDown {
                KeyAction::Down
            } else {
                KeyAction::Up
            };
            InputEvent::Key {
                code,
                chars: String::new(), // TODO: Get actual characters from event
                modifiers: 0,         // TODO: Extract modifier flags
                action,
            }
        }
        kCGScrollWheelEvent => {
            let dx = CGEventGetIntegerValueField(event, 0) as i32;
            let dy = CGEventGetIntegerValueField(event, 1) as i32;
            InputEvent::Scroll {
                dx,
                dy,
                phase: 0, // TODO: Extract scroll phase
            }
        }
        _ => return event, // Pass through unhandled events
    };

    let _ = tx.send(input_event);
    event
}

/// macOS element locator using AXUIElement / Accessibility API.
struct MacosLocator;

impl ElementLocator for MacosLocator {
    fn inspect_at(&self, x: i32, y: i32) -> anyhow::Result<Option<ElementInfo>> {
        unsafe {
            // Get system-wide accessibility element
            let system_wide = AXUIElementCreateSystemWide();
            
            let mut element: AXUIElementRef = std::ptr::null_mut();
            let result = AXUIElementCopyElementAtPosition(
                system_wide,
                x as f32,
                y as f32,
                &mut element,
            );

            if result != kAXErrorSuccess || element.is_null() {
                return Ok(None);
            }

            // Extract role
            let role = get_ax_string_attribute(element, kAXRoleAttribute);
            
            // Extract name/title
            let name = get_ax_string_attribute(element, kAXTitleAttribute)
                .or_else(|| get_ax_string_attribute(element, kAXValueAttribute))
                .unwrap_or_default();

            // Extract application
            let app = get_ax_string_attribute(element, kAXApplicationAttribute)
                .unwrap_or_else(|| String::from("Unknown"));

            CFRelease(element as *const c_void);

            Ok(Some(ElementInfo {
                role,
                name,
                app,
                fallback_coords: Some((x, y)),
            }))
        }
    }
}

/// Helper function to extract string attributes from AXUIElement
unsafe fn get_ax_string_attribute(element: AXUIElementRef, attribute: &str) -> Option<String> {
    use std::ffi::CStr;
    
    let cf_string = attribute.to_cfstring();
    let mut value: AXValueRef = std::ptr::null_mut();
    
    if AXUIElementCopyAttributeValue(element, cf_string as CFStringRef, &mut value) != kAXErrorSuccess {
        return None;
    }

    if value.is_null() {
        return None;
    }

    // Try to get as CFString
    let type_id = CFGetTypeID(value as *const c_void);
    let cf_string_type_id = CFGetTypeID(std::ptr::null()); // This is a simplification
    
    // For now, try direct C string extraction
    let c_str = CFStringGetCStringPtr(value as CFStringRef, 0x08000100); // kCFStringEncodingUTF8
    if !c_str.is_null() {
        let rust_str = CStr::from_ptr(c_str).to_string_lossy().into_owned();
        CFRelease(value as *const c_void);
        return Some(rust_str);
    }

    CFRelease(value as *const c_void);
    None
}

// Helper trait for converting &str to CFStringRef
trait ToCFString {
    fn to_cfstring(&self) -> *const c_void;
}

impl ToCFString for &str {
    fn to_cfstring(&self) -> *const c_void {
        // Simplified - in production would use proper CFStringCreateWithBytes
        std::ptr::null()
    }
}

/// macOS replay engine using enigo.
struct MacosReplayer;

impl ReplayEngine for MacosReplayer {
    fn execute(&self, events: &[InputEvent], stop_flag: Arc<AtomicBool>) -> anyhow::Result<()> {
        let mut enigo = Enigo::new();

        for event in events {
            if stop_flag.load(Ordering::Relaxed) {
                return Ok(());
            }

            match event {
                InputEvent::MouseClick { x, y, button, .. } => {
                    // Move to position
                    enigo.mouse_move_to(*x, *y);
                    
                    // Click the appropriate button
                    let mouse_button = match button {
                        0 | 1 => MouseButton::Left,
                        2 | 3 => MouseButton::Right,
                        _ => MouseButton::Left,
                    };
                    
                    enigo.mouse_click(mouse_button);
                }
                InputEvent::Key { code, chars, action, .. } => {
                    match action {
                        KeyAction::Down => {
                            // Try using scancode first, fall back to character
                            if !chars.is_empty() {
                                enigo.key_down(enigo::Key::Layout(chars.chars().next().unwrap_or(' ')));
                            } else {
                                enigo.key_down(enigo::Key::Raw(*code));
                            }
                        }
                        KeyAction::Up => {
                            if !chars.is_empty() {
                                enigo.key_up(enigo::Key::Layout(chars.chars().next().unwrap_or(' ')));
                            } else {
                                enigo.key_up(enigo::Key::Raw(*code));
                            }
                        }
                    }
                }
                InputEvent::Scroll { dx, dy, .. } => {
                    enigo.scroll(*dx, *dy);
                }
                InputEvent::Delay { ms } => {
                    std::thread::sleep(std::time::Duration::from_millis(*ms));
                }
            }
        }

        Ok(())
    }
}
