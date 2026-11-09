use std::{ffi::c_void, path::Path};
use windows::{
    Win32::{
        System::Com::{CoTaskMemFree, IDataObject},
        UI::Shell::{
            ASSOC_FILTER_NONE, ASSOCF_INIT_DEFAULTTOSTAR, ASSOCSTR_PROGID, AssocQueryStringW,
            IAssocHandler, IEnumAssocHandlers, SHAssocEnumHandlers, SHCreateDataObject,
            SHParseDisplayName,
        },
    },
    core::{PCWSTR, PWSTR},
};

use super::{OpenWithCandidate, apps_folder::take_string, run_sta, wide};

pub(super) fn candidates(extension: &str) -> Result<Vec<OpenWithCandidate>, String> {
    let extension = normalize_extension(extension);
    if extension.is_empty() {
        return Ok(Vec::new());
    }
    run_sta(move || unsafe { enumerate_handlers(&extension) })
}

pub(super) fn open(file: &Path, extension: &str, application_id: &str) -> Result<(), String> {
    let file = file.to_path_buf();
    let extension = normalize_extension(extension);
    let application_id = application_id.to_owned();
    run_sta(move || unsafe { invoke_handler(&file, &extension, &application_id) })
}

fn normalize_extension(extension: &str) -> String {
    let value = extension.trim().trim_start_matches('.');
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return String::new();
    }
    format!(".{value}")
}

unsafe fn enumerate_handlers(extension: &str) -> Result<Vec<OpenWithCandidate>, String> {
    // SAFETY: The extension buffer is NUL-terminated and stays alive during the Shell call; returned COM interfaces remain owned while queried.
    unsafe {
        let wide = wide(extension);
        let default_id = query_default_handler(&wide);
        let handlers = SHAssocEnumHandlers(PCWSTR(wide.as_ptr()), ASSOC_FILTER_NONE)
            .map_err(|error| error.to_string())?;
        let mut output = Vec::new();
        while let Some(handler) = next_handler(&handlers) {
            let Some(id) = handler_string(&handler, false) else {
                continue;
            };
            let name = handler_string(&handler, true).unwrap_or_else(|| id.clone());
            if name.trim().is_empty() {
                continue;
            }
            let icon_source = handler_icon(&handler);
            let is_recommended = handler.IsRecommended().is_ok();
            if output
                .iter()
                .any(|candidate: &OpenWithCandidate| candidate.id == id)
            {
                continue;
            }
            output.push(OpenWithCandidate {
                is_default: default_id.as_deref() == Some(&id),
                is_recommended,
                icon_source,
                id,
                name,
            });
        }
        output.sort_by(|left, right| {
            right
                .is_default
                .cmp(&left.is_default)
                .then_with(|| right.is_recommended.cmp(&left.is_recommended))
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        Ok(output)
    }
}

unsafe fn query_default_handler(extension: &[u16]) -> Option<String> {
    // SAFETY: extension is a live NUL-terminated UTF-16 buffer; the output buffer remains allocated for each AssocQueryStringW call.
    unsafe {
        let mut required = 0;
        let status = AssocQueryStringW(
            ASSOCF_INIT_DEFAULTTOSTAR,
            ASSOCSTR_PROGID,
            PCWSTR(extension.as_ptr()),
            PCWSTR::null(),
            None,
            &mut required,
        );
        if required == 0 || status.is_err() {
            return None;
        }
        let mut buffer = vec![0_u16; required as usize];
        let status = AssocQueryStringW(
            ASSOCF_INIT_DEFAULTTOSTAR,
            ASSOCSTR_PROGID,
            PCWSTR(extension.as_ptr()),
            PCWSTR::null(),
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut required,
        );
        status.ok().ok()?;
        Some(String::from_utf16_lossy(
            &buffer[..required.saturating_sub(1) as usize],
        ))
    }
}

unsafe fn next_handler(handlers: &IEnumAssocHandlers) -> Option<IAssocHandler> {
    // SAFETY: handlers is a live COM interface reference and the COM enumerator initializes the optional output interface.
    unsafe {
        let mut next = [None];
        let mut fetched = 0;
        handlers
            .Next(&mut next, Some(&mut fetched))
            .ok()
            .filter(|_| fetched == 1)?;
        next.into_iter().flatten().next()
    }
}

unsafe fn handler_string(handler: &IAssocHandler, display: bool) -> Option<String> {
    // SAFETY: handler is a live COM interface reference; the returned COM-allocated string is transferred to take_string for conversion and release.
    unsafe {
        let value = if display {
            handler.GetUIName().ok()?
        } else {
            handler.GetName().ok()?
        };
        take_string(value)
    }
}

unsafe fn handler_icon(handler: &IAssocHandler) -> Option<String> {
    // SAFETY: handler is a live COM interface reference; GetIconLocation initializes an owned Shell string that take_string converts and frees.
    unsafe {
        let mut icon = PWSTR::null();
        let mut index = 0;
        handler.GetIconLocation(&mut icon, &mut index).ok()?;
        let path = take_string(icon)?;
        Some(format!("{path},{index}"))
    }
}

unsafe fn invoke_handler(file: &Path, extension: &str, application_id: &str) -> Result<(), String> {
    // SAFETY: All UTF-16 buffers outlive their Shell calls; the PIDL returned by SHParseDisplayName is freed exactly once after data-object creation and invocation.
    unsafe {
        let candidates = enumerate_handlers(extension)?;
        if !candidates
            .iter()
            .any(|candidate| candidate.id == application_id)
        {
            return Err(
                "The selected application is no longer associated with this file type".into(),
            );
        }
        let wide_extension = wide(extension);
        let handlers = SHAssocEnumHandlers(PCWSTR(wide_extension.as_ptr()), ASSOC_FILTER_NONE)
            .map_err(|error| error.to_string())?;
        let mut selected = None;
        while let Some(handler) = next_handler(&handlers) {
            if handler_string(&handler, false).as_deref() == Some(application_id) {
                selected = Some(handler);
                break;
            }
        }
        let handler = selected.ok_or("The selected application is no longer registered")?;
        let path = wide(&file.to_string_lossy());
        let mut item = std::ptr::null_mut();
        SHParseDisplayName(PCWSTR(path.as_ptr()), None, &mut item, 0, None)
            .map_err(|error| error.to_string())?;
        if item.is_null() {
            return Err("Windows could not resolve the selected file".into());
        }
        let item_list = [item.cast_const()];
        let data: IDataObject = match SHCreateDataObject(None, Some(&item_list), None) {
            Ok(value) => value,
            Err(error) => {
                CoTaskMemFree(Some(item.cast::<c_void>()));
                return Err(error.to_string());
            }
        };
        let result = handler.Invoke(&data).map_err(|error| error.to_string());
        CoTaskMemFree(Some(item.cast::<c_void>()));
        result
    }
}
