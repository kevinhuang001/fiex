//! Hidden PowerShell bridge for shell-namespace enumeration. It runs with no
//! console window and never asks Explorer to show a folder.
#[cfg(target_os = "windows")]
pub fn run(script: &str) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // PowerShell 5 otherwise emits the active OEM code page for native stdout.
    let script = format!("[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); {script}");
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
[ComImport, Guid("bcc18b79-ba16-442f-80c4-8a59c30c463b"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface FiexImageFactory {
    [PreserveSig] int GetImage(FiexSize size, uint flags, out IntPtr bitmap);
}
public static class FiexShellIcon {
    [DllImport("shell32.dll", CharSet=CharSet.Unicode, PreserveSig=false)]
    static extern void SHCreateItemFromParsingName(string path, IntPtr bind, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out FiexImageFactory item);
    [DllImport("gdi32.dll")] static extern bool DeleteObject(IntPtr handle);
    public static void Save(string source, string target) {
        Guid iid = new Guid("bcc18b79-ba16-442f-80c4-8a59c30c463b");
        FiexImageFactory item = null; IntPtr handle = IntPtr.Zero;
        try {
            SHCreateItemFromParsingName(source, IntPtr.Zero, ref iid, out item);
            int hr = item.GetImage(new FiexSize { cx=128, cy=128 }, 0x104, out handle);
            Marshal.ThrowExceptionForHR(hr);
            using (Bitmap bitmap = Bitmap.FromHbitmap(handle)) { bitmap.Save(target, ImageFormat.Png); }
        } finally {
            if (handle != IntPtr.Zero) DeleteObject(handle);
            if (item != null) Marshal.ReleaseComObject(item);
        }
    }
}
'@
"#;
    run(&format!("$ErrorActionPreference='Stop'; {code} [FiexShellIcon]::Save({}, {})", quote(source), quote(&target.to_string_lossy())))?;
    if target.is_file() { Ok(()) } else { Err("Shell icon unavailable".into()) }
}
