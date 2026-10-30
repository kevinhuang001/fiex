//! Hidden PowerShell bridge for shell-namespace enumeration. It runs with no
//! console window and never asks Explorer to show a folder.
#[cfg(target_os = "windows")]
pub fn run(script: &str) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // PowerShell 5 otherwise emits the active OEM code page for native stdout.
    let script =
        format!("[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); {script}");
    let mut last_error = String::new();
    for executable in ["powershell.exe", "pwsh.exe"] {
        match std::process::Command::new(executable)
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            Ok(output) if output.status.success() => {
                return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            }
            Ok(output) => last_error = String::from_utf8_lossy(&output.stderr).trim().to_string(),
            Err(error) => last_error = error.to_string(),
        }
    }
    Err(if last_error.is_empty() {
        crate::i18n::t("无法访问系统应用列表").into()
    } else {
        last_error
    })
}

/// Ask the Shell image factory for the registered item's icon. Unlike file
/// extraction this accepts AppsFolder identities and packaged applications.
#[cfg(target_os = "windows")]
pub fn extract_icon(source: &str, target: &std::path::Path) -> Result<(), String> {
    let quote = |value: &str| format!("'{}'", value.replace('\'', "''"));
    let code = r#"
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
[StructLayout(LayoutKind.Sequential)] public struct FiexSize { public int cx, cy; }
[StructLayout(LayoutKind.Sequential)] public struct FiexBitmap {
    public int bmType; public int bmWidth; public int bmHeight; public int bmWidthBytes;
    public short bmPlanes; public short bmBitsPixel; public IntPtr bmBits;
}
[StructLayout(LayoutKind.Sequential)] public struct FiexBitmapInfoHeader {
    public int biSize; public int biWidth; public int biHeight;
    public short biPlanes; public short biBitCount;
    public int biCompression; public int biSizeImage;
    public int biXPelsPerMeter; public int biYPelsPerMeter;
    public int biClrUsed; public int biClrImportant;
}
[ComImport, Guid("bcc18b79-ba16-442f-80c4-8a59c30c463b"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface FiexImageFactory {
    [PreserveSig] int GetImage(FiexSize size, uint flags, out IntPtr bitmap);
}
public static class FiexShellIcon {
    [DllImport("shell32.dll", CharSet=CharSet.Unicode, PreserveSig=false)]
    static extern void SHCreateItemFromParsingName(string path, IntPtr bind, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out FiexImageFactory item);
    [DllImport("gdi32.dll")] static extern bool DeleteObject(IntPtr handle);
    [DllImport("gdi32.dll")] static extern int GetObject(IntPtr handle, int size, ref FiexBitmap info);
    [DllImport("gdi32.dll")] static extern int GetDIBits(IntPtr dc, IntPtr bitmap, uint start, uint lines, byte[] bits, ref FiexBitmapInfoHeader info, uint usage);
    [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr window);
    [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr window, IntPtr dc);

    // Bitmap.FromHbitmap drops the alpha channel, which painted every
    // transparent app icon on a black square. Rebuild the ARGB pixels from
    // GetDIBits instead so the PNG keeps its transparency.
    public static void Save(string source, string target) {
        Guid iid = new Guid("bcc18b79-ba16-442f-80c4-8a59c30c463b");
        FiexImageFactory item = null;
        IntPtr handle = IntPtr.Zero;
        try {
            SHCreateItemFromParsingName(source, IntPtr.Zero, ref iid, out item);
            int hr = item.GetImage(new FiexSize { cx = 128, cy = 128 }, 0x104, out handle);
            Marshal.ThrowExceptionForHR(hr);
            FiexBitmap info = new FiexBitmap();
            if (GetObject(handle, Marshal.SizeOf(typeof(FiexBitmap)), ref info) == 0) throw new Exception("icon bitmap unavailable");
            int width = info.bmWidth;
            int height = info.bmHeight;
            byte[] pixels = new byte[width * height * 4];
            FiexBitmapInfoHeader header = new FiexBitmapInfoHeader();
            header.biSize = Marshal.SizeOf(typeof(FiexBitmapInfoHeader));
            header.biWidth = width;
            header.biHeight = -height;
            header.biPlanes = 1;
            header.biBitCount = 32;
            header.biCompression = 0;
            IntPtr dc = GetDC(IntPtr.Zero);
            int lines = GetDIBits(dc, handle, 0, (uint)height, pixels, ref header, 0);
            ReleaseDC(IntPtr.Zero, dc);
            if (lines == 0) throw new Exception("icon pixels unavailable");
            bool hasAlpha = false;
            for (int i = 3; i < pixels.Length; i += 4) {
                if (pixels[i] != 0) { hasAlpha = true; break; }
            }
            using (Bitmap bitmap = new Bitmap(width, height, PixelFormat.Format32bppArgb)) {
                BitmapData data = bitmap.LockBits(new Rectangle(0, 0, width, height), ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
                for (int y = 0; y < height; y++)
                    Marshal.Copy(pixels, y * width * 4, new IntPtr(data.Scan0.ToInt64() + (long)y * data.Stride), width * 4);
                bitmap.UnlockBits(data);
                if (!hasAlpha) {
                    // No alpha channel at all: keep the icon visible instead of fully transparent.
                    for (int y = 0; y < height; y++)
                        for (int x = 0; x < width; x++) {
                            Color color = bitmap.GetPixel(x, y);
                            bitmap.SetPixel(x, y, Color.FromArgb(255, color));
                        }
                }
                bitmap.Save(target, ImageFormat.Png);
            }
        } finally {
            if (handle != IntPtr.Zero) DeleteObject(handle);
            if (item != null) Marshal.ReleaseComObject(item);
        }
    }
}
'@
"#;
    run(&format!(
        "$ErrorActionPreference='Stop'; {code} [FiexShellIcon]::Save({}, {})",
        quote(source),
        quote(&target.to_string_lossy())
    ))?;
    if target.is_file() {
        Ok(())
    } else {
        Err("Shell icon unavailable".into())
    }
}

/// Resolve an AppsFolder target such as
/// \`{FOLDERID_System}\msinfo32.exe\` into a real filesystem path. AppsFolder
/// uses known-folder IDs in place of drive letters for some Win32 shortcuts.
/// Resolving the identifier is separate from checking that the target exists.
#[cfg(target_os = "windows")]
pub fn resolve_known_folder_path(value: &str) -> Option<std::path::PathBuf> {
    use std::ffi::{OsStr, OsString, c_void};
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
        assert!(resolve_known_folder_path(r"{00000000-0000-0000-0000-000000000000}\x.exe").is_none());
    }
}
