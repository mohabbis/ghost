//! macOS backend implementation using CGEventTap, AXUIElement, and enigo.

use crate::core::events::{ElementInfo, InputEvent};
use crate::core::traits::{ElementLocator, InputRecorder, ReplayEngine};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::mpsc;

/// macOS-specific backend providing recorder, locator, and replayer implementations.
pub struct MacosBackend;

impl MacosBackend {
    pub fn new() -> Self {
        MacosBackend
    }

    /// Returns a boxed input recorder for macOS.
    pub fn recorder() -> Box<dyn InputRecorder> {
        Box::new(MacosRecorder)
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
struct MacosRecorder;

impl InputRecorder for MacosRecorder {
    fn start(&self, _tx: mpsc::Sender<InputEvent>) -> anyhow::Result<()> {
        // TODO: Implement CGEventTap → mpsc bridge
        // 1. Create CGEventTap for HID events (LeftMouseDown, KeyDown, etc.)
        // 2. On each event, construct InputEvent variant
        // 3. Send via tx.send(event)
        // 4. Run on dedicated thread with CFRunLoop
        // 5. Store runloop handle for stop()
        anyhow::bail!("TODO: CGEventTap implementation pending")
    }

    fn stop(&self) {
        // TODO: Signal the CGEventTap runloop to stop
        // Use stored CFRunLoopRef to call CFRunLoopStop()
    }
}

/// macOS element locator using AXUIElement / Accessibility API.
struct MacosLocator;

impl ElementLocator for MacosLocator {
    fn inspect_at(&self, _x: i32, _y: i32) -> anyhow::Result<Option<ElementInfo>> {
        // TODO: Implement AXUIElement lookup at screen coordinates
        // 1. Use AXUIElementCopyElementAtPosition to find element
        // 2. Extract role (AXRole), name (AXTitle/AXValue), app (AXApplication)
        // 3. Return ElementInfo with fallback_coords if needed
        Ok(Some(ElementInfo {
            role: String::from("TODO: AXRole"),
            name: String::from("TODO: AXTitle"),
            app: String::from("TODO: AXApplication"),
            fallback_coords: Some((_x, _y)),
        }))
    }
}

/// macOS replay engine using enigo.
struct MacosReplayer;

impl ReplayEngine for MacosReplayer {
    fn execute(&self, _events: &[InputEvent], stop_flag: Arc<AtomicBool>) -> anyhow::Result<()> {
        // TODO: Implement enigo-based replay with stop check
        // 1. Iterate through events slice
        // 2. Check stop_flag.load(Ordering::Relaxed) before each event
        // 3. For MouseClick: enigo.move_mouse + enigo.button_click
        // 4. For Key: enigo.key_press / enigo.key_release
        // 5. For Scroll: enigo.scroll
        // 6. For Delay: std::thread::sleep
        // 7. Return error if any action fails
        for _event in _events {
            if stop_flag.load(Ordering::Relaxed) {
                return Ok(());
            }
            // TODO: Replace with actual enigo calls
        }
        Ok(())
    }
}
