#[path = "../plugin_entry.rs"]
mod plugin_entry;
fn main() {
    plugin_entry::run("webdav");
}
