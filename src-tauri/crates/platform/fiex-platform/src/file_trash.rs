//! Move a host filesystem path to the operating system's Trash without
//! presenting a second confirmation dialog.

use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;

#[cfg(not(windows))]
pub fn move_to_trash(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|error| error.to_string())
}

#[cfg(windows)]
pub fn move_to_trash(path: &Path) -> Result<(), String> {
    let target = canonicalize_target(path)?;
    std::thread::Builder::new()
        .name("fiex-windows-trash".into())
        .spawn(move || move_to_trash_sta(target))
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "Windows Shell Trash worker panicked".to_owned())?
}

#[cfg(windows)]
fn canonicalize_target(path: &Path) -> Result<PathBuf, String> {
    let target = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let parent = target
        .parent()
        .ok_or_else(|| format!("{}: cannot move a filesystem root to Trash", path.display()))?;
    let name = target
        .file_name()
        .ok_or_else(|| format!("{}: missing file name", path.display()))?;
    let parent = dunce::canonicalize(parent).map_err(|error| error.to_string())?;
    Ok(parent.join(name))
}

#[cfg(windows)]
fn move_to_trash_sta(path: PathBuf) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::{
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
            COINIT_DISABLE_OLE1DDE,
        },
        UI::Shell::{
            FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName,
            FOFX_RECYCLEONDELETE, FOF_ALLOWUNDO, FOF_NO_UI,
        },
    };

    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
        .ok()
        .map_err(|error| error.to_string())?;
    let result = (|| unsafe {
        let path = path.to_string_lossy();
        let shell_path = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else if let Some(ordinary) = path.strip_prefix(r"\\?\") {
            ordinary.to_owned()
        } else {
            path.into_owned()
        };
        let parsing_name = std::ffi::OsStr::new(&shell_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let operation: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)
            .map_err(|error| error.to_string())?;
        operation
            .SetOperationFlags(FOF_NO_UI | FOF_ALLOWUNDO | FOFX_RECYCLEONDELETE)
            .map_err(|error| error.to_string())?;
        let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(parsing_name.as_ptr()), None)
            .map_err(|error| error.to_string())?;
        operation
            .DeleteItem(&item, None)
            .map_err(|error| error.to_string())?;
        operation
            .PerformOperations()
            .map_err(|error| error.to_string())?;
        if operation
            .GetAnyOperationsAborted()
            .map_err(|error| error.to_string())?
            .as_bool()
        {
            return Err("Windows did not move the item to the Recycle Bin".into());
        }
        Ok(())
    })();
    unsafe { CoUninitialize() };
    result
}
