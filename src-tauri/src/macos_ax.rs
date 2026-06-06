#![cfg(target_os = "macos")]

use core_foundation::base::{CFType, TCFType};
use core_foundation::runloop::{CFRunLoopGetMain, CFRunLoopRunInMode, kCFRunLoopDefaultMode, CFRunLoopSourceContext};
use core_foundation::string::CFString;
use accessibility_sys::*;
use std::ffi::c_void;
use std::ptr;
use std::sync::OnceLock;
use std::thread;
use tauri::AppHandle;
use crate::{ClickEvent, ElementInfo};

// Global event tap reference
static EVENT_TAP: OnceLock<EventTapRefWrapper> = OnceLock::new();

pub struct EventTapRefWrapper {
    tap: AXEventTapRef,
}

impl Drop for EventTapRefWrapper {
    fn drop(&mut self) {
        unsafe {
            AXEventTapInvalidate(self.tap);
            AXEventTapRelease(self.tap);
        }
    }
}

unsafe extern "C" fn event_tap_callback(
    _proxy: *mut c_void,
    _type_: AXEventType,
    event_ref: AXEventRef,
    refcon: *mut c_void,
) -> AXEventRef {
    // Get the app handle from context
    let app_handle_ptr = refcon as *mut AppHandle;
    
    unsafe {
        // Get mouse location from the event
        let mut point = CGPoint { x: 0.0, y: 0.0 };
        AXEventGetIntegerValueForKey(event_ref, kAXEventMouseGlobalPosition as CFStringRef, &mut point as *mut _ as *mut i32);
        
        // Try to get the element at this position
        let element = AXUIElementCreateSystemWide();
        
        // Get the element at the mouse position
        let mut clicked_element: AXUIElementRef = ptr::null_mut();
        let result = AXUIElementCopyElementAtPosition(element, point.x as f32, point.y as f32, &mut clicked_element);
        
        if result == 0 && !clicked_element.is_null() {
            // Get the title/role/description of the element
            let mut title_cf: CFStringRef = ptr::null_mut();
            let title_result = AXUIElementCopyAttributeValue(clicked_element, kAXTitleAttribute as CFStringRef, &mut title_cf as *mut _ as *mut _);
            
            let mut role_cf: CFStringRef = ptr::null_mut();
            let role_result = AXUIElementCopyAttributeValue(clicked_element, kAXRoleAttribute as CFStringRef, &mut role_cf as *mut _ as *mut _);
            
            let mut description_cf: CFStringRef = ptr::null_mut();
            let description_result = AXUIElementCopyAttributeValue(clicked_element, kAXDescriptionAttribute as CFStringRef, &mut description_cf as *mut _ as *mut _);
            
            let title = if title_result == 0 && !title_cf.is_null() {
                CFString::wrap_under_create_rule(title_cf).to_string()
            } else {
                String::from("")
            };
            
            let role = if role_result == 0 && !role_cf.is_null() {
                CFString::wrap_under_create_rule(role_cf).to_string()
            } else {
                String::from("")
            };
            
            let description = if description_result == 0 && !description_cf.is_null() {
                CFString::wrap_under_create_rule(description_cf).to_string()
            } else {
                String::from("")
            };
            
            // Create click event with enriched element info
            let click_event = ClickEvent {
                x: point.x,
                y: point.y,
                element: ElementInfo {
                    role,
                    title,
                    description,
                },
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            
            // Emit event to frontend
            if !app_handle_ptr.is_null() {
                let app_handle = &*app_handle_ptr;
                let _ = app_handle.emit("ghost:click-captured", click_event);
            }
            
            AXUIElementRelease(clicked_element);
        }
        
        AXUIElementRelease(element);
    }
    
    // Return the event to let it propagate
    event_ref
}

pub fn check_permissions() -> bool {
    unsafe {
        let options = create_options_dict(false);
        AXIsProcessTrustedWithOptions(options)
    }
}

pub fn request_permissions() -> bool {
    unsafe {
        let options = create_options_dict(true);
        AXIsProcessTrustedWithOptions(options)
    }
}

unsafe fn create_options_dict(prompt: bool) -> *const CFDictionary {
    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let value = CFString::new(if prompt { "1" } else { "0" });
    
    let keys: [CFType; 1] = [key.as_CFType()];
    let values: [CFType; 1] = [value.as_CFType()];
    
    let dict = core_foundation::dictionary::CFDictionary::from_CFType_pairs(&[(keys[0].clone(), values[0].clone())]);
    dict.as_concrete_TypeRef() as *const CFDictionary
}

pub fn start_event_tap(handle: AppHandle) -> Result<(), String> {
    if EVENT_TAP.get().is_some() {
        return Err("Event tap already running".into());
    }

    unsafe {
        // Create the event tap - we need to pass the app handle as context
        let events_of_interest = kAXEventMouseDown | kAXEventMouseUp | kAXEventMouseMoved;
        
        // Box the handle to keep it alive and get a raw pointer
        let handle_box = Box::new(handle);
        let handle_ptr = Box::into_raw(handle_box);
        
        let tap = AXEventTapCreate(kAXSessionID, events_of_interest, kAXEventTapListenOnly, event_tap_callback, handle_ptr as *mut c_void);
        
        if tap.is_null() {
            // Clean up the leaked handle
            let _ = Box::from_raw(handle_ptr);
            return Err("Failed to create event tap".into());
        }

        let wrapper = EventTapRefWrapper { tap };
        
        // Store the tap
        if EVENT_TAP.set(wrapper).is_err() {
            AXEventTapInvalidate(tap);
            AXEventTapRelease(tap);
            let _ = Box::from_raw(handle_ptr);
            return Err("Failed to store event tap".into());
        }

        // Run the event tap on a separate thread with its own run loop
        thread::spawn(move || {
            unsafe {
                let mode_ref = kCFRunLoopDefaultMode as CFStringRef;
                
                // Get the current thread's run loop (not main, we're in a spawned thread)
                let run_loop = CFRunLoopGetMain();
                
                // Get the mach port from the tap and create a run loop source
                let mach_port = AXEventTapGetMachPort(tap);
                
                let mut context = CFRunLoopSourceContext {
                    version: 0,
                    info: handle_ptr as *mut c_void,
                    retain: None,
                    release: Some(|info| {
                        let _ = Box::from_raw(info as *mut AppHandle);
                    }),
                    copyDescription: None,
                    equal: None,
                    hash: None,
                    schedule: None,
                    cancel: None,
                    perform: None,
                };
                
                let source = CFRunLoopSourceCreate(ptr::null(), 0, &mut context);
                CFRunLoopAddSource(run_loop, source, mode_ref);
                
                // Enable the tap
                AXEventTapEnable(tap, true);
                
                // Run the run loop
                CFRunLoopRunInMode(mode_ref, 0.0, false);
            }
        });

        Ok(())
    }
}

pub fn stop_event_tap() {
    if let Some(wrapper) = EVENT_TAP.get() {
        unsafe {
            AXEventTapInvalidate(wrapper.tap);
        }
    }
    // Take ownership to trigger Drop
    let _ = EVENT_TAP.take();
}

pub fn click_at(x: f64, y: f64) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use enigo::{Enigo, Mouse, Settings, Coordinate};
        
        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("Failed to initialize enigo: {}", e))?;
        
        // Move to position
        enigo.move_mouse(x as i32, y as i32, Coordinate::Abs)
            .map_err(|e| format!("Failed to move mouse: {}", e))?;
        
        // Click
        enigo.button(enigo::Button::Left, enigo::Direction::Click)
            .map_err(|e| format!("Failed to click: {}", e))?;
        
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("macOS only".into())
    }
}
