#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    fiex_lib::plugin_sandbox::run_wrapper_if_requested();
    fiex_lib::run();
}
