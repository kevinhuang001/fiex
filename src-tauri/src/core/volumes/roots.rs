//! Stable roots above the mount table: Recent, the user's home folders and the
//! platform applications entry.

use super::Location;
use std::path::PathBuf;

pub(super) fn base_locations() -> Vec<Location> {
    let mut out = Vec::new();
    out.push(Location {
        name: "最近使用".into(),
        path: "recent://".into(),
        kind: "recent".into(),
        available: 0,
        total: 0,
        removable: false,
        filesystem: String::new(),
        device: String::new(),
        mounted: true,
        usbfs: false,
    });
    let named: [(&str, Option<PathBuf>); 7] = [
        ("个人文件夹", dirs::home_dir()),
        ("桌面", dirs::desktop_dir()),
        ("下载", dirs::download_dir()),
        ("文稿", dirs::document_dir()),
        ("图片", dirs::picture_dir()),
        ("音乐", dirs::audio_dir()),
        ("视频", dirs::video_dir()),
    ];
    for (name, path) in named {
        if let Some(path) = path.filter(|p| p.exists()) {
            out.push(Location {
                name: name.into(),
                path: path.to_string_lossy().into_owned(),
                kind: "favorite".into(),
                available: 0,
                total: 0,
                removable: false,
                filesystem: String::new(),
                device: String::new(),
                mounted: true,
                usbfs: false,
            });
        }
    }
    // macOS has a real Applications folder. Windows and Linux expose installed
    // applications through shell/desktop services, so those entries only open
    // the platform UI instead of pretending to be a browsable directory.
    let applications = if cfg!(target_os = "macos") {
        let path = PathBuf::from("/Applications");
        path.exists().then_some(path)
    } else {
        None
    };
    out.push(Location {
        name: "应用程序".into(),
        path: applications
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
        kind: "applications".into(),
        available: 0,
        total: 0,
        removable: false,
        filesystem: String::new(),
        device: String::new(),
        mounted: true,
        usbfs: false,
    });
    out
}
