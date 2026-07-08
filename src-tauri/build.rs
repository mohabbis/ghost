fn main() {
    if std::env::var("CARGO_CFG_TEST").is_ok() {
        return;
    }
    tauri_build::build()
}
