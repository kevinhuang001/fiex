//! GTK must keep the URI provider alive until the destination finishes reading.
//! `drop-performed` only starts the transfer; `drag-end` owns final cleanup.
use gtk::{gdk, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tauri::{ipc::Channel, Window};

pub async fn start(
    window: Window,
    paths: Vec<String>,
    on_event: Channel<String>,
) -> Result<(), String> {
    if paths.is_empty() {
        return Err("请选择要拖动的文件".into());
    }
    let uris = paths
        .iter()
        .map(|path| {
            reqwest::Url::from_file_path(path)
                .map(|uri| uri.to_string())
                .map_err(|_| "拖动文件必须使用绝对路径".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let main = window.clone();
    window
        .run_on_main_thread(move || {
            let result = begin(&main, uris, on_event);
            let _ = send.send(result);
        })
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || receive.recv().map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())??
}

fn begin(window: &Window, uris: Vec<String>, on_event: Channel<String>) -> Result<(), String> {
    let window = window.gtk_window().map_err(|e| e.to_string())?;
    let action = gdk::DragAction::COPY;
    window.drag_source_set(gdk::ModifierType::BUTTON1_MASK, &[], action);
    window.drag_source_add_uri_targets();
    let targets = window
        .drag_source_get_target_list()
        .ok_or("无法创建拖动目标")?;
    let handlers: Rc<RefCell<Vec<glib::SignalHandlerId>>> = Rc::default();
    let provided = Rc::new(Cell::new(false));
    let failed = Rc::new(Cell::new(false));
    let performed = Rc::new(Cell::new(false));
    let diagnostic = std::env::var_os("FIEX_DRAG_DIAGNOSTICS").is_some();
    if diagnostic {
        eprintln!("GTK drag: start");
    }
    let sent = provided.clone();
    handlers
        .borrow_mut()
        .push(window.connect_drag_data_get(move |_, _, data, _, _| {
            let refs = uris.iter().map(String::as_str).collect::<Vec<_>>();
            if diagnostic {
                eprintln!("GTK drag: data requested");
            }
            data.set_uris(&refs);
            sent.set(true);
        }));
    let failure = failed.clone();
    handlers
        .borrow_mut()
        .push(window.connect_drag_failed(move |_, _, _| {
            if diagnostic {
                eprintln!("GTK drag: failed");
            }
            failure.set(true);
            glib::Propagation::Proceed
        }));
    let cleanup = handlers.clone();
    let ended = performed.clone();
    handlers
        .borrow_mut()
        .push(window.connect_drag_end(move |widget, _| {
            let result = if provided.get() && ended.get() && !failed.get() {
                "Dropped"
            } else {
                "Cancelled"
            };
            if diagnostic {
                eprintln!(
                    "GTK drag: end provided={} performed={} failed={}",
                    provided.get(),
                    ended.get(),
                    failed.get()
                );
            }
            let _ = on_event.send(result.to_string());
            for handler in cleanup.borrow_mut().drain(..) {
                widget.disconnect(handler);
            }
            widget.drag_source_unset();
        }));
    if let Some(context) = window.drag_begin_with_coordinates(&targets, action, 1, None, -1, -1) {
        context.connect_drop_performed(move |_, _| {
            if diagnostic {
                eprintln!("GTK drag: drop performed");
            }
            performed.set(true)
        });
        context.drag_set_icon_name("text-x-generic", 0, 0);
        Ok(())
    } else {
        for handler in handlers.borrow_mut().drain(..) {
            window.disconnect(handler);
        }
        window.drag_source_unset();
        Err("无法开始文件拖动".into())
    }
}
