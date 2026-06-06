//! Windows backend implementation using Win32 hooks, UIA, and enigo.

use crate::core::events::{ElementInfo, InputEvent, KeyAction};
use crate::core::traits::{ElementLocator, InputRecorder, ReplayEngine};
use enigo::{Enigo, MouseButton, MouseControllable};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::thread;

// Win32 types
type HWND = isize;
type LPARAM = isize;
type WPARAM = usize;
type LRESULT = isize;
type HHOOK = isize;
type DWORD = u32;
type WORD = u16;

// Hook constants
const WH_MOUSE_LL: i32 = 14;
const WH_KEYBOARD_LL: i32 = 13;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;

// Hook procedure type
type HOOKPROC = unsafe extern "system" fn(code: i32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;

// External Win32 functions
#[link(name = "user32")]
extern "system" {
    fn SetWindowsHookExA(idHook: i32, lpfn: HOOKPROC, hmod: *mut c_void, dwThreadId: DWORD) -> HHOOK;
    fn UnhookWindowsHookEx(hhk: HHOOK) -> bool;
    fn CallNextHookEx(hhk: HHOOK, nCode: i32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn GetModuleHandleA(lpModuleName: *const u8) -> *mut c_void;
    fn GetMessageA(lpMsg: *mut c_void, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
    fn TranslateMessage(lpMsg: *const c_void) -> bool;
    fn DispatchMessageA(lpMsg: *const c_void) -> LRESULT;
    fn GetCursorPos(lpPoint: *mut POINT) -> bool;
}

#[repr(C)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
struct MSLLHOOKSTRUCT {
    pt: POINT,
    mouseData: DWORD,
    flags: DWORD,
    time: DWORD,
    dwExtraInfo: usize,
}

#[repr(C)]
struct KBDLLHOOKSTRUCT {
    vkCode: DWORD,
    scanCode: DWORD,
    flags: DWORD,
    time: DWORD,
    dwExtraInfo: usize,
}

/// Windows-specific backend providing recorder, locator, and replayer implementations.
pub struct WindowsBackend;

// State for managing hooks
struct HookState {
    mouse_hook: Option<HHOOK>,
    keyboard_hook: Option<HHOOK>,
    is_running: Arc<AtomicBool>,
}

impl WindowsBackend {
    pub fn new() -> Self {
        WindowsBackend
    }

    /// Returns a boxed input recorder for Windows.
    pub fn recorder() -> Box<dyn InputRecorder> {
        Box::new(WindowsRecorder::new())
    }

    /// Returns a boxed element locator for Windows.
    pub fn locator() -> Box<dyn ElementLocator> {
        Box::new(WindowsLocator)
    }

    /// Returns a boxed replay engine for Windows.
    pub fn replayer() -> Box<dyn ReplayEngine> {
        Box::new(WindowsReplayer)
    }
}

/// Windows event recorder using SetWindowsHookEx.
struct WindowsRecorder {
    state: Arc<Mutex<Option<HookState>>>,
}

impl WindowsRecorder {
    fn new() -> Self {
        WindowsRecorder {
            state: Arc::new(Mutex::new(None)),
        }
    }
}

unsafe impl Send for WindowsRecorder {}
unsafe impl Sync for WindowsRecorder {}

impl InputRecorder for WindowsRecorder {
    fn start(&self, tx: mpsc::Sender<InputEvent>) -> anyhow::Result<()> {
        let state_clone = self.state.clone();
        let is_running = Arc::new(AtomicBool::new(true));
        
        // Wrap tx in Arc<Mutex<>> for sharing between hooks
        let tx_arc = Arc::new(Mutex::new(tx));
        let tx_mouse = tx_arc.clone();
        let tx_keyboard = tx_arc;

        thread::spawn(move || {
            unsafe {
                let h_instance = GetModuleHandleA(std::ptr::null());

                // Create mouse hook
                let mouse_hook = SetWindowsHookExA(
                    WH_MOUSE_LL,
                    Some(mouse_hook_proc),
                    h_instance,
                    0,
                );

                // Create keyboard hook
                let keyboard_hook = SetWindowsHookExA(
                    WH_KEYBOARD_LL,
                    Some(keyboard_hook_proc),
                    h_instance,
                    0,
                );

                if mouse_hook == 0 || keyboard_hook == 0 {
                    eprintln!("Failed to create Windows hooks");
                    return;
                }

                *state_clone.lock().unwrap() = Some(HookState {
                    mouse_hook: Some(mouse_hook),
                    keyboard_hook: Some(keyboard_hook),
                    is_running: is_running.clone(),
                });

                // Store tx_arc globally for hook procedures (simplified - would need proper global storage)
                // For now, we'll use a simplified approach

                // Run message loop
                let mut msg = std::mem::zeroed();
                while is_running.load(Ordering::Relaxed) {
                    let result = GetMessageA(&mut msg, 0, 0, 0);
                    if result <= 0 {
                        break;
                    }
                    TranslateMessage(&msg);
                    DispatchMessageA(&msg);
                }

                // Cleanup
                UnhookWindowsHookEx(mouse_hook);
                UnhookWindowsHookEx(keyboard_hook);
            }
        });

        Ok(())
    }

    fn stop(&self) {
        if let Some(mut state) = self.state.lock().unwrap().take() {
            state.is_running.store(false, Ordering::Relaxed);
            
            unsafe {
                if let Some(hook) = state.mouse_hook {
                    UnhookWindowsHookEx(hook);
                }
                if let Some(hook) = state.keyboard_hook {
                    UnhookWindowsHookEx(hook);
                }
            }
        }
    }
}

// Global state for hook callbacks (simplified - production would use proper synchronization)
static mut GLOBAL_TX: Option<Arc<Mutex<mpsc::Sender<InputEvent>>>> = None;

unsafe extern "system" fn mouse_hook_proc(code: i32, wParam: WPARAM, lParam: LPARAM) -> LRESULT {
    if code >= 0 {
        if let Some(tx_arc) = &GLOBAL_TX {
            if let Ok(tx_guard) = tx_arc.lock() {
                let mouse_struct = &*(lParam as *const MSLLHOOKSTRUCT);
                
                match wParam as u32 {
                    WM_LBUTTONDOWN | WM_LBUTTONUP => {
                        let button = if wParam as u32 == WM_LBUTTONDOWN { 0 } else { 1 };
                        let event = InputEvent::MouseClick {
                            x: mouse_struct.pt.x,
                            y: mouse_struct.pt.y,
                            button,
                            element: None,
                        };
                        let _ = tx_guard.send(event);
                    }
                    WM_RBUTTONDOWN | WM_RBUTTONUP => {
                        let button = if wParam as u32 == WM_RBUTTONDOWN { 2 } else { 3 };
                        let event = InputEvent::MouseClick {
                            x: mouse_struct.pt.x,
                            y: mouse_struct.pt.y,
                            button,
                            element: None,
                        };
                        let _ = tx_guard.send(event);
                    }
                    _ => {}
                }
            }
        }
    }
    
    CallNextHookEx(0, code, wParam, lParam)
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wParam: WPARAM, lParam: LPARAM) -> LRESULT {
    if code >= 0 {
        if let Some(tx_arc) = &GLOBAL_TX {
            if let Ok(tx_guard) = tx_arc.lock() {
                let kbd_struct = &*(lParam as *const KBDLLHOOKSTRUCT);
                
                match wParam as u32 {
                    WM_KEYDOWN => {
                        let event = InputEvent::Key {
                            code: kbd_struct.vkCode as u16,
                            chars: String::new(), // TODO: Get actual character
                            modifiers: 0,         // TODO: Extract modifier state
                            action: KeyAction::Down,
                        };
                        let _ = tx_guard.send(event);
                    }
                    WM_KEYUP => {
                        let event = InputEvent::Key {
                            code: kbd_struct.vkCode as u16,
                            chars: String::new(),
                            modifiers: 0,
                            action: KeyAction::Up,
                        };
                        let _ = tx_guard.send(event);
                    }
                    _ => {}
                }
            }
        }
    }
    
    CallNextHookEx(0, code, wParam, lParam)
}

/// Windows element locator using UI Automation (UIA).
struct WindowsLocator;

impl ElementLocator for WindowsLocator {
    fn inspect_at(&self, x: i32, y: i32) -> anyhow::Result<Option<ElementInfo>> {
        // TODO: Implement proper UIA element lookup
        // This requires COM initialization and IUIAutomation interface
        // For now, return a stub with coordinates
        
        Ok(Some(ElementInfo {
            role: String::from("UIA Element"),
            name: String::from("TODO: UIA Name"),
            app: String::from("TODO: UIA Application"),
            fallback_coords: Some((x, y)),
        }))
    }
}

/// Windows replay engine using enigo.
struct WindowsReplayer {
    speed_factor: Arc<Mutex<f32>>,
}

impl WindowsReplayer {
    fn new() -> Self {
        WindowsReplayer {
            speed_factor: Arc::new(Mutex::new(1.0)),
        }
    }
    
    /// Set playback speed factor (1.0 = normal, 2.0 = 2x speed, etc.)
    fn set_speed(&self, factor: f32) {
        *self.speed_factor.lock().unwrap() = factor.max(0.1);
    }
}

impl ReplayEngine for WindowsReplayer {
    fn execute(&self, events: &[InputEvent], stop_flag: Arc<AtomicBool>) -> anyhow::Result<()> {
        let mut enigo = Enigo::new();
        let speed = *self.speed_factor.lock().unwrap();

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
                    // Apply speed factor to delay
                    let adjusted_ms = (*ms as f32 / speed) as u64;
                    std::thread::sleep(std::time::Duration::from_millis(adjusted_ms));
                }
            }
        }

        Ok(())
    }
}
