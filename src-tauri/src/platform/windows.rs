//! Windows backend implementation using Win32 hooks, UIA, and enigo.

use crate::core::events::{ElementInfo, InputEvent, KeyAction};
use crate::core::traits::{ElementLocator, InputRecorder, ReplayEngine};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

// Win32 types
type HWND = isize;
type LPARAM = isize;
type WPARAM = usize;
type LRESULT = isize;
type HHOOK = isize;
type DWORD = u32;

// Hook constants
const WH_MOUSE_LL: i32 = 14;
const WH_KEYBOARD_LL: i32 = 13;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_MOUSEHWHEEL: u32 = 0x020E;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;

// Virtual key codes for modifier detection
const VK_SHIFT: i32 = 0x10;
const VK_CONTROL: i32 = 0x11;
const VK_MENU: i32 = 0x12; // Alt
const VK_LWIN: i32 = 0x5B;
const VK_RWIN: i32 = 0x5C;

// GetAncestor flags
const GA_ROOTOWNER: u32 = 3;

type HOOKPROC = unsafe extern "system" fn(code: i32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;

#[link(name = "user32")]
extern "system" {
    fn SetWindowsHookExA(
        idHook: i32,
        lpfn: HOOKPROC,
        hmod: *mut c_void,
        dwThreadId: DWORD,
    ) -> HHOOK;
    fn UnhookWindowsHookEx(hhk: HHOOK) -> bool;
    fn CallNextHookEx(hhk: HHOOK, nCode: i32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn GetModuleHandleA(lpModuleName: *const u8) -> *mut c_void;
    fn GetMessageA(lpMsg: *mut c_void, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
    fn TranslateMessage(lpMsg: *const c_void) -> bool;
    fn DispatchMessageA(lpMsg: *const c_void) -> LRESULT;
    // Modifier and character extraction
    fn GetKeyState(nVirtKey: i32) -> i16;
    fn GetKeyboardState(lpKeyState: *mut u8) -> bool;
    fn ToUnicode(
        wVirtKey: u32,
        wScanCode: u32,
        lpKeyState: *const u8,
        pwszBuff: *mut u16,
        cchBuff: i32,
        wFlags: u32,
    ) -> i32;
    // Element inspection
    fn WindowFromPoint(Point: POINT) -> HWND;
    fn GetClassNameA(hWnd: HWND, lpClassName: *mut u8, nMaxCount: i32) -> i32;
    fn GetWindowTextA(hWnd: HWND, lpString: *mut u8, nMaxCount: i32) -> i32;
    fn GetAncestor(hwnd: HWND, gaFlags: u32) -> HWND;
}

#[repr(C)]
#[derive(Clone, Copy)]
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

impl WindowsBackend {
    pub fn new() -> Self {
        WindowsBackend
    }

    pub fn recorder() -> Box<dyn InputRecorder> {
        Box::new(WindowsRecorder::new())
    }

    pub fn locator() -> Box<dyn ElementLocator> {
        Box::new(WindowsLocator)
    }

    pub fn replayer() -> Box<dyn ReplayEngine> {
        Box::new(WindowsReplayer::new())
    }

    pub fn check_accessibility() -> bool {
        true
    }

    pub fn request_accessibility() -> bool {
        Self::check_accessibility()
    }
}

struct HookState {
    mouse_hook: Option<HHOOK>,
    keyboard_hook: Option<HHOOK>,
    is_running: Arc<AtomicBool>,
}

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
        let tx_arc = Arc::new(Mutex::new(tx));

        thread::spawn(move || {
            unsafe {
                // Set global sender before installing hooks
                GLOBAL_TX = Some(tx_arc.clone());

                let h_instance = GetModuleHandleA(std::ptr::null());

                let mouse_hook = SetWindowsHookExA(WH_MOUSE_LL, mouse_hook_proc, h_instance, 0);
                let keyboard_hook =
                    SetWindowsHookExA(WH_KEYBOARD_LL, keyboard_hook_proc, h_instance, 0);

                if mouse_hook == 0 || keyboard_hook == 0 {
                    eprintln!("Failed to create Windows hooks");
                    GLOBAL_TX = None;
                    return;
                }

                *state_clone.lock().unwrap() = Some(HookState {
                    mouse_hook: Some(mouse_hook),
                    keyboard_hook: Some(keyboard_hook),
                    is_running: is_running.clone(),
                });

                let mut msg = std::mem::zeroed();
                while is_running.load(Ordering::Relaxed) {
                    let result = GetMessageA(&mut msg, 0, 0, 0);
                    if result <= 0 {
                        break;
                    }
                    TranslateMessage(&msg);
                    DispatchMessageA(&msg);
                }

                UnhookWindowsHookEx(mouse_hook);
                UnhookWindowsHookEx(keyboard_hook);
                GLOBAL_TX = None;
            }
        });

        Ok(())
    }

    fn stop(&self) {
        if let Some(state) = self.state.lock().unwrap().take() {
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

static mut GLOBAL_TX: Option<Arc<Mutex<mpsc::Sender<InputEvent>>>> = None;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Pack current Win32 modifier key states into the compact u8 format.
/// Bit layout: 0=Shift 1=Control 2=Alt 3=Win
unsafe fn get_modifier_state() -> u8 {
    let mut m: u8 = 0;
    // GetKeyState bit 15 = 1 when the key is held down
    if GetKeyState(VK_SHIFT) < 0 {
        m |= 0x01;
    }
    if GetKeyState(VK_CONTROL) < 0 {
        m |= 0x02;
    }
    if GetKeyState(VK_MENU) < 0 {
        m |= 0x04;
    }
    if GetKeyState(VK_LWIN) < 0 || GetKeyState(VK_RWIN) < 0 {
        m |= 0x08;
    }
    m
}

/// Translate a virtual key + scan code to the Unicode character it produces,
/// taking the current keyboard layout and modifier state into account.
unsafe fn get_key_char(vk_code: DWORD, scan_code: DWORD) -> String {
    let mut key_state = [0u8; 256];
    if !GetKeyboardState(key_state.as_mut_ptr()) {
        return String::new();
    }
    let mut buf = [0u16; 8];
    let n = ToUnicode(
        vk_code,
        scan_code,
        key_state.as_ptr(),
        buf.as_mut_ptr(),
        8,
        0,
    );
    if n > 0 {
        String::from_utf16_lossy(&buf[..n as usize])
    } else {
        String::new()
    }
}

/// Look up the Win32 window at (x, y) and return element metadata.
/// Role = window class name (e.g. "Button", "Edit"), name = window text,
/// app = root-owner window text.
unsafe fn get_element_at(x: i32, y: i32) -> Option<ElementInfo> {
    let point = POINT { x, y };
    let hwnd = WindowFromPoint(point);
    if hwnd == 0 {
        return None;
    }

    let mut class_buf = [0u8; 256];
    let class_len = GetClassNameA(hwnd, class_buf.as_mut_ptr(), 256);
    let role = if class_len > 0 {
        String::from_utf8_lossy(&class_buf[..class_len as usize]).to_string()
    } else {
        String::from("Window")
    };

    let mut text_buf = [0u8; 512];
    let text_len = GetWindowTextA(hwnd, text_buf.as_mut_ptr(), 512);
    let name = if text_len > 0 {
        String::from_utf8_lossy(&text_buf[..text_len as usize]).to_string()
    } else {
        String::new()
    };

    // Walk to the root-owner window to find the app title
    let root = GetAncestor(hwnd, GA_ROOTOWNER);
    let app = if root != 0 && root != hwnd {
        let mut app_buf = [0u8; 512];
        let app_len = GetWindowTextA(root, app_buf.as_mut_ptr(), 512);
        if app_len > 0 {
            String::from_utf8_lossy(&app_buf[..app_len as usize]).to_string()
        } else {
            String::from("Unknown")
        }
    } else if !name.is_empty() {
        name.clone()
    } else {
        String::from("Unknown")
    };

    Some(ElementInfo {
        role,
        name,
        app,
        fallback_coords: Some((x, y)),
        ..Default::default()
    })
}

// ── Hook callbacks ────────────────────────────────────────────────────────────

unsafe extern "system" fn mouse_hook_proc(code: i32, wParam: WPARAM, lParam: LPARAM) -> LRESULT {
    if code >= 0 {
        if let Some(tx_arc) = &GLOBAL_TX {
            if let Ok(tx) = tx_arc.lock() {
                let ms = &*(lParam as *const MSLLHOOKSTRUCT);

                let event = match wParam as u32 {
                    WM_LBUTTONDOWN => Some(InputEvent::MouseClick {
                        x: ms.pt.x,
                        y: ms.pt.y,
                        button: 0,
                        element: get_element_at(ms.pt.x, ms.pt.y),
                        timestamp: None,
                        retry_count: None,
                        semantic_tag: None,
                        self_heal: None,
                    }),
                    WM_LBUTTONUP => Some(InputEvent::MouseClick {
                        x: ms.pt.x,
                        y: ms.pt.y,
                        button: 1,
                        element: None,
                        timestamp: None,
                        retry_count: None,
                        semantic_tag: None,
                        self_heal: None,
                    }),
                    WM_RBUTTONDOWN => Some(InputEvent::MouseClick {
                        x: ms.pt.x,
                        y: ms.pt.y,
                        button: 2,
                        element: get_element_at(ms.pt.x, ms.pt.y),
                        timestamp: None,
                        retry_count: None,
                        semantic_tag: None,
                        self_heal: None,
                    }),
                    WM_RBUTTONUP => Some(InputEvent::MouseClick {
                        x: ms.pt.x,
                        y: ms.pt.y,
                        button: 3,
                        element: None,
                        timestamp: None,
                        retry_count: None,
                        semantic_tag: None,
                        self_heal: None,
                    }),
                    WM_MOUSEWHEEL => {
                        // HIWORD of mouseData = signed wheel delta; positive = forward (up)
                        let delta = (ms.mouseData >> 16) as i16;
                        let dy = -(delta as i32) / 120; // normalise; negative = scroll up visually
                        Some(InputEvent::Scroll {
                            dx: 0,
                            dy,
                            phase: 0,
                            timestamp: None,
                        })
                    }
                    WM_MOUSEHWHEEL => {
                        let delta = (ms.mouseData >> 16) as i16;
                        let dx = delta as i32 / 120;
                        Some(InputEvent::Scroll {
                            dx,
                            dy: 0,
                            phase: 0,
                            timestamp: None,
                        })
                    }
                    _ => None,
                };

                if let Some(ev) = event {
                    let _ = tx.send(ev);
                }
            }
        }
    }
    CallNextHookEx(0, code, wParam, lParam)
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wParam: WPARAM, lParam: LPARAM) -> LRESULT {
    if code >= 0 {
        if let Some(tx_arc) = &GLOBAL_TX {
            if let Ok(tx) = tx_arc.lock() {
                let kb = &*(lParam as *const KBDLLHOOKSTRUCT);

                let action = match wParam as u32 {
                    WM_KEYDOWN | WM_SYSKEYDOWN => Some(KeyAction::Down),
                    WM_KEYUP | WM_SYSKEYUP => Some(KeyAction::Up),
                    _ => None,
                };

                if let Some(action) = action {
                    let modifiers = get_modifier_state();
                    let chars = match action {
                        KeyAction::Down => get_key_char(kb.vkCode, kb.scanCode),
                        KeyAction::Up => String::new(),
                    };
                    let _ = tx.send(InputEvent::Key {
                        code: kb.vkCode as u16,
                        chars,
                        modifiers,
                        action,
                        timestamp: None,
                        retry_count: None,
                        semantic_tag: None,
                    });
                }
            }
        }
    }
    CallNextHookEx(0, code, wParam, lParam)
}

// ── Element locator ───────────────────────────────────────────────────────────

struct WindowsLocator;

impl ElementLocator for WindowsLocator {
    fn inspect_at(&self, x: i32, y: i32) -> anyhow::Result<Option<ElementInfo>> {
        unsafe { Ok(get_element_at(x, y)) }
    }
}

// ── Replay engine ─────────────────────────────────────────────────────────────

struct WindowsReplayer {
    speed_factor: Arc<Mutex<f32>>,
}

impl WindowsReplayer {
    fn new() -> Self {
        WindowsReplayer {
            speed_factor: Arc::new(Mutex::new(1.0)),
        }
    }

    fn set_speed(&self, factor: f32) {
        *self.speed_factor.lock().unwrap() = factor.max(0.1);
    }
}

impl ReplayEngine for WindowsReplayer {
    fn execute(&self, events: &[InputEvent], stop_flag: Arc<AtomicBool>) -> anyhow::Result<()> {
        let mut enigo = Enigo::new(&Settings::default())?;
        let speed = *self.speed_factor.lock().unwrap();

        for event in events {
            if stop_flag.load(Ordering::Relaxed) {
                return Ok(());
            }

            match event {
                InputEvent::MouseClick {
                    x,
                    y,
                    button,
                    retry_count,
                    ..
                } => {
                    let max_retries = retry_count.unwrap_or(0);
                    let mut attempts = 0;
                    let mut success = false;
                    while attempts <= max_retries && !success {
                        enigo.move_mouse(*x, *y, Coordinate::Abs)?;
                        let btn = match button {
                            0 | 1 => Button::Left,
                            2 | 3 => Button::Right,
                            _ => Button::Left,
                        };
                        enigo.button(btn, Direction::Click)?;
                        success = true;
                        attempts += 1;
                    }
                }
                InputEvent::Key {
                    code,
                    chars,
                    action,
                    retry_count,
                    ..
                } => {
                    let max_retries = retry_count.unwrap_or(0);
                    let mut attempts = 0;
                    while attempts <= max_retries {
                        let key = if !chars.is_empty() {
                            Key::Unicode(chars.chars().next().unwrap_or(' '))
                        } else {
                            Key::Other(*code as u32)
                        };
                        match action {
                            KeyAction::Down => enigo.key(key, Direction::Press)?,
                            KeyAction::Up => enigo.key(key, Direction::Release)?,
                        }
                        attempts += 1;
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
                    std::thread::sleep(std::time::Duration::from_millis(adjusted_ms));
                }
                InputEvent::Wait {
                    condition,
                    timeout_ms,
                    poll_interval_ms,
                } => {
                    tracing::info!("Waiting for condition: {:?}", condition);
                    let locator = crate::platform::windows::WindowsBackend::locator();
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
                } => match crate::core::vision::capture_screenshot() {
                    Ok(img_bytes) => {
                        if let Ok(current_img) = image::load_from_memory(&img_bytes) {
                            if let Ok(similarity) = crate::core::vision::compare_images(
                                baseline_screenshot,
                                &current_img,
                            ) {
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
                                                std::thread::sleep(
                                                    std::time::Duration::from_millis(500),
                                                );
                                                if let Ok(b) =
                                                    crate::core::vision::capture_screenshot()
                                                {
                                                    if let Ok(img) = image::load_from_memory(&b) {
                                                        if crate::core::vision::compare_images(
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
                    let mut var_context = crate::core::wait::VariableContext::new();
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
        reliability: &crate::core::events::ReliabilitySettings,
    ) -> anyhow::Result<()> {
        let mut enigo = Enigo::new(&Settings::default())?;
        let speed = *self.speed_factor.lock().unwrap();

        for event in events {
            if stop_flag.load(Ordering::Relaxed) {
                return Ok(());
            }

            match event {
                InputEvent::MouseClick {
                    x,
                    y,
                    button,
                    retry_count,
                    ..
                } => {
                    let max_retries = retry_count.unwrap_or(reliability.retry_config.max_attempts);
                    let mut attempts = 0;
                    let mut success = false;

                    while attempts <= max_retries && !success {
                        enigo.move_mouse(*x, *y, Coordinate::Abs)?;
                        let btn = match button {
                            0 | 1 => Button::Left,
                            2 | 3 => Button::Right,
                            _ => Button::Left,
                        };
                        enigo.button(btn, Direction::Click)?;
                        success = true;
                        attempts += 1;

                        if !success && attempts <= max_retries && reliability.continue_on_error {
                            let backoff = reliability.retry_config.backoff_ms
                                * (reliability.retry_config.backoff_multiplier as u64)
                                    .pow(attempts - 1);
                            std::thread::sleep(std::time::Duration::from_millis(backoff));
                        }
                    }
                }
                InputEvent::Key {
                    code,
                    chars,
                    action,
                    retry_count,
                    ..
                } => {
                    let max_retries = retry_count.unwrap_or(reliability.retry_config.max_attempts);
                    for attempt in 0..=max_retries {
                        if stop_flag.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        let key = if !chars.is_empty() {
                            Key::Unicode(chars.chars().next().unwrap_or(' '))
                        } else {
                            Key::Other(*code as u32)
                        };
                        match action {
                            KeyAction::Down => enigo.key(key, Direction::Press)?,
                            KeyAction::Up => enigo.key(key, Direction::Release)?,
                        }
                        if attempt < max_retries {
                            std::thread::sleep(std::time::Duration::from_millis(
                                reliability.retry_config.backoff_ms
                                    * reliability.retry_config.backoff_multiplier as u64,
                            ));
                        }
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
                    std::thread::sleep(std::time::Duration::from_millis(adjusted_ms));
                }
                InputEvent::Wait {
                    condition,
                    timeout_ms,
                    poll_interval_ms,
                } => {
                    if reliability.validate_elements {
                        let locator = crate::platform::windows::WindowsBackend::locator();
                        let result = crate::core::wait::wait_for_condition(
                            condition,
                            locator.as_ref(),
                            *timeout_ms,
                            *poll_interval_ms,
                        );
                        match result {
                            crate::core::wait::WaitResult::Error(e) => {
                                if reliability.continue_on_error {
                                    tracing::warn!("Wait failed (continuing): {}", e);
                                } else {
                                    return Err(anyhow::anyhow!("Wait condition failed: {}", e));
                                }
                            }
                            crate::core::wait::WaitResult::Timeout => {
                                tracing::warn!("Wait condition timed out");
                            }
                            crate::core::wait::WaitResult::Success => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}
