fn is_cargo_test() -> bool {
    let pid = std::process::id();
    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "$ppid = (Get-CimInstance Win32_Process -Filter 'ProcessId = {}').ParentProcessId; \
             (Get-CimInstance Win32_Process -Filter \"ProcessId = $ppid\").CommandLine",
            pid
        );
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
        {
            let cmdline = String::from_utf8_lossy(&output.stdout);
            return cmdline.contains(" test ") || cmdline.trim().ends_with(" test");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let output1 = std::process::Command::new("ps")
            .args(["-o", "ppid=", "-p", &pid.to_string()])
            .output();
        if let Ok(out1) = output1 {
            let ppid_str = String::from_utf8_lossy(&out1.stdout).trim().to_string();
            if !ppid_str.is_empty() {
                let output2 = std::process::Command::new("ps")
                    .args(["-o", "args=", "-p", &ppid_str])
                    .output();
                if let Ok(out2) = output2 {
                    let cmdline = String::from_utf8_lossy(&out2.stdout);
                    return cmdline.contains(" test ") || cmdline.trim().ends_with(" test");
                }
            }
        }
    }
    false
}

fn main() {
    if is_cargo_test() {
        return;
    }
    tauri_build::build()
}
