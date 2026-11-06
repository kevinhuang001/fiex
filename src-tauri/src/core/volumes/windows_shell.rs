//! Small Windows Shell helpers that still require Shell-specific COM APIs.

/// Shell icon work is serialized through a bounded STA queue. Shell COM
/// interfaces never cross worker threads, and application icons are disk
/// cached by the caller so each source is normally extracted only once.
#[cfg(all(target_os = "windows", feature = "desktop"))]
struct ShellIconRequest {
    source: String,
    target: std::path::PathBuf,
    done: std::sync::mpsc::Sender<Result<(), String>>,
}

#[cfg(all(target_os = "windows", feature = "desktop"))]
pub fn extract_icon(source: &str, target: &std::path::Path) -> Result<(), String> {
    use std::sync::{mpsc, OnceLock};
    static WORKER: OnceLock<mpsc::SyncSender<ShellIconRequest>> = OnceLock::new();
    let worker = WORKER.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel::<ShellIconRequest>(32);
        std::thread::Builder::new()
            .name("fiex-shell-icons-sta".into())
            .spawn(move || icon_worker(receiver))
            .expect("Windows Shell icon worker could not start");
        sender
    });
    let (done, result) = mpsc::channel();
    worker
        .send(ShellIconRequest {
            source: source.to_owned(),
            target: target.to_path_buf(),
            done,
        })
        .map_err(|_| "Windows Shell icon worker stopped".to_owned())?;
    result
        .recv()
        .map_err(|_| "Windows Shell icon worker stopped".to_owned())?
}

#[cfg(all(target_os = "windows", feature = "desktop"))]
fn icon_worker(receiver: std::sync::mpsc::Receiver<ShellIconRequest>) {
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };

    let initialized =
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
            .ok()
            .is_ok();
    while let Ok(request) = receiver.recv() {
        let result = if initialized {
            extract_icon_sta(&request.source, &request.target)
        } else {
            Err("Could not initialize the Windows Shell STA worker".into())
        };
        let _ = request.done.send(result);
    }
    if initialized {
        unsafe { CoUninitialize() };
    }
}

#[cfg(all(target_os = "windows", feature = "desktop"))]
fn extract_icon_sta(source: &str, target: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::SIZE,
            Graphics::Gdi::{
                DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
                BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
            },
            UI::Shell::{
                IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK,
                SIIGBF_ICONONLY,
            },
        },
    };

    let source = std::ffi::OsStr::new(source)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let factory: IShellItemImageFactory = unsafe {
        SHCreateItemFromParsingName(PCWSTR(source.as_ptr()), None)
            .map_err(|error| error.to_string())?
    };
    let bitmap = unsafe {
        factory
            .GetImage(
                SIZE { cx: 128, cy: 128 },
                SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK,
            )
            .map_err(|error| error.to_string())?
    };
    let result = (|| {
        let mut dimensions = BITMAP::default();
        let object_size = std::mem::size_of::<BITMAP>() as i32;
        if unsafe {
            GetObjectW(
                bitmap.into(),
                object_size,
                Some((&mut dimensions as *mut BITMAP).cast()),
            )
        } == 0
        {
            return Err("Windows Shell icon bitmap is unavailable".into());
        }
        if dimensions.bmWidth <= 0 || dimensions.bmHeight <= 0 {
            return Err("Windows Shell returned an empty icon".into());
        }
        let width = dimensions.bmWidth as u32;
        let height = dimensions.bmHeight as u32;
        let byte_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or("Windows Shell icon is too large")?;
        let mut pixels = vec![0_u8; byte_len];
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: dimensions.bmWidth,
                biHeight: -dimensions.bmHeight,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let dc = unsafe { GetDC(None) };
        if dc.is_invalid() {
            return Err("Windows could not create a device context for the icon".into());
        }
        let copied = unsafe {
            GetDIBits(
                dc,
                bitmap,
                0,
                height,
                Some(pixels.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            )
        };
        unsafe { ReleaseDC(None, dc) };
        if copied != height as i32 {
            return Err("Windows Shell icon pixels are unavailable".into());
        }
        let has_alpha = pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] != 0);
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
            if !has_alpha {
                pixel[3] = 255;
            }
        }
        let image = image::RgbaImage::from_raw(width, height, pixels)
            .ok_or("Windows Shell returned an invalid icon bitmap")?;
        image.save(target).map_err(|error| error.to_string())
    })();
    let _ = unsafe { DeleteObject(bitmap.into()) };
    result
}

/// Resolve an AppsFolder target such as
/// \`{FOLDERID_System}\msinfo32.exe\` into a real filesystem path. AppsFolder
/// uses known-folder IDs in place of drive letters for some Win32 shortcuts.
/// Resolving the identifier is separate from checking that the target exists.
#[cfg(target_os = "windows")]
pub fn resolve_known_folder_path(value: &str) -> Option<std::path::PathBuf> {
    use std::ffi::{c_void, OsStr, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::ptr;

    #[repr(C)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CLSIDFromString(value: *const u16, guid: *mut Guid) -> i32;
        fn CoTaskMemFree(ptr: *mut c_void);
    }
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHGetKnownFolderPath(
            folder: *const Guid,
            flags: u32,
            token: *mut c_void,
            result: *mut *mut u16,
        ) -> i32;
    }

    let end = value.find('}')?;
    let folder = value.get(..=end)?;
    let rest = value.get(end + 1..)?;
    if !folder.starts_with('{')
        || folder.len() != 38
        || !(rest.starts_with('\\') || rest.starts_with('/'))
        || rest.trim_start_matches(['\\', '/']).is_empty()
    {
        return None;
    }

    let wide = OsStr::new(folder)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut guid = Guid {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    // Win32 HRESULTs with the high bit set indicate an error.
    if unsafe { CLSIDFromString(wide.as_ptr(), &mut guid) } < 0 {
        return None;
    }

    let mut raw = ptr::null_mut();
    let status = unsafe { SHGetKnownFolderPath(&guid, 0, ptr::null_mut(), &mut raw) };
    if status < 0 || raw.is_null() {
        if !raw.is_null() {
            unsafe { CoTaskMemFree(raw.cast()) };
        }
        return None;
    }

    // The returned PWSTR is allocated by the Shell and must be freed with
    // CoTaskMemFree, including when a namespace target cannot be used.
    let base = unsafe {
        let mut len = 0;
        while *raw.add(len) != 0 {
            len += 1;
        }
        let value = OsString::from_wide(std::slice::from_raw_parts(raw, len));
        CoTaskMemFree(raw.cast());
        value
    };
    Some(std::path::PathBuf::from(base).join(rest.trim_start_matches(['\\', '/'])))
}

#[cfg(all(test, target_os = "windows"))]
mod known_folder_tests {
    use super::resolve_known_folder_path;

    #[test]
    fn system_folder_identity_resolves_without_hardcoded_paths() {
        let path =
            resolve_known_folder_path(r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\msinfo32.exe")
                .expect("FOLDERID_System should exist");
        assert!(path.ends_with("msinfo32.exe"));
        assert!(path.to_string_lossy().to_lowercase().contains("system32"));
    }

    #[test]
    fn malformed_and_unknown_shell_identities_do_not_become_paths() {
        assert!(resolve_known_folder_path(r"{invalid}\msinfo32.exe").is_none());
        assert!(resolve_known_folder_path(r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}").is_none());
        assert!(
            resolve_known_folder_path(r"{00000000-0000-0000-0000-000000000000}\x.exe").is_none()
        );
    }
}
