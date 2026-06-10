//! macOS backend implementation using CGEventTap, AXUIElement, and enigo.

use crate::core::events::{ElementInfo, InputEvent, KeyAction};
use crate::core::replay_support::{self, check_continue, interruptible_sleep, pacing_gap_ms};
use crate::core::traits::{ElementLocator, InputRecorder, ReplayEngine};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
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

    static kCFRunLoopCommonModes: CFStringRef;
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
    tap_port: Option<CFMachPortRef>,
    is_running: Arc<AtomicBool>,
}

unsafe impl Send for TapState {}
unsafe impl Sync for TapState {}

impl MacosBackend {
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
unsafe fn extract_key_chars(event: CGEventRef) -> String {
    let mut actual_len: usize = 0;
    let mut buf = [0u16; 8];
    CGEventKeyboardGetUnicodeString(event, buf.len(), &mut actual_len, buf.as_mut_ptr());
    if actual_len > 0 {
        String::from_utf16_lossy(&buf[..actual_len])
    } else {
        String::new()
    }
}

// ── CGEventTap callback ───────────────────────────────────────────────────────

unsafe extern "C" fn cg_event_callback(
    _proxy: CGEventTapId,
    etype: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    let tx = &*(user_info as *mut mpsc::Sender<InputEvent>);

    let input_event = match etype {
        // Mouse down: perform AX element lookup while we have the coordinates
        kCGMouseEventLeftMouseDown => {
            let loc = CGEventGetLocation(event);
            let (x, y) = (loc.x as i32, loc.y as i32);
            let element = ax_info_at(x, y);
            if element.as_ref().map_or(false, |e| is_secure_role(&e.role)) {
                return event;
            }
            InputEvent::MouseClick {
                x,
                y,
                button: 0,
                element,
                timestamp: None,
                retry_count: None,
                semantic_tag: None,
                self_heal: None,
            }
        }
        kCGMouseEventLeftMouseUp => {
            let loc = CGEventGetLocation(event);
            let (x, y) = (loc.x as i32, loc.y as i32);
            let element = ax_info_at(x, y);
            if element.as_ref().map_or(false, |e| is_secure_role(&e.role)) {
                return event;
            }
            InputEvent::MouseClick {
                x,
                y,
                button: 1,
                element: None,
                timestamp: None,
                retry_count: None,
                semantic_tag: None,
                self_heal: None,
            }
        }
        kCGMouseEventRightMouseDown => {
            let loc = CGEventGetLocation(event);
            let (x, y) = (loc.x as i32, loc.y as i32);
            let element = ax_info_at(x, y);
            if element.as_ref().map_or(false, |e| is_secure_role(&e.role)) {
                return event;
            }
            InputEvent::MouseClick {
                x,
                y,
                button: 2,
                element,
                timestamp: None,
                retry_count: None,
                semantic_tag: None,
                self_heal: None,
            }
        }
        kCGMouseEventRightMouseUp => {
            let loc = CGEventGetLocation(event);
            let (x, y) = (loc.x as i32, loc.y as i32);
            InputEvent::MouseClick {
                x,
                y,
                button: 3,
                element: None,
                timestamp: None,
                retry_count: None,
                semantic_tag: None,
                self_heal: None,
            }
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
            InputEvent::Key {
                code,
                chars,
                modifiers,
                action,
                timestamp: None,
                retry_count: None,
                semantic_tag: None,
            }
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
            InputEvent::Scroll {
                dx,
                dy,
                phase,
                timestamp: None,
            }
        }
        _ => return event,
    };

    let _ = tx.send(input_event);
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
    })
}

/// AX exposes secure inputs as `AXSecureTextField` (casing has varied
/// historically), so the match is case-insensitive and substring-based.
fn is_secure_role(role: &str) -> bool {
    role.to_ascii_lowercase().contains("securetextfield")
}

/// Re-resolve where to click for a recorded click. Scans nearby points when
/// the element has moved (shared spiral in core::replay_support); `None`
/// means no matching element exists anywhere near the recorded point.
fn try_resolve_click_point(target: &ElementInfo, rx: i32, ry: i32) -> Option<(i32, i32)> {
    replay_support::try_resolve_click_point(target, rx, ry, |x, y| unsafe { ax_info_at(x, y) })
}

/// Like `try_resolve_click_point`, but falls back to the recorded coordinates
/// so plain replay always proceeds.
fn resolve_click_point(target: &ElementInfo, rx: i32, ry: i32) -> (i32, i32) {
    try_resolve_click_point(target, rx, ry).unwrap_or((rx, ry))
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
    ) -> anyhow::Result<()> {
        use crate::core::vision;
        use crate::core::wait::VariableContext;

        let mut enigo = Enigo::new(&Settings::default())?;
        let speed = speed.max(0.1);
        let mut var_context = VariableContext::new();
        let mut prev_ts: Option<u64> = None;

        for event in events {
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
                    // user ended them.
                    let (cx, cy) = match (element, &direction) {
                        (Some(desc), Direction::Press) => resolve_click_point(desc, *x, *y),
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
        reliability: &crate::core::events::ReliabilitySettings,
    ) -> anyhow::Result<()> {
        let mut enigo = Enigo::new(&Settings::default())?;
        let speed = speed.max(0.1);
        let mut prev_ts: Option<u64> = None;

        for event in events {
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
                                resolved = try_resolve_click_point(desc, *x, *y);
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
                            match resolved {
                                Some(point) => point,
                                None if reliability.continue_on_error => {
                                    tracing::warn!(
                                        "Element \"{}\" not found after {} attempts; using recorded coordinates",
                                        desc.name,
                                        max_attempts
                                    );
                                    (*x, *y)
                                }
                                None => {
                                    return Err(anyhow::anyhow!(
                                        "Element \"{}\" not found after {} attempts",
                                        desc.name,
                                        max_attempts
                                    ))
                                }
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
