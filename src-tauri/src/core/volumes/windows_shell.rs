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
