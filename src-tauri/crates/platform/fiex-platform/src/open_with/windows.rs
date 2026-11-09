use super::OpenWithCandidate;
use std::{
    ffi::c_void,
    path::Path,
    sync::{OnceLock, mpsc},
};
use windows::{
    Win32::{
        Storage::EnhancedStorage::{PKEY_AppUserModel_ID, PKEY_Link_TargetParsingPath},
        System::Com::{
            CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IDataObject,
        },
        UI::Shell::{
            ASSOC_FILTER_NONE, ASSOCF_INIT_DEFAULTTOSTAR, ASSOCSTR_PROGID,
            ApplicationActivationManager, AssocQueryStringW, BHID_EnumItems, FOLDERID_AppsFolder,
            IApplicationActivationManager, IAssocHandler, IEnumAssocHandlers, IEnumShellItems,
            IShellItem, IShellItem2, KF_FLAG_DEFAULT, SHAssocEnumHandlers, SHCreateDataObject,
            SHCreateShellItemArrayFromIDLists, SHGetKnownFolderItem, SHParseDisplayName,
            SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY,
        },
    },
    core::{Interface, PCWSTR, PWSTR},
};

pub(super) struct AppsFolderItem {
    pub name: String,
    pub aumid: String,
    pub path: String,
    pub parsing_name: String,
}

type Job = Box<dyn FnOnce(bool) + Send + 'static>;

fn worker() -> &'static mpsc::SyncSender<Job> {
    static WORKER: OnceLock<mpsc::SyncSender<Job>> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel::<Job>(32);
        std::thread::Builder::new()
            .name("fiex-windows-shell-sta".into())
            .spawn(move || {
                let initialized = unsafe {
                    CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)
                        .ok()
                        .is_ok()
                };
                while let Ok(job) = receiver.recv() {
                    job(initialized);
                }
                if initialized {
                    unsafe { CoUninitialize() };
                }
            })
            .expect("Windows Shell STA worker could not start");
        sender
    })
}

fn run_sta<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (done, result) = mpsc::channel();
    worker()
        .send(Box::new(move |initialized| {
            let value = if initialized {
                operation()
            } else {
                Err("Could not initialize the Windows Shell STA worker".into())
            };
            let _ = done.send(value);
        }))
        .map_err(|_| "Windows Shell STA worker stopped".to_owned())?;
    result
        .recv()
        .map_err(|_| "Windows Shell STA worker stopped".to_owned())?
}

pub(super) fn apps_folder() -> Result<Vec<AppsFolderItem>, String> {
    run_sta(|| unsafe { enumerate_apps_folder() })
}

pub(super) fn applications() -> Result<Vec<OpenWithCandidate>, String> {
    let entries = apps_folder()?;
    let mut output = Vec::new();
    for app in entries {
        let aumid = if app.aumid.is_empty() {
            app.parsing_name
                .strip_prefix("shell:AppsFolder\\")
                .unwrap_or_default()
                .to_owned()
        } else {
            app.aumid
        };
        let (id, icon_source) = if aumid.contains('!') {
            (
                format!("aumid:{aumid}"),
                format!("shell:AppsFolder\\{aumid}"),
            )
        } else if Path::new(&app.path).is_file() {
            (format!("path:{}", app.path), app.path.clone())
        } else {
            continue;
        };
        if output
            .iter()
            .any(|candidate: &OpenWithCandidate| candidate.id == id)
        {
            continue;
        }
        output.push(OpenWithCandidate {
            id,
            name: app.name,
            icon_source: Some(icon_source),
            is_default: false,
            is_recommended: false,
        });
    }
    output.sort_by_key(|candidate| candidate.name.to_lowercase());
    Ok(output)
}

pub(super) fn open_application(file: &Path, application_id: &str) -> Result<(), String> {
    let installed = applications()?;
    if !installed
        .iter()
        .any(|candidate| candidate.id == application_id)
    {
        return Err("The selected application is no longer installed".into());
    }
    if let Some(aumid) = application_id.strip_prefix("aumid:") {
        let file = file.to_path_buf();
        let aumid = aumid.to_owned();
        return run_sta(move || unsafe { activate_package_for_file(&file, &aumid) });
    }
    let executable = application_id
        .strip_prefix("path:")
        .ok_or("Invalid Windows application identity")?;
    use std::os::windows::process::CommandExt;
    std::process::Command::new(executable)
        .arg(file)
        .creation_flags(0x0800_0000)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

unsafe fn activate_package_for_file(file: &Path, aumid: &str) -> Result<(), String> {
    // SAFETY: The Shell pointers are derived from live UTF-16 buffers or returned by the preceding Shell call, and the COM objects remain owned for the duration of each call.
    unsafe {
        use windows::Win32::UI::Shell::Common::ITEMIDLIST;
        let manager: IApplicationActivationManager =
            CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)
                .map_err(|error| error.to_string())?;
        let path = wide(&file.to_string_lossy());
        let mut item = std::ptr::null_mut();
        SHParseDisplayName(PCWSTR(path.as_ptr()), None, &mut item, 0, None)
            .map_err(|error| error.to_string())?;
        if item.is_null() {
            return Err("Windows could not resolve the selected file".into());
        }
        let list: [*const ITEMIDLIST; 1] = [item.cast_const()];
        let result = (|| {
            let items =
                SHCreateShellItemArrayFromIDLists(&list).map_err(|error| error.to_string())?;
            let aumid = wide(aumid);
            let verb = wide("open");
            manager
                .ActivateForFile(PCWSTR(aumid.as_ptr()), &items, PCWSTR(verb.as_ptr()))
                .map_err(|error| error.to_string())?;
            Ok(())
        })();
        CoTaskMemFree(Some(item.cast::<c_void>()));
        result
    }
}

unsafe fn enumerate_apps_folder() -> Result<Vec<AppsFolderItem>, String> {
    // SAFETY: The COM interfaces are owned references on the initialized STA thread; Shell-allocated strings are converted and released by take_string.
    unsafe {
        let folder: IShellItem = SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)
            .map_err(|error| error.to_string())?;
        let items: IEnumShellItems = folder
            .BindToHandler(None, &BHID_EnumItems as *const _)
            .map_err(|error| error.to_string())?;
        let mut output = Vec::new();
        loop {
            let mut next = [None];
            let mut fetched = 0;
            if items.Next(&mut next, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(item) = next.into_iter().flatten().next() else {
                continue;
            };
            let Ok(item2) = item.cast::<IShellItem2>() else {
                continue;
            };
            let Some(name) = display_name(&item, SIGDN_NORMALDISPLAY) else {
                continue;
            };
            if name.trim().is_empty() {
                continue;
            }
            let aumid = property_string(&item2, &PKEY_AppUserModel_ID).unwrap_or_default();
            let path = property_string(&item2, &PKEY_Link_TargetParsingPath)
                .or_else(|| display_name(&item, SIGDN_FILESYSPATH))
                .unwrap_or_default();
            let parsing_name =
                display_name(&item, SIGDN_DESKTOPABSOLUTEPARSING).unwrap_or_default();
            output.push(AppsFolderItem {
                name,
                aumid,
                path,
                parsing_name,
            });
        }
        Ok(output)
    }
}

unsafe fn display_name(
    item: &IShellItem,
    kind: windows::Win32::UI::Shell::SIGDN,
) -> Option<String> {
    // SAFETY: item is a live COM interface reference; GetDisplayName returns a Shell-allocated string that take_string converts and frees.
    unsafe {
        let value = item.GetDisplayName(kind).ok()?;
        take_string(value)
    }
}

unsafe fn property_string(
    item: &IShellItem2,
    key: &windows::Win32::Foundation::PROPERTYKEY,
) -> Option<String> {
    // SAFETY: item is a live COM interface reference; GetString returns a Shell-allocated string that take_string converts and frees.
    unsafe {
        let value = item.GetString(key as *const _).ok()?;
        take_string(value)
    }
}

unsafe fn take_string(value: PWSTR) -> Option<String> {
    // SAFETY: value is an owned PWSTR returned by a Windows Shell/COM API and is freed exactly once after conversion.
    unsafe {
        let result = value.to_string().ok();
        CoTaskMemFree(Some(value.0.cast::<c_void>()));
        result.filter(|value| !value.is_empty())
    }
}

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

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
