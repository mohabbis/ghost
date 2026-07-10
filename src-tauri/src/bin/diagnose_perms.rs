#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOHIDCheckAccess(request_type: u32) -> u32;
}

const kIOHIDRequestTypeListenEvent: u32 = 1;
const kIOHIDAccessTypeGranted: u32 = 0;

fn main() {
    let acc = unsafe { accessibility_sys::AXIsProcessTrusted() };
    let im = unsafe { IOHIDCheckAccess(kIOHIDRequestTypeListenEvent) == kIOHIDAccessTypeGranted };
    println!("DIAGNOSTIC: Accessibility = {}, Input Monitoring = {}", acc, im);
}
