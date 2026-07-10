/// Best-effort detection of a `cargo test` (or `cargo bench`) build.
///
/// Build scripts are never compiled with the `test` cfg, so cargo gives them no
/// direct "this is a test build" signal. We instead walk the process ancestry and
/// look for the driving `cargo test`/`cargo bench` invocation. The whole chain is
/// inspected (not just the immediate parent) because the build-script process is
/// not always a direct child of the top-level cargo process.
fn is_cargo_test() -> bool {
    let pid = std::process::id();

    #[cfg(target_os = "windows")]
    {
        // Single PowerShell pass: walk up to a bounded number of ancestors and
        // print "MATCH" if any command line looks like `cargo ... test|bench`.
        let script = format!(
            "$id = {pid}; \
             for ($i = 0; $i -lt 12 -and $id) {{ \
               $p = Get-CimInstance Win32_Process -Filter \"ProcessId = $id\" -ErrorAction SilentlyContinue; \
               if (-not $p) {{ break }}; \
               if ($p.CommandLine -and $p.CommandLine -match '(?i)cargo(\\.exe)?[^\\n]*\\b(test|bench)\\b') {{ Write-Output 'MATCH'; break }}; \
               $id = $p.ParentProcessId \
             }}",
            pid = pid
        );
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
        {
            return String::from_utf8_lossy(&output.stdout).contains("MATCH");
        }
        false
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Walk parents via `ps`, matching a `cargo test`/`cargo bench` command line.
        let mut cur = pid;
        for _ in 0..12 {
            let out = std::process::Command::new("ps")
                .args(["-o", "ppid=,args=", "-p", &cur.to_string()])
                .output();
            let Ok(out) = out else { return false };
            let line = String::from_utf8_lossy(&out.stdout);
            let line = line.trim();
            if line.is_empty() {
                return false;
            }
            // `ps` prints "<ppid> <args...>"; split off the ppid, keep the rest.
            let mut parts = line.splitn(2, char::is_whitespace);
            let ppid = parts.next().unwrap_or("").trim();
            let args = parts.next().unwrap_or("");
            if args.contains("cargo") && (args.contains(" test") || args.contains(" bench")) {
                return true;
            }
            match ppid.parse::<u32>() {
                Ok(p) if p != 0 && p != cur => cur = p,
                _ => return false,
            }
        }
        false
    }
}

fn main() {
    // Running tauri_build::build() while compiling the `cargo test` harness produces a
    // Windows test binary that crashes on load (STATUS_ENTRYPOINT_NOT_FOUND, 0xc0000139),
    // so the test build must skip it. Build scripts cannot see the `test` cfg, so CI sets
    // GHOST_SKIP_TAURI_BUILD on the test step (see .github/workflows/rust.yml) as the
    // deterministic signal; is_cargo_test() is a best-effort fallback for local `cargo test`.
    println!("cargo:rerun-if-env-changed=GHOST_SKIP_TAURI_BUILD");
    if std::env::var_os("GHOST_SKIP_TAURI_BUILD").is_some() || is_cargo_test() {
        return;
    }
    tauri_build::build()
}
