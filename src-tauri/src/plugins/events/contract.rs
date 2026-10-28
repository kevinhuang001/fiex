//! One source for Core event ownership and dispatch semantics.
#[derive(Clone, Copy)]
pub(super) enum Dispatch {
    Merge,
    FirstSupported,
    Owner,
}

pub(super) struct Event {
    pub extension: &'static str,
    pub dispatch: Dispatch,
}

pub(super) fn event(method: &str) -> Option<Event> {
    use Dispatch::*;
    let (extension, dispatch) = match method {
        "info.resolve" => ("info", Merge),
        "preview.open" => ("preview", FirstSupported),
        "decorations.resolve" => ("decorations", Merge),
        "contextMenu.resolve" => ("contextMenus", Merge),
        "contextMenu.execute" => ("contextMenus", Owner),
        "search.query" => ("search", Merge),
        "toolbar.invoke" => ("toolbar", Owner),
        "hooks.invoke" => ("hooks", FirstSupported),
        "connection.open" | "connection.create" | "connection.validate" => ("connections", Owner),
        "connectionContextMenu.resolve" => ("connectionContextMenus", Merge),
        "connectionContextMenu.execute" => ("connectionContextMenus", Owner),
        _ => return None,
    };
    Some(Event {
        extension,
        dispatch,
    })
}
