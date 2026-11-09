use super::OpenWithCandidate;
use std::{
    path::Path,
    sync::{OnceLock, mpsc},
};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};

mod apps_folder;
mod association;

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
    run_sta(|| unsafe { apps_folder::enumerate_apps_folder() })
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
        return run_sta(move || unsafe { apps_folder::activate_package_for_file(&file, &aumid) });
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
pub(super) fn candidates(extension: &str) -> Result<Vec<OpenWithCandidate>, String> {
    association::candidates(extension)
}

pub(super) fn open(file: &Path, extension: &str, application_id: &str) -> Result<(), String> {
    association::open(file, extension, application_id)
}

pub(super) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
