//! Keep WebKit clear above a partially translucent, behind-window AppKit material.
use objc2::{class, msg_send, runtime::AnyObject};

pub fn configure(window: &tauri::WebviewWindow) {
    let (Ok(ns_window), Ok(ns_view)) = (window.ns_window(), window.ns_view()) else {
        return;
    };
    // Integer handles can cross the scheduling boundary; dereference only on the UI thread.
    let window_handle = ns_window as usize;
    let view_handle = ns_view as usize;
    let _ = window.with_webview(move |webview| unsafe {
        let native_window = window_handle as *mut AnyObject;
        let content_view = view_handle as *mut AnyObject;
        let clear: *mut AnyObject = msg_send![class!(NSColor), clearColor];
        let _: () = msg_send![native_window, setOpaque: false];
        let _: () = msg_send![native_window, setBackgroundColor: clear];

        // A white WKWebView canvas would hide native translucency despite transparent CSS.
        let key: *mut AnyObject =
            msg_send![class!(NSString), stringWithUTF8String: c"drawsBackground".as_ptr()];
        let no: *mut AnyObject = msg_send![class!(NSNumber), numberWithBool: false];
        let wk_view = webview.inner() as *mut AnyObject;
        let _: () = msg_send![wk_view, setValue: no forKey: key];

        // This is window-vibrancy's tagged NSVisualEffectView (the material used by Tauri).
        // Keep its blur behind the window and retain its full blur instead of blending sharp desktop details through it.
        let material: *mut AnyObject = msg_send![content_view, viewWithTag: 91376254_isize];
        if !material.is_null() {
            let _: () = msg_send![material, setBlendingMode: 0_isize];
            let _: () = msg_send![material, setState: 1_isize];
            let _: () = msg_send![material, setAlphaValue: 1.0_f64];
        }
    });
}
