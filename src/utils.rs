/// Initialize panic hook and logging for WASM
pub fn init() {
    // Better panic messages in the console
    console_error_panic_hook::set_once();

    // Initialize logging to console (ignore if already initialized)
    let _ = console_log::init_with_level(log::Level::Info);
}
