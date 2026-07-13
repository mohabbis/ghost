// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "mcp" && args[2] == "serve" {
        ghost_lib::mcp::run_stdio();
        return;
    }
    ghost_lib::run()
}
