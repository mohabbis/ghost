//! macOS backend implementation using CGEventTap, AXUIElement, and enigo.
#![allow(non_upper_case_globals)]

use crate::core::events::{ElementInfo, InputEvent, KeyAction};
use crate::core::replay_support::{
    self, check_continue, interruptible_sleep, pacing_gap_ms, ReplayProgress,
};
use crate::core::traits::{ElementLocator, InputRecorder, ReplayEngine};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Core Foundation types
type CFMachPortRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CGEventRef = *mut c_void;
type CGEventTapId = u32;
type CGEventType = u32;
type AXUIElementRef = *mut c_void;
type AXError = i32;
type CFTypeRef = *const c_void;
type CFAllocatorRef = *const c_void;
type CFStringEncoding = u32;
type Boolean = u8;

// CGEventType constants
const kCGMouseEventLeftMouseDown: CGEventType = 1;
const kCGMouseEventLeftMouseUp: CGEventType = 2;
const kCGMouseEventRightMouseDown: CGEventType = 3;
const kCGMouseEventRightMouseUp: CGEventType = 4;
const kCGKeyDown: CGEventType = 10;
const kCGKeyUp: CGEventType = 11;
const kCGScrollWheelEvent: CGEventType = 22;

// CGEventField constants (CGEventTypes.h — do NOT guess these; wrong fields
// silently return garbage). Mouse coordinates are not integer fields at all:
// they come from CGEventGetLocation().
const kCGKeyboardEventKeycode: u32 = 9;
const kCGScrollWheelEventDeltaAxis1: u32 = 11;
const kCGScrollWheelEventDeltaAxis2: u32 = 12;
const kCGScrollWheelEventScrollPhase: u32 = 99;
const kCGScrollWheelEventMomentumPhase: u32 = 123;

/// CGPoint for CGEventGetLocation.
#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

// CGEventFlags modifier mask constants
const kCGEventFlagMaskCapsLock: u64 = 0x0001_0000;
const kCGEventFlagMaskShift: u64 = 0x0002_0000;
const kCGEventFlagMaskControl: u64 = 0x0004_0000;
const kCGEventFlagMaskAlternate: u64 = 0x0008_0000;
const kCGEventFlagMaskCommand: u64 = 0x0010_0000;

// AX constants
const kAXErrorSuccess: AXError = 0;
const kAXRoleAttribute: &str = "AXRole";
const kAXTitleAttribute: &str = "AXTitle";
const kAXValueAttribute: &str = "AXValue";
const kAXDescriptionAttribute: &str = "AXDescription";
const kAXIdentifierAttribute: &str = "AXIdentifier";
const kAXRoleDescriptionAttribute: &str = "AXRoleDescription";
const kAXWindowAttribute: &str = "AXWindow";
const kAXWindowsAttribute: &str = "AXWindows";
const kAXPositionAttribute: &str = "AXPosition";

// AXValue wrapper types (AXValue.h) — AXPosition/AXSize come back as AXValue
// boxes, not CFStrings, and must be unpacked with AXValueGetValue.
const kAXValueCGPointType: u32 = 1;

// External C functions (Core Graphics)
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap_place: CGEventTapId,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CFRunLoopRun();
    fn CFRunLoopStop(rl: CFRunLoopRef);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventKeyboardGetUnicodeString(
        event: CGEventRef,
        max_string_length: usize,
        actual_string_length: *mut usize,
        unicode_string: *mut u16,
    );
    fn CGEventTapEnable(tap: CFMachPortRef, enable: Boolean);
}

type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapId,
    etype: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

type CFStringRef = *const c_void;

// Accessibility external functions
#[link(name = "ApplicationServices", kind = "framework")]
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
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
    fn AXValueGetValue(value: CFTypeRef, value_type: u32, value_ptr: *mut c_void) -> Boolean;
}

// Core Foundation external functions
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: isize,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFStringCreateWithBytes(
        alloc: CFAllocatorRef,
        bytes: *const u8,
        num_bytes: isize,
        encoding: CFStringEncoding,
        is_external_representation: Boolean,
    ) -> CFStringRef;
    fn CFStringGetLength(theString: CFStringRef) -> isize;
    fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: CFStringEncoding) -> isize;
    fn CFStringGetCString(
        theString: CFStringRef,
        buffer: *mut c_char,
        buffer_size: isize,
        encoding: CFStringEncoding,
    ) -> Boolean;
    fn CFStringGetCStringPtr(theString: CFStringRef, encoding: CFStringEncoding) -> *const c_char;
    fn CFRelease(cf: CFTypeRef);
    fn CFArrayGetCount(theArray: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(theArray: CFTypeRef, idx: isize) -> *const c_void;

    static kCFRunLoopCommonModes: CFStringRef;
}

// libproc (part of libSystem, linked into every binary — no framework link
// needed): permission-free process enumeration, used to find the pid owning
// a recorded window so its AXWindows can be walked under the Accessibility
// permission replay already requires. Deliberately NOT CGWindowList, which
// would demand the Screen Recording permission.
extern "C" {
    fn proc_listallpids(buffer: *mut c_void, buffersize: c_int) -> c_int;
    fn proc_name(pid: c_int, buffer: *mut c_void, buffersize: u32) -> c_int;
}

// IOKit HID access (Input Monitoring permission — required for keyboard
// capture via event taps since macOS 10.15, separate from Accessibility).
#[link(name = "IOKit", kind = "framework")]
extern "C" {
    /// Returns kIOHIDAccessTypeGranted (0), Denied (1), or Unknown (2).
    fn IOHIDCheckAccess(request_type: u32) -> u32;
    /// Prompts the user (once) and returns true if access is granted.
    fn IOHIDRequestAccess(request_type: u32) -> bool;
}

const kIOHIDRequestTypeListenEvent: u32 = 1;
const kIOHIDAccessTypeGranted: u32 = 0;

const kCFStringEncodingUTF8: CFStringEncoding = 0x08000100;
const kCGSessionEventTap: CGEventTapId = 0;
const kCGHeadInsertEventTap: u32 = 0;
// Listen-only tap: does not block or filter events, safe for slow callbacks
const kCGEventTapOptionListenOnly: u32 = 1;

/// macOS-specific backend providing recorder, locator, and replayer implementations.
pub struct MacosBackend;

struct TapState {
    run_loop: Option<CFRunLoopRef>,
    #[allow(dead_code)]
    tap_port: Option<CFMachPortRef>,
    is_running: Arc<AtomicBool>,
}

unsafe impl Send for TapState {}
unsafe impl Sync for TapState {}

impl MacosBackend {
    #[allow(dead_code)]
    pub fn new() -> Self {
        MacosBackend
    }

    pub fn recorder() -> Box<dyn InputRecorder> {
        Box::new(MacosRecorder::new())
    }

    pub fn locator() -> Box<dyn ElementLocator> {
        Box::new(MacosLocator)
    }

    pub fn replayer() -> Box<dyn ReplayEngine> {
        Box::new(MacosReplayer)
    }

    /// Check if accessibility permissions are granted, without prompting.
    pub fn check_accessibility() -> bool {
        unsafe { accessibility_sys::AXIsProcessTrusted() }
    }

    /// Request accessibility permissions, surfacing the system "Ghost would
    /// like to control this computer" prompt if not already granted.
    ///
    /// macOS only shows that prompt ONCE per app — every later call silently
    /// returns false. So when we're still untrusted after the call, open
    /// System Settings → Privacy & Security → Accessibility directly so the
    /// button always visibly does something.
    pub fn request_accessibility() -> bool {
        use accessibility_sys::kAXTrustedCheckOptionPrompt;
        use core_foundation::base::TCFType;
        use core_foundation::boolean::CFBoolean;
        use core_foundation::dictionary::CFDictionary;
        use core_foundation::string::CFString;

        let trusted = unsafe {
            let prompt_key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let options = CFDictionary::from_CFType_pairs(&[(
                prompt_key.as_CFType(),
                CFBoolean::true_value().as_CFType(),
            )]);
            accessibility_sys::AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef())
        };

        if !trusted {
            Self::open_privacy_pane("Privacy_Accessibility");
        }
        trusted
    }

    /// Check Input Monitoring permission (needed to capture keystrokes).
    pub fn check_input_monitoring() -> bool {
        unsafe { IOHIDCheckAccess(kIOHIDRequestTypeListenEvent) == kIOHIDAccessTypeGranted }
    }

    /// Request Input Monitoring permission; prompts once, then falls back to
    /// opening the System Settings pane (same one-shot prompt behavior as
    /// Accessibility).
    pub fn request_input_monitoring() -> bool {
        let granted = unsafe { IOHIDRequestAccess(kIOHIDRequestTypeListenEvent) };
        if !granted {
            Self::open_privacy_pane("Privacy_ListenEvent");
        }
        granted
    }

    /// Open System Settings → Privacy & Security at the given anchor.
    fn open_privacy_pane(anchor: &str) {
        let url = format!(
            "x-apple.systempreferences:com.apple.preference.security?{}",
            anchor
        );
        if let Err(e) = std::process::Command::new("open").arg(&url).spawn() {
            eprintln!("Failed to open System Settings ({}): {}", anchor, e);
        }
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
            let event_mask: u64 = (1 << kCGMouseEventLeftMouseDown)
                | (1 << kCGMouseEventLeftMouseUp)
                | (1 << kCGMouseEventRightMouseDown)
                | (1 << kCGMouseEventRightMouseUp)
                | (1 << kCGKeyDown)
                | (1 << kCGKeyUp)
                | (1 << kCGScrollWheelEvent);

            unsafe {
                let tap = CGEventTapCreate(
                    kCGSessionEventTap,
                    kCGHeadInsertEventTap,
                    kCGEventTapOptionListenOnly,
                    event_mask,
                    cg_event_callback,
                    Box::into_raw(Box::new(tx)) as *mut c_void,
                );

                if tap.is_null() {
                    eprintln!(
                        "Failed to create CGEventTap - Accessibility permissions may be required"
                    );
                    return;
                }

                let run_loop_source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
                let current_run_loop = CFRunLoopGetCurrent();
                CFRunLoopAddSource(current_run_loop, run_loop_source, kCFRunLoopCommonModes);

                CGEventTapEnable(tap, 1);

                *state_clone.lock().unwrap() = Some(TapState {
                    run_loop: Some(current_run_loop),
                    tap_port: Some(tap),
                    is_running: is_running.clone(),
                });

                CFRunLoopRun();
            }
        });

        Ok(())
    }

    fn stop(&self) {
        if let Some(state) = self.state.lock().unwrap().take() {
            state.is_running.store(false, Ordering::Relaxed);
            if let Some(rl) = state.run_loop {
                unsafe {
                    CFRunLoopStop(rl);
                }
            }
        }
    }
}

// ── Helpers called from the event tap callback ────────────────────────────────

/// Pack CGEventFlags into a compact u8 modifier bitmask.
/// Bit layout: 0=Shift 1=Control 2=Alt/Option 3=Command 4=CapsLock
unsafe fn extract_modifiers(flags: u64) -> u8 {
    let mut m: u8 = 0;
    if flags & kCGEventFlagMaskShift != 0 {
        m |= 0x01;
    }
    if flags & kCGEventFlagMaskControl != 0 {
        m |= 0x02;
    }
    if flags & kCGEventFlagMaskAlternate != 0 {
        m |= 0x04;
    }
    if flags & kCGEventFlagMaskCommand != 0 {
        m |= 0x08;
    }
    if flags & kCGEventFlagMaskCapsLock != 0 {
        m |= 0x10;
    }
    m
}

/// Extract the Unicode string produced by a keyboard event.
///
/// The 32-unit buffer comfortably covers any single keystroke — ordinary
/// keys produce 1–2 UTF-16 units and even IME/dead-key composition bursts
/// stay in single digits. Overflow is never silent: a recorder that quietly
/// mangles a keystroke is worse than one that says so, so truncation is
/// logged, and the cut never splits a surrogate pair (no manufactured
/// U+FFFD replacement characters).
unsafe fn extract_key_chars(event: CGEventRef) -> String {
    let mut actual_len: usize = 0;
    let mut buf = [0u16; 32];
    CGEventKeyboardGetUnicodeString(event, buf.len(), &mut actual_len, buf.as_mut_ptr());
    if actual_len == 0 {
        return String::new();
    }
    let safe_len = utf16_capture_len(&buf, actual_len);
    if actual_len > buf.len() {
        tracing::warn!(
            "keyboard event produced {} UTF-16 units; captured the first {} — \
             this recording may be missing typed characters",
            actual_len,
            safe_len
        );
    }
    String::from_utf16_lossy(&buf[..safe_len])
}

/// Clamp a reported UTF-16 length to what the capture buffer actually holds,
/// without splitting a surrogate pair: if a truncated prefix would end on an
/// unpaired high surrogate, drop that unit rather than emit U+FFFD for half
/// a character.
fn utf16_capture_len(buf: &[u16], actual_len: usize) -> usize {
    let mut len = actual_len.min(buf.len());
    if actual_len > buf.len() && len > 0 && (0xD800..0xDC00).contains(&buf[len - 1]) {
        len -= 1;
    }
    len
}

// ── CGEventTap callback ───────────────────────────────────────────────────────

/// Map a captured mouse-click event into the recorded `InputEvent`, applying
/// the secure-field suppression uniformly to every press AND release. Pure
/// (no FFI) so tests cover the actual wiring — that each of the four click
/// types passes the secure gate and maps to the right button code — not just
/// the `is_secure_click` predicate in isolation.
fn capture_click(
    etype: CGEventType,
    x: i32,
    y: i32,
    element: Option<ElementInfo>,
) -> Option<InputEvent> {
    if is_secure_click(&element) {
        return None;
    }
    // Presses retain the element descriptor for replay re-resolution;
    // releases replay at recorded coordinates and carry none.
    let (button, keeps_element) = match etype {
        kCGMouseEventLeftMouseDown => (0, true),
        kCGMouseEventLeftMouseUp => (1, false),
        kCGMouseEventRightMouseDown => (2, true),
        kCGMouseEventRightMouseUp => (3, false),
        _ => return None,
    };
    Some(InputEvent::MouseClick {
        x,
        y,
        button,
        element: if keeps_element { element } else { None },
        timestamp: None,
        retry_count: None,
        semantic_tag: None,
        self_heal: None,
    })
}

/// Safe wrapper for event processing that returns Result instead of panicking.
/// This is called from cg_event_callback which cannot unwind across FFI boundary.
unsafe fn process_cg_event(etype: CGEventType, event: CGEventRef) -> Option<InputEvent> {
    match etype {
        kCGMouseEventLeftMouseDown
        | kCGMouseEventLeftMouseUp
        | kCGMouseEventRightMouseDown
        | kCGMouseEventRightMouseUp => {
            let loc = CGEventGetLocation(event);
            let (x, y) = (loc.x as i32, loc.y as i32);
            // The AX lookup runs for every press and release so the secure
            // gate inside capture_click always sees the element under the
            // cursor — a release over a password field must not leak its
            // coordinates any more than a press.
            capture_click(etype, x, y, ax_info_at(x, y))
        }
        kCGKeyDown | kCGKeyUp => {
            let code = CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode) as u16;
            let flags = CGEventGetFlags(event);
            let modifiers = extract_modifiers(flags);
            // Only extract characters on key-down; up events carry the same string redundantly
            let chars = if etype == kCGKeyDown {
                extract_key_chars(event)
            } else {
                String::new()
            };
            let action = if etype == kCGKeyDown {
                KeyAction::Down
            } else {
                KeyAction::Up
            };
            Some(InputEvent::Key {
                code,
                chars,
                modifiers,
                action,
                timestamp: None,
                retry_count: None,
                semantic_tag: None,
            })
        }
        kCGScrollWheelEvent => {
            let dx = CGEventGetIntegerValueField(event, kCGScrollWheelEventDeltaAxis2) as i32;
            let dy = CGEventGetIntegerValueField(event, kCGScrollWheelEventDeltaAxis1) as i32;
            // kCGScrollWheelEventScrollPhase: 0=none 1=began 2=changed 4=ended 128=mayBegin
            // kCGScrollWheelEventMomentumPhase: 0=none 1=begin 2=continue 3=end
            let scroll_phase =
                CGEventGetIntegerValueField(event, kCGScrollWheelEventScrollPhase) as u8;
            let momentum_phase =
                CGEventGetIntegerValueField(event, kCGScrollWheelEventMomentumPhase) as u8;
            // Prefer gesture phase; fall back to momentum phase for coasting scrolls
            let phase = if scroll_phase != 0 {
                scroll_phase
            } else {
                momentum_phase
            };
            Some(InputEvent::Scroll {
                dx,
                dy,
                phase,
                timestamp: None,
            })
        }
        _ => None,
    }
}

unsafe extern "C" fn cg_event_callback(
    _proxy: CGEventTapId,
    etype: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    // Defensive: if user_info is null, just return the event unchanged
    if user_info.is_null() {
        return event;
    }

    // Defensive: if event is null, return it unchanged (shouldn't happen but be safe)
    if event.is_null() {
        return event;
    }

    // Cast user_info to the sender. We've already checked it's not null.
    let tx = &*(user_info as *mut mpsc::Sender<InputEvent>);

    // TRIPWIRE: the catch_unwind below only works under the default "unwind"
    // panic strategy. Setting `panic = "abort"` in a profile — a routine
    // binary-size optimization — would silently turn every callback panic
    // into an instant process abort with the OS event tap still installed.
    // Fail the build instead of failing in the field:
    #[cfg(panic = "abort")]
    compile_error!(
        "cg_event_callback relies on std::panic::catch_unwind to contain panics at the \
         CGEventTap FFI boundary; the abort panic strategy disables catch_unwind. Remove \
         `panic = \"abort\"` from the profile, or redesign the callback's panic containment \
         before shipping with it."
    );

    // Wrap everything in catch_unwind to prevent panics from escaping the FFI boundary.
    // A panic in an extern "C" function causes abort, so we must catch any potential panic.
    let result = std::panic::catch_unwind(|| unsafe { process_cg_event(etype, event) });

    match result {
        Ok(Some(input_event)) => {
            // Send the event, but ignore send errors (receiver may have closed)
            let _ = tx.send(input_event);
        }
        Ok(None) => {
            // Event was filtered out (e.g., secure field) or unknown type
            // Just pass it through unchanged
        }
        Err(e) => {
            // A panic occurred inside the callback logic. Log it and continue.
            // This should never happen, but if it does, we don't want to crash the app.
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic".to_string()
            };
            eprintln!("PANIC in cg_event_callback (ignored): {}", msg);
            // Optionally log with tracing if available
            // tracing::error!("PANIC in cg_event_callback: {}", msg);
        }
    }

    // Always return the original event (listen-only tap, we don't modify events)
    event
}

// ── AX string attribute helper ────────────────────────────────────────────────

/// macOS element locator using AXUIElement / Accessibility API.
struct MacosLocator;

impl ElementLocator for MacosLocator {
    fn inspect_at(&self, x: i32, y: i32) -> anyhow::Result<Option<ElementInfo>> {
        Ok(unsafe { ax_info_at(x, y) })
    }
}

/// Resolve the accessibility element at screen point (x, y) into an `ElementInfo`.
/// Shared by the recorder (to tag captured clicks), the element inspector, and replay
/// descriptor re-resolution.
unsafe fn ax_info_at(x: i32, y: i32) -> Option<ElementInfo> {
    let system_wide = AXUIElementCreateSystemWide();
    if system_wide.is_null() {
        return None;
    }

    let mut element: AXUIElementRef = std::ptr::null_mut();
    let result = AXUIElementCopyElementAtPosition(system_wide, x as f32, y as f32, &mut element);

    if result != kAXErrorSuccess || element.is_null() {
        CFRelease(system_wide as *const c_void);
        return None;
    }

    let role = get_ax_string_attribute(element, kAXRoleAttribute).unwrap_or_default();
    let title = get_ax_string_attribute(element, kAXTitleAttribute);
    let description = get_ax_string_attribute(element, kAXDescriptionAttribute);
    let value = get_ax_string_attribute(element, kAXValueAttribute);
    let identifier = get_ax_string_attribute(element, kAXIdentifierAttribute);
    let role_description = get_ax_string_attribute(element, kAXRoleDescriptionAttribute);
    let name = title
        .clone()
        .or_else(|| description.clone())
        .or_else(|| value.clone())
        .unwrap_or_default();

    // Resolve app name via PID → AXUIElementCreateApplication → AXTitle
    let app = {
        let mut pid: i32 = 0;
        if AXUIElementGetPid(element, &mut pid) == kAXErrorSuccess && pid > 0 {
            let app_elem = AXUIElementCreateApplication(pid);
            if !app_elem.is_null() {
                let n = get_ax_string_attribute(app_elem, kAXTitleAttribute)
                    .unwrap_or_else(|| String::from("Unknown"));
                CFRelease(app_elem as *const c_void);
                n
            } else {
                String::from("Unknown")
            }
        } else {
            String::from("Unknown")
        }
    };

    let (window_title, window_rel) = window_info_for(element, x, y);

    CFRelease(element as *const c_void);
    CFRelease(system_wide as *const c_void);

    Some(ElementInfo {
        role,
        name,
        app,
        fallback_coords: Some((x, y)),
        value,
        description,
        identifier,
        role_description,
        window_title,
        window_rel,
    })
}

/// Resolve the window containing `element`: its title, and the click point
/// relative to the window's top-left corner. Best-effort — either part is
/// `None` when the AXWindow ancestor or its frame can't be read, so capture
/// degrades gracefully on windowless elements (menu bar, desktop).
unsafe fn window_info_for(
    element: AXUIElementRef,
    x: i32,
    y: i32,
) -> (Option<String>, Option<(i32, i32)>) {
    let attr = str_to_cfstring(kAXWindowAttribute);
    if attr.is_null() {
        return (None, None);
    }
    let mut window: CFTypeRef = std::ptr::null();
    let got = AXUIElementCopyAttributeValue(element, attr, &mut window);
    CFRelease(attr);
    if got != kAXErrorSuccess || window.is_null() {
        return (None, None);
    }
    let window_el = window as AXUIElementRef;

    let title = get_ax_string_attribute(window_el, kAXTitleAttribute).filter(|t| !t.is_empty());
    let rel = ax_element_origin(window_el).map(|(ox, oy)| (x - ox, y - oy));

    CFRelease(window);
    (title, rel)
}

/// Read an element's AXPosition — an AXValue-boxed CGPoint, not a CFString,
/// so it must be unpacked with AXValueGetValue rather than the string helper.
/// Best-effort: `None` on any failure.
unsafe fn ax_element_origin(element: AXUIElementRef) -> Option<(i32, i32)> {
    let pos_attr = str_to_cfstring(kAXPositionAttribute);
    if pos_attr.is_null() {
        return None;
    }
    let mut pos_value: CFTypeRef = std::ptr::null();
    let got = AXUIElementCopyAttributeValue(element, pos_attr, &mut pos_value);
    CFRelease(pos_attr);
    if got != kAXErrorSuccess || pos_value.is_null() {
        return None;
    }
    let mut origin = CGPoint { x: 0.0, y: 0.0 };
    let ok = AXValueGetValue(
        pos_value,
        kAXValueCGPointType,
        &mut origin as *mut CGPoint as *mut c_void,
    ) != 0;
    CFRelease(pos_value);
    ok.then_some((origin.x as i32, origin.y as i32))
}

/// Does a live process name match the recorded app name? `proc_name` output
/// truncates at 32 bytes (2 × MAXCOMLEN), so a long recorded app name also
/// matches its 32-byte prefix.
fn proc_name_matches_app(proc_name: &str, app: &str) -> bool {
    if proc_name.is_empty() || app.is_empty() {
        return false;
    }
    if proc_name.eq_ignore_ascii_case(app) {
        return true;
    }
    proc_name.len() >= 32
        && app.len() > proc_name.len()
        && app.as_bytes()[..proc_name.len()].eq_ignore_ascii_case(proc_name.as_bytes())
}

/// Find the current top-left corner of the recorded element's window,
/// permission-free: enumerate pids via libproc, match the process name
/// against the recorded app, then walk that app's AXWindows (covered by the
/// Accessibility permission replay already requires) for a title match.
/// Best-effort at every step — any failure returns `None` and the resolution
/// chain falls through to the spiral / recorded coordinates.
fn find_window_origin(target: &ElementInfo) -> Option<(i32, i32)> {
    let title = target.window_title.as_deref().filter(|t| !t.is_empty())?;
    let app = match target.app.as_str() {
        "" | "Unknown" => return None,
        a => a,
    };

    unsafe {
        // First call sizes the pid list; slack absorbs processes spawned
        // between the two calls.
        let bytes = proc_listallpids(std::ptr::null_mut(), 0);
        if bytes <= 0 {
            return None;
        }
        let pid_size = std::mem::size_of::<c_int>();
        let mut pids = vec![0 as c_int; bytes as usize / pid_size + 16];
        let bytes = proc_listallpids(
            pids.as_mut_ptr() as *mut c_void,
            (pids.len() * pid_size) as c_int,
        );
        if bytes <= 0 {
            return None;
        }
        pids.truncate(bytes as usize / pid_size);

        for &pid in &pids {
            if pid <= 0 {
                continue;
            }
            let mut name_buf = [0u8; 64];
            let len = proc_name(pid, name_buf.as_mut_ptr() as *mut c_void, 64);
            if len <= 0 {
                continue;
            }
            let Ok(pname) = std::str::from_utf8(&name_buf[..len as usize]) else {
                continue;
            };
            if !proc_name_matches_app(pname, app) {
                continue;
            }
            if let Some(origin) = app_window_origin_by_title(pid, title) {
                return Some(origin);
            }
        }
    }
    None
}

/// Walk one app's AXWindows for a window whose AXTitle equals `title`
/// (case-insensitive) and return its origin.
unsafe fn app_window_origin_by_title(pid: c_int, title: &str) -> Option<(i32, i32)> {
    let app_el = AXUIElementCreateApplication(pid);
    if app_el.is_null() {
        return None;
    }
    let attr = str_to_cfstring(kAXWindowsAttribute);
    if attr.is_null() {
        CFRelease(app_el as *const c_void);
        return None;
    }
    let mut windows: CFTypeRef = std::ptr::null();
    let got = AXUIElementCopyAttributeValue(app_el, attr, &mut windows);
    CFRelease(attr);

    let mut origin = None;
    if got == kAXErrorSuccess && !windows.is_null() {
        let count = CFArrayGetCount(windows);
        for i in 0..count {
            // Array values follow the CF get-rule (borrowed): never released.
            let win = CFArrayGetValueAtIndex(windows, i) as AXUIElementRef;
            if win.is_null() {
                continue;
            }
            let win_title = get_ax_string_attribute(win, kAXTitleAttribute).unwrap_or_default();
            if win_title.eq_ignore_ascii_case(title) {
                origin = ax_element_origin(win);
                break;
            }
        }
        CFRelease(windows);
    }
    CFRelease(app_el as *const c_void);
    origin
}

/// AX exposes secure inputs as `AXSecureTextField` (casing has varied
/// historically), so the match is case-insensitive and substring-based.
fn is_secure_role(role: &str) -> bool {
    role.to_ascii_lowercase().contains("securetextfield")
}

/// Suppression decision shared by every mouse press *and* release capture in
/// `process_cg_event`: a click over a secure input must not be recorded at
/// all, not even as bare coordinates.
fn is_secure_click(element: &Option<ElementInfo>) -> bool {
    element.as_ref().is_some_and(|e| is_secure_role(&e.role))
}

/// Re-resolve where to click for a recorded click. Scans nearby points when
/// the element has moved (shared spiral in core::replay_support); `None`
/// means no matching element exists anywhere near the recorded point, and
/// the `ResolutionKind` reports which strategy found it — feeding the
/// per-run resolution trace.
fn try_resolve_click_point_traced(
    target: &ElementInfo,
    rx: i32,
    ry: i32,
) -> Option<((i32, i32), replay_support::ResolutionKind)> {
    replay_support::try_resolve_click_point_traced(
        target,
        rx,
        ry,
        |x, y| unsafe { ax_info_at(x, y) },
        find_window_origin,
    )
}

/// Resolve a press target and record the outcome (recorded point, spiral hit,
/// or coordinate fallback) into the run's resolution trace.
fn resolve_press_traced(
    desc: &ElementInfo,
    rx: i32,
    ry: i32,
    idx: usize,
    progress: &replay_support::ReplayProgress,
) -> (i32, i32) {
    let (point, kind) = match try_resolve_click_point_traced(desc, rx, ry) {
        Some(resolved) => resolved,
        None => ((rx, ry), replay_support::ResolutionKind::CoordinateFallback),
    };
    progress.record_resolution(replay_support::StepResolution {
        step_index: idx,
        kind,
        target_name: Some(desc.name.clone()),
        point,
    });
    point
}

unsafe fn get_ax_string_attribute(element: AXUIElementRef, attribute: &str) -> Option<String> {
    let cf_string = str_to_cfstring(attribute);
    if cf_string.is_null() {
        return None;
    }

    let mut value: CFTypeRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(element, cf_string, &mut value) != kAXErrorSuccess {
        CFRelease(cf_string);
        return None;
    }
    CFRelease(cf_string);

    if value.is_null() {
        return None;
    }

    let c_str = CFStringGetCStringPtr(value, kCFStringEncodingUTF8);
    let result = if !c_str.is_null() {
        Some(CStr::from_ptr(c_str).to_string_lossy().into_owned())
    } else {
        let len = CFStringGetLength(value);
        let max_size = CFStringGetMaximumSizeForEncoding(len, kCFStringEncodingUTF8);
        if max_size > 0 {
            let mut buffer = vec![0u8; (max_size + 1) as usize];
            if CFStringGetCString(
                value,
                buffer.as_mut_ptr() as *mut c_char,
                max_size + 1,
                kCFStringEncodingUTF8,
            ) != 0
            {
                Some(
                    String::from_utf8_lossy(
                        &buffer[..buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len())],
                    )
                    .to_string(),
                )
            } else {
                None
            }
        } else {
            None
        }
    };

    CFRelease(value);
    result
}

fn str_to_cfstring(s: &str) -> CFStringRef {
    unsafe {
        CFStringCreateWithBytes(
            std::ptr::null(),
            s.as_ptr(),
            s.len() as isize,
            kCFStringEncodingUTF8,
            0,
        )
    }
}

// ── Element locator ───────────────────────────────────────────────────────────

// ── Replay engine ─────────────────────────────────────────────────────────────

struct MacosReplayer;

/// Map a recorded button code to the synthesized button + direction.
/// Recordings capture mouse-down (0/2) and mouse-up (1/3) as separate
/// events; replay mirrors press/release so single clicks don't double-fire
/// and drags / double-clicks survive faithfully.
fn click_action(button: u8) -> (Button, Direction) {
    match button {
        0 => (Button::Left, Direction::Press),
        1 => (Button::Left, Direction::Release),
        2 => (Button::Right, Direction::Press),
        3 => (Button::Right, Direction::Release),
        _ => (Button::Left, Direction::Click),
    }
}

impl ReplayEngine for MacosReplayer {
    fn execute(
        &self,
        events: &[InputEvent],
        stop_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        speed: f32,
        progress: Arc<ReplayProgress>,
    ) -> anyhow::Result<()> {
        use crate::core::vision;
        use crate::core::wait::VariableContext;

        let mut enigo = Enigo::new(&Settings::default())?;
        let speed = speed.max(0.1);
        let mut var_context = VariableContext::new();
        let mut prev_ts: Option<u64> = None;

        for (idx, event) in events.iter().enumerate() {
            progress.set_step(idx);
            if !check_continue(&stop_flag, &pause_flag) {
                return Ok(());
            }

            // Reproduce the recorded rhythm between events (recordings made
            // before timestamps existed simply run back-to-back).
            let gap = pacing_gap_ms(prev_ts, event.timestamp());
            if gap > 0 && !interruptible_sleep((gap as f32 / speed) as u64, &stop_flag, &pause_flag)
            {
                return Ok(());
            }
            if let Some(ts) = event.timestamp() {
                prev_ts = Some(ts);
            }

            match event {
                InputEvent::MouseClick {
                    x,
                    y,
                    button,
                    element,
                    ..
                } => {
                    let (mouse_button, direction) = click_action(*button);

                    // Re-resolve press targets whose element moved; releases
                    // stay at recorded coordinates so drags end where the
                    // user ended them. Each press records how its target was
                    // resolved into the run trace.
                    let (cx, cy) = match (element, &direction) {
                        (Some(desc), Direction::Press) => {
                            resolve_press_traced(desc, *x, *y, idx, &progress)
                        }
                        (None, Direction::Press) => {
                            progress.record_resolution(replay_support::StepResolution {
                                step_index: idx,
                                kind: replay_support::ResolutionKind::NoDescriptor,
                                target_name: None,
                                point: (*x, *y),
                            });
                            (*x, *y)
                        }
                        _ => (*x, *y),
                    };

                    enigo.move_mouse(cx, cy, Coordinate::Abs)?;
                    enigo.button(mouse_button, direction)?;
                }
                InputEvent::Key {
                    code,
                    chars,
                    action,
                    ..
                } => {
                    let key = if !chars.is_empty() {
                        Key::Unicode(chars.chars().next().unwrap_or(' '))
                    } else {
                        Key::Other(*code as u32)
                    };
                    match action {
                        KeyAction::Down => enigo.key(key, Direction::Press)?,
                        KeyAction::Up => enigo.key(key, Direction::Release)?,
                    }
                }
                InputEvent::Scroll { dx, dy, .. } => {
                    if *dx != 0 {
                        enigo.scroll(*dx, Axis::Horizontal)?;
                    }
                    if *dy != 0 {
                        enigo.scroll(*dy, Axis::Vertical)?;
                    }
                }
                InputEvent::Delay { ms, .. } => {
                    let adjusted_ms = (*ms as f32 / speed) as u64;
                    if !interruptible_sleep(adjusted_ms, &stop_flag, &pause_flag) {
                        return Ok(());
                    }
                }
                InputEvent::Wait {
                    condition,
                    timeout_ms,
                    poll_interval_ms,
                } => {
                    tracing::info!("Waiting for condition: {:?}", condition);
                    let locator = crate::platform::macos::MacosBackend::locator();
                    let result = crate::core::wait::wait_for_condition(
                        condition,
                        locator.as_ref(),
                        *timeout_ms,
                        *poll_interval_ms,
                    );
                    match result {
                        crate::core::wait::WaitResult::Error(e) => {
                            tracing::warn!("Wait condition failed: {}", e);
                        }
                        crate::core::wait::WaitResult::Timeout => {
                            tracing::warn!("Wait condition timed out");
                        }
                        crate::core::wait::WaitResult::Success => {}
                    }
                }
                InputEvent::VisualCheck {
                    baseline_screenshot,
                    threshold,
                    on_mismatch,
                } => match vision::capture_screenshot() {
                    Ok(img_bytes) => {
                        if let Ok(current_img) = image::load_from_memory(&img_bytes) {
                            if let Ok(similarity) =
                                vision::compare_images(baseline_screenshot, &current_img)
                            {
                                if similarity < *threshold {
                                    tracing::warn!(
                                        "Visual mismatch: {:.2} < {}",
                                        similarity,
                                        threshold
                                    );
                                    match on_mismatch {
                                        crate::core::events::MismatchAction::Fail => {
                                            return Err(anyhow::anyhow!(
                                                "Visual regression detected"
                                            ));
                                        }
                                        crate::core::events::MismatchAction::Retry { attempts } => {
                                            for _ in 0..*attempts {
                                                thread::sleep(Duration::from_millis(500));
                                                if let Ok(b) = vision::capture_screenshot() {
                                                    if let Ok(img) = image::load_from_memory(&b) {
                                                        if vision::compare_images(
                                                            baseline_screenshot,
                                                            &img,
                                                        )
                                                        .unwrap_or(1.0)
                                                            >= *threshold
                                                        {
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        crate::core::events::MismatchAction::LogOnly => {}
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => tracing::error!("Screenshot failed: {}", e),
                },
                InputEvent::Variable {
                    name,
                    value_template,
                    var_type,
                } => {
                    let resolved = var_context
                        .resolve(name, var_type)
                        .unwrap_or_else(|_| value_template.clone());
                    var_context.set(name.clone(), resolved);
                }
                InputEvent::VariableRef { name } => {
                    tracing::debug!("Variable reference: {}", name);
                }
            }
        }

        Ok(())
    }

    fn execute_with_reliability(
        &self,
        events: &[InputEvent],
        stop_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        speed: f32,
        progress: Arc<ReplayProgress>,
        reliability: &crate::core::events::ReliabilitySettings,
    ) -> anyhow::Result<()> {
        let mut enigo = Enigo::new(&Settings::default())?;
        let speed = speed.max(0.1);
        let mut prev_ts: Option<u64> = None;

        for (idx, event) in events.iter().enumerate() {
            progress.set_step(idx);
            if !check_continue(&stop_flag, &pause_flag) {
                return Ok(());
            }

            let gap = pacing_gap_ms(prev_ts, event.timestamp());
            if gap > 0 && !interruptible_sleep((gap as f32 / speed) as u64, &stop_flag, &pause_flag)
            {
                return Ok(());
            }
            if let Some(ts) = event.timestamp() {
                prev_ts = Some(ts);
            }

            match event {
                InputEvent::MouseClick {
                    x,
                    y,
                    button,
                    element,
                    retry_count,
                    ..
                } => {
                    let (mouse_button, direction) = click_action(*button);

                    // Reliability means retrying the element *lookup* with
                    // backoff, so replay waits out slow-loading UIs instead of
                    // blind-clicking stale coordinates.
                    let (cx, cy) = match (element, &direction) {
                        (Some(desc), Direction::Press) => {
                            let max_attempts = retry_count
                                .unwrap_or(reliability.retry_config.max_attempts)
                                .max(1);
                            let mut resolved = None;
                            for attempt in 0..max_attempts {
                                resolved = try_resolve_click_point_traced(desc, *x, *y);
                                if resolved.is_some() {
                                    break;
                                }
                                if attempt + 1 < max_attempts {
                                    let backoff = (reliability.retry_config.backoff_ms as f32
                                        * reliability
                                            .retry_config
                                            .backoff_multiplier
                                            .max(1.0)
                                            .powi(attempt as i32))
                                        as u64;
                                    if !interruptible_sleep(backoff, &stop_flag, &pause_flag) {
                                        return Ok(());
                                    }
                                }
                            }
                            let (point, kind) = match resolved {
                                Some(resolved) => resolved,
                                None => {
                                    ((*x, *y), replay_support::ResolutionKind::CoordinateFallback)
                                }
                            };
                            progress.record_resolution(replay_support::StepResolution {
                                step_index: idx,
                                kind: kind.clone(),
                                target_name: Some(desc.name.clone()),
                                point,
                            });
                            match kind {
                                replay_support::ResolutionKind::CoordinateFallback
                                    if !reliability.continue_on_error =>
                                {
                                    return Err(anyhow::anyhow!(
                                        "Element \"{}\" not found after {} attempts",
                                        desc.name,
                                        max_attempts
                                    ))
                                }
                                replay_support::ResolutionKind::CoordinateFallback => {
                                    tracing::warn!(
                                        "Element \"{}\" not found after {} attempts; using recorded coordinates",
                                        desc.name,
                                        max_attempts
                                    );
                                    point
                                }
                                _ => point,
                            }
                        }
                        _ => (*x, *y),
                    };

                    enigo.move_mouse(cx, cy, Coordinate::Abs)?;
                    enigo.button(mouse_button, direction)?;
                }
                InputEvent::Key {
                    code,
                    chars,
                    action,
                    ..
                } => {
                    let key = if !chars.is_empty() {
                        Key::Unicode(chars.chars().next().unwrap_or(' '))
                    } else {
                        Key::Other(*code as u32)
                    };
                    match action {
                        KeyAction::Down => enigo.key(key, Direction::Press)?,
                        KeyAction::Up => enigo.key(key, Direction::Release)?,
                    }
                }
                InputEvent::Scroll { dx, dy, .. } => {
                    if *dx != 0 {
                        enigo.scroll(*dx, Axis::Horizontal)?;
                    }
                    if *dy != 0 {
                        enigo.scroll(*dy, Axis::Vertical)?;
                    }
                }
                InputEvent::Delay { ms, .. } => {
                    let adjusted_ms = (*ms as f32 / speed) as u64;
                    if !interruptible_sleep(adjusted_ms, &stop_flag, &pause_flag) {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element_with_role(role: &str) -> Option<ElementInfo> {
        Some(ElementInfo {
            role: role.into(),
            ..Default::default()
        })
    }

    // `capture_click` IS the wiring for all four click captures: the same
    // function `process_cg_event` calls, minus only the CGEvent/AX FFI glue.
    // RightMouseUp once skipped the secure gate and captured coordinates
    // unconditionally; these tests exercise every event-type arm — not just
    // the `is_secure_click` predicate — so removing the gate from any single
    // arm fails the suite.

    const ALL_CLICK_TYPES: [CGEventType; 4] = [
        kCGMouseEventLeftMouseDown,
        kCGMouseEventLeftMouseUp,
        kCGMouseEventRightMouseDown,
        kCGMouseEventRightMouseUp,
    ];

    #[test]
    fn every_click_type_is_suppressed_over_secure_fields() {
        for etype in ALL_CLICK_TYPES {
            assert!(
                capture_click(etype, 10, 20, element_with_role("AXSecureTextField")).is_none(),
                "event type {etype} captured a click over a secure field"
            );
        }
    }

    #[test]
    fn secure_role_match_is_case_insensitive() {
        for role in ["axsecuretextfield", "AXSECURETEXTFIELD"] {
            assert!(
                capture_click(kCGMouseEventRightMouseUp, 10, 20, element_with_role(role)).is_none()
            );
        }
    }

    #[test]
    fn ordinary_clicks_map_button_codes_and_element_retention() {
        // (event type, expected button code, press keeps the element)
        let cases = [
            (kCGMouseEventLeftMouseDown, 0u8, true),
            (kCGMouseEventLeftMouseUp, 1, false),
            (kCGMouseEventRightMouseDown, 2, true),
            (kCGMouseEventRightMouseUp, 3, false),
        ];
        for (etype, expected_button, keeps_element) in cases {
            match capture_click(etype, 10, 20, element_with_role("AXButton")) {
                Some(InputEvent::MouseClick {
                    x,
                    y,
                    button,
                    element,
                    ..
                }) => {
                    assert_eq!((x, y), (10, 20));
                    assert_eq!(button, expected_button, "event type {etype}");
                    assert_eq!(element.is_some(), keeps_element, "event type {etype}");
                }
                other => panic!("event type {etype}: expected MouseClick, got {other:?}"),
            }
        }
    }

    #[test]
    fn clicks_with_no_resolved_element_are_captured() {
        for etype in ALL_CLICK_TYPES {
            assert!(capture_click(etype, 10, 20, None).is_some());
        }
    }

    #[test]
    fn non_click_event_types_capture_nothing() {
        assert!(capture_click(kCGKeyDown, 10, 20, None).is_none());
        assert!(capture_click(kCGScrollWheelEvent, 10, 20, None).is_none());
    }

    // ── keyboard capture truncation ───────────────────────────────────────

    #[test]
    fn utf16_capture_len_passes_through_when_string_fits() {
        let buf = [0x0041u16; 8]; // 'A' × 8
        assert_eq!(utf16_capture_len(&buf, 2), 2);
        assert_eq!(utf16_capture_len(&buf, 8), 8);
    }

    #[test]
    fn utf16_capture_len_clamps_overflow_to_buffer() {
        let buf = [0x0041u16; 8];
        assert_eq!(utf16_capture_len(&buf, 20), 8);
    }

    // ── window lookup: process-name matching ─────────────────────────────

    #[test]
    fn proc_name_matches_app_exact_and_case_insensitive() {
        assert!(proc_name_matches_app("Notes", "Notes"));
        assert!(proc_name_matches_app("notes", "Notes"));
        assert!(!proc_name_matches_app("Notes", "Numbers"));
        assert!(!proc_name_matches_app("", "Notes"));
        assert!(!proc_name_matches_app("Notes", ""));
    }

    #[test]
    fn proc_name_matches_app_accepts_32_byte_truncation() {
        // proc_name truncates at 32 bytes; a longer recorded app name must
        // still match its truncated prefix…
        let long_app = "An Extremely Long Application Name";
        let truncated = &long_app[..32];
        assert_eq!(truncated.len(), 32);
        assert!(proc_name_matches_app(truncated, long_app));

        // …but a SHORT process name must not prefix-match a longer app name
        // ("Note" is not "Notes Helper").
        assert!(!proc_name_matches_app("Note", "Notes Helper"));
    }

    #[test]
    fn find_window_origin_bails_without_title_or_usable_app() {
        // Pure gate before any FFI runs: missing title or an unusable app
        // name must return None without touching process enumeration.
        let mut el = element_with_role("AXButton").unwrap();
        el.app = "Notes".into();
        el.window_title = None;
        assert_eq!(find_window_origin(&el), None);

        el.window_title = Some("Report".into());
        el.app = "Unknown".into();
        assert_eq!(find_window_origin(&el), None);
        el.app = String::new();
        assert_eq!(find_window_origin(&el), None);
    }

    #[test]
    fn utf16_capture_len_never_splits_a_surrogate_pair() {
        // Truncated string ends on a high surrogate whose low half was lost
        // beyond the buffer: the lone half must be dropped, not turned into
        // a U+FFFD replacement character.
        let mut buf = [0x0041u16; 8];
        buf[7] = 0xD83D; // high surrogate (e.g. first half of an emoji)
        assert_eq!(utf16_capture_len(&buf, 9), 7);

        // But a high surrogate at the end of a string that FITS is kept:
        // that's genuinely what the event contained.
        assert_eq!(utf16_capture_len(&buf, 8), 8);
    }
}
