// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "mcp" && args[2] == "serve" {
        ghost_lib::mcp::run_stdio();
        return;
    }
    if args.len() >= 4 && args[1] == "mcp" && args[2] == "serve" && args[3] == "http" {
        let port = args
            .get(4)
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8787);
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        ghost_lib::mcp::run_localhost_http(port, stop);
        return;
    }
    ghost_lib::run()
}
