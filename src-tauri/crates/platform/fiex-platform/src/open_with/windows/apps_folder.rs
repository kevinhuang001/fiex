use std::ffi::c_void;
use std::path::Path;

use super::{AppsFolderItem, wide};
use windows::{
    Win32::{
        Storage::EnhancedStorage::{PKEY_AppUserModel_ID, PKEY_Link_TargetParsingPath},
        System::Com::{CLSCTX_LOCAL_SERVER, CoCreateInstance, CoTaskMemFree},
        UI::Shell::{
            ApplicationActivationManager, BHID_EnumItems, FOLDERID_AppsFolder,
            IApplicationActivationManager, IEnumShellItems, IShellItem, IShellItem2,
            KF_FLAG_DEFAULT, SHCreateShellItemArrayFromIDLists, SHGetKnownFolderItem,
            SHParseDisplayName, SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_FILESYSPATH,
            SIGDN_NORMALDISPLAY,
        },
    },
    core::{Interface, PCWSTR, PWSTR},
};

pub(super) unsafe fn activate_package_for_file(file: &Path, aumid: &str) -> Result<(), String> {
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

pub(super) unsafe fn enumerate_apps_folder() -> Result<Vec<AppsFolderItem>, String> {
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

pub(super) unsafe fn take_string(value: PWSTR) -> Option<String> {
    // SAFETY: value is an owned PWSTR returned by a Windows Shell/COM API and is freed exactly once after conversion.
    unsafe {
        let result = value.to_string().ok();
        CoTaskMemFree(Some(value.0.cast::<c_void>()));
        result.filter(|value| !value.is_empty())
    }
}
