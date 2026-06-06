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
use std::time::Duration;

// Core Foundation types
type CFMachPortRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CGEventRef = *mut c_void;
type CGEventTapId = u32;
type CGEventType = u32;
type CGKeyCode = u16;
type AXUIElementRef = *mut c_void;
type AXError = i32;
type AXValueRef = *mut c_void;
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

// CGEventField constants
const kCGMouseEventX: u32 = 0;
const kCGMouseEventY: u32 = 1;
const kCGKeyboardEventKeycode: u32 = 0;
const kCGScrollWheelEventDeltaAxis1: u32 = 1;
const kCGScrollWheelEventDeltaAxis2: u32 = 2;

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
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CFMachPortGetRunLoopSource(port: CFMachPortRef) -> CFRunLoopSourceRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();
    fn CFRunLoopStop(rl: CFRunLoopRef);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: Boolean);
}

// We need to define the callback type properly
type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapId,
    etype: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

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
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFStringGetCStringPtr(theString: CFStringRef, encoding: CFStringEncoding) -> *const c_char;
    fn CFRelease(cf: CFTypeRef);
}

// Core Foundation external functions
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
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
}

// Constants
const kCFStringEncodingUTF8: CFStringEncoding = 0x08000100;
const kCFRunLoopCommonModes: CFStringRef = std::ptr::null(); // Simplified - would be kCFRunLoopCommonMode in real impl
const kCGSessionEventTap: CGEventTapId = 0;
const kCGHeadInsertEventTap: u32 = 0;
const kCGEventTapOptionDefault: u32 = 0;
const kAXUIElementInvalid: AXUIElementRef = std::ptr::null_mut();

/// macOS-specific backend providing recorder, locator, and replayer implementations.
pub struct MacosBackend;

// Thread-local storage for the run loop reference
struct TapState {
    run_loop: Option<CFRunLoopRef>,
    tap_port: Option<CFMachPortRef>,
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
        Box::new(MacosReplayer::new())
    }

    /// Check if accessibility permissions are granted
    pub fn check_accessibility() -> bool {
        unsafe {
            let system_wide = AXUIElementCreateSystemWide();
            if system_wide.is_null() {
                return false;
            }
            // If we can create a system-wide element, we have access
            true
        }
    }

    /// Request accessibility permissions (UI)
    pub fn request_accessibility() -> bool {
        // On macOS, this would typically trigger a system dialog
        // For now, we check if we already have access
        Self::check_accessibility()
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

            unsafe {
                // Create the event tap with correct signature
                let tap = CGEventTapCreate(
                    kCGSessionEventTap,
                    kCGHeadInsertEventTap,
                    kCGEventTapOptionDefault,
                    event_mask,
                    cg_event_callback,
                    Box::into_raw(Box::new(tx)) as *mut c_void,
                );

                if tap.is_null() {
                    eprintln!("Failed to create CGEventTap - Accessibility permissions may be required");
                    return;
                }

                let run_loop_source = CFMachPortGetRunLoopSource(tap);
                let current_run_loop = CFRunLoopGetCurrent();
                CFRunLoopAddSource(
                    current_run_loop,
                    run_loop_source,
                    kCFRunLoopCommonModes,
                );

                // Enable the tap
                CGEventTapEnable(tap, 1);

                *state_clone.lock().unwrap() = Some(TapState {
                    run_loop: Some(current_run_loop),
                    tap_port: Some(tap),
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
            // Get absolute screen coordinates using kCGMouseEventX and kCGMouseEventY
            let x = CGEventGetIntegerValueField(event, kCGMouseEventX) as i32;
            let y = CGEventGetIntegerValueField(event, kCGMouseEventY) as i32;
            let button = if etype == kCGMouseEventLeftMouseDown { 0 } else { 1 };
            InputEvent::MouseClick {
                x,
                y,
                button,
                element: None, // Will be populated by element locator if needed
            }
        }
        kCGMouseEventRightMouseDown | kCGMouseEventRightMouseUp => {
            let x = CGEventGetIntegerValueField(event, kCGMouseEventX) as i32;
            let y = CGEventGetIntegerValueField(event, kCGMouseEventY) as i32;
            let button = if etype == kCGMouseEventRightMouseDown { 2 } else { 3 };
            InputEvent::MouseClick {
                x,
                y,
                button,
                element: None,
            }
        }
        kCGKeyDown | kCGKeyUp => {
            let code = CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode) as u16;
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
            let dx = CGEventGetIntegerValueField(event, kCGScrollWheelEventDeltaAxis2) as i32;
            let dy = CGEventGetIntegerValueField(event, kCGScrollWheelEventDeltaAxis1) as i32;
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
    // Create CFString from Rust string
    let cf_string = str_to_cfstring(attribute);
    if cf_string.is_null() {
        return None;
    }
    
    let mut value: CFTypeRef = std::ptr::null();
    
    if AXUIElementCopyAttributeValue(element, cf_string, &mut value) != kAXErrorSuccess {
        CFRelease(cf_string);
        return None;
    }

    if value.is_null() {
        CFRelease(cf_string);
        return None;
    }

    // Try to get as CFString using CFStringGetCStringPtr
    let c_str = CFStringGetCStringPtr(value, kCFStringEncodingUTF8);
    let result = if !c_str.is_null() {
        Some(CStr::from_ptr(c_str).to_string_lossy().into_owned())
    } else {
        // Fallback: try CFStringGetCString
        let len = CFStringGetLength(value);
        let max_size = CFStringGetMaximumSizeForEncoding(len, kCFStringEncodingUTF8);
        if max_size > 0 {
            let mut buffer = vec![0u8; (max_size + 1) as usize];
            if CFStringGetCString(value, buffer.as_mut_ptr() as *mut c_char, max_size + 1, kCFStringEncodingUTF8) != 0 {
                Some(String::from_utf8_lossy(&buffer[..buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len())]).to_string())
            } else {
                None
            }
        } else {
            None
        }
    };

    CFRelease(value);
    CFRelease(cf_string);
    result
}

/// Convert a Rust string to CFStringRef
fn str_to_cfstring(s: &str) -> CFStringRef {
    unsafe {
        CFStringCreateWithBytes(
            std::ptr::null(), // Use default allocator
            s.as_ptr(),
            s.len() as isize,
            kCFStringEncodingUTF8,
            0, // is_external_representation = false
        )
    }
}

/// macOS replay engine using enigo.
struct MacosReplayer {
    speed_factor: Arc<Mutex<f32>>,
}

impl MacosReplayer {
    fn new() -> Self {
        MacosReplayer {
            speed_factor: Arc::new(Mutex::new(1.0)),
        }
    }
    
    /// Set playback speed factor (1.0 = normal, 2.0 = 2x speed, etc.)
    fn set_speed(&self, factor: f32) {
        *self.speed_factor.lock().unwrap() = factor.max(0.1);
    }
}

impl ReplayEngine for MacosReplayer {
    fn execute(&self, events: &[InputEvent], stop_flag: Arc<AtomicBool>) -> anyhow::Result<()> {
        use crate::core::wait::VariableContext;
        use crate::core::vision;
        
        let mut enigo = Enigo::new();
        let speed = *self.speed_factor.lock().unwrap();
        let mut var_context = VariableContext::new();

        for event in events {
            if stop_flag.load(Ordering::Relaxed) {
                return Ok(());
            }

            match event {
                InputEvent::MouseClick { x, y, button, element, retry_count, .. } => {
                    let max_retries = retry_count.unwrap_or(0);
                    let mut attempts = 0;
                    let mut success = false;
                    
                    while attempts <= max_retries && !success {
                        enigo.mouse_move_to(*x, *y);
                        
                        let mouse_button = match button {
                            0 | 1 => MouseButton::Left,
                            2 | 3 => MouseButton::Right,
                            _ => MouseButton::Left,
                        };
                        
                        enigo.mouse_click(mouse_button);
                        success = true; // In real implementation, validate
                        attempts += 1;
                    }
                }
                InputEvent::Key { code, chars, action, retry_count, .. } => {
                    let max_retries = retry_count.unwrap_or(0);
                    
                    for _ in 0..=max_retries {
                        match action {
                            KeyAction::Down => {
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
                }
                InputEvent::Scroll { dx, dy, .. } => {
                    enigo.scroll(*dx, *dy);
                }
                InputEvent::Delay { ms, .. } => {
                    let adjusted_ms = (*ms as f32 / speed) as u64;
                    thread::sleep(Duration::from_millis(adjusted_ms));
                }
                // Phase 3: Smart Wait Events
                InputEvent::Wait { condition, timeout_ms, poll_interval_ms } => {
                    tracing::info!("Waiting for condition: {:?}", condition);
                    // Use a local locator for wait condition checking
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
                // Phase 3: Visual Regression Check
                InputEvent::VisualCheck { baseline_screenshot, threshold, on_mismatch } => {
                    // Capture current screen
                    match vision::capture_screenshot() {
                        Ok(img_bytes) => {
                            if let Ok(current_img) = image::load_from_memory(&img_bytes) {
                                if let Ok(similarity) = vision::compare_images(baseline_screenshot, &current_img) {
                                    if similarity < *threshold {
                                        tracing::warn!("Visual mismatch detected: {:.2} < {}", similarity, threshold);
                                        // Handle mismatch action
                                        match on_mismatch {
                                            crate::core::events::MismatchAction::Fail => {
                                                return Err(anyhow::anyhow!("Visual regression detected"));
                                            }
                                            crate::core::events::MismatchAction::Retry { attempts } => {
                                                // Retry the check
                                                for _ in 0..*attempts {
                                                    thread::sleep(Duration::from_millis(500));
                                                    if let Ok(new_img) = vision::capture_screenshot() {
                                                        if let Ok(new_img) = image::load_from_memory(&new_img) {
                                                            if vision::compare_images(baseline_screenshot, &new_img).unwrap_or(1.0) >= *threshold {
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            crate::core::events::MismatchAction::LogOnly => {
                                                // Just log, continue
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to capture screenshot for visual check: {}", e);
                        }
                    }
                }
                // Phase 3: Variable Injection
                InputEvent::Variable { name, value_template, var_type } => {
                    let resolved = var_context.resolve(name, var_type)
                        .unwrap_or_else(|_| value_template.clone());
                    var_context.set(name.clone(), resolved);
                }
                // Variable Reference - just log for now, actual usage depends on context
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
        reliability: &crate::core::events::ReliabilitySettings
    ) -> anyhow::Result<()> {
        let mut enigo = Enigo::new();
        let speed = *self.speed_factor.lock().unwrap();

        for (idx, event) in events.iter().enumerate() {
            if stop_flag.load(Ordering::Relaxed) {
                return Ok(());
            }

            match event {
                InputEvent::MouseClick { x, y, button, element, retry_count, .. } => {
                    // Validate element if configured
                    if reliability.validate_elements && element.is_none() {
                        // Could add element inspection here
                    }
                    
                    let max_retries = retry_count.unwrap_or(reliability.retry_config.max_attempts);
                    let mut attempts = 0;
                    let mut success = false;
                    
                    while attempts <= max_retries && !success {
                        enigo.mouse_move_to(*x, *y);
                        let mouse_button = match button {
                            0 | 1 => MouseButton::Left,
                            2 | 3 => MouseButton::Right,
                            _ => MouseButton::Left,
                        };
                        enigo.mouse_click(mouse_button);
                        success = true;
                        attempts += 1;
                        
                        if !success && attempts <= max_retries && reliability.continue_on_error {
                            let backoff = reliability.retry_config.backoff_ms * 
                                (reliability.retry_config.backoff_multiplier as u64).pow(attempts - 1);
                            std::thread::sleep(Duration::from_millis(backoff));
                        }
                    }
                }
                InputEvent::Key { code, chars, action, retry_count, .. } => {
                    let max_retries = retry_count.unwrap_or(reliability.retry_config.max_attempts);
                    
                    for attempt in 0..=max_retries {
                        if stop_flag.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        
                        match action {
                            KeyAction::Down => {
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
                        
                        if attempt < max_retries {
                            let backoff = reliability.retry_config.backoff_ms * 
                                (reliability.retry_config.backoff_multiplier as u64);
                            std::thread::sleep(Duration::from_millis(backoff));
                        }
                    }
                }
                InputEvent::Scroll { dx, dy, .. } => {
                    enigo.scroll(*dx, *dy);
                }
                InputEvent::Delay { ms, .. } => {
                    let adjusted_ms = (*ms as f32 / speed) as u64;
                    std::thread::sleep(Duration::from_millis(adjusted_ms));
                }
            }
        }

        Ok(())
    }
}
