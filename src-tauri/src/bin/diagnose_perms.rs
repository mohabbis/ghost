//! macOS-only diagnostic: reports whether this process currently holds the
//! Accessibility and Input Monitoring permissions. Not built on other
//! platforms — `accessibility-sys` and the IOKit framework link are only
//! declared as dependencies under `target_os = "macos"` in Cargo.toml.

#[cfg(target_os = "macos")]
#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOHIDCheckAccess(request_type: u32) -> u32;
}

#[cfg(target_os = "macos")]
const kIOHIDRequestTypeListenEvent: u32 = 1;
#[cfg(target_os = "macos")]
const kIOHIDAccessTypeGranted: u32 = 0;

#[cfg(target_os = "macos")]
fn main() {
    let acc = unsafe { accessibility_sys::AXIsProcessTrusted() };
    let im = unsafe { IOHIDCheckAccess(kIOHIDRequestTypeListenEvent) == kIOHIDAccessTypeGranted };
    println!(
        "DIAGNOSTIC: Accessibility = {}, Input Monitoring = {}",
        acc, im
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("diagnose_perms is only supported on macOS");
}
