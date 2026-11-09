use windows::{
    Win32::{
        Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE},
        Storage::FileSystem::{
            CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
            FindFirstVolumeW, FindNextVolumeW, OPEN_EXISTING, SetVolumeMountPointW,
        },
        System::IO::DeviceIoControl,
        System::Ioctl::{FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME, IOCTL_STORAGE_EJECT_MEDIA},
    },
    core::PCWSTR,
};

use super::shell::{
    VolumeSearch, device_number, free_drive_letter, volume_filesystem, volume_paths, wide,
    wide_string,
};

pub(crate) fn mount_volume(device: &str) -> Result<(String, String), String> {
    let (disk, partition) = parse_device(device)?;
    let mut buffer = [0u16; 1024];
    // SAFETY: buffer is writable storage for the NUL-terminated volume name.
    let search = unsafe { FindFirstVolumeW(&mut buffer) }.map_err(|error| error.to_string())?;
    let _search = VolumeSearch::new(search);
    let mut result = None;
    loop {
        let volume = wide_string(&buffer);
        if device_number(&volume).is_some_and(|number| {
            number.DeviceNumber == disk && number.PartitionNumber == partition
        }) {
            let paths = volume_paths(&volume);
            if let Some(path) = paths.first() {
                result = Some((path.clone(), volume_filesystem(&volume)));
            } else {
                let letter = free_drive_letter().ok_or("No drive letter is available")?;
                let mount_point = format!("{letter}:\\");
                // SAFETY: both strings are NUL-terminated and live through SetVolumeMountPointW.
                unsafe {
                    SetVolumeMountPointW(
                        PCWSTR(wide(&mount_point).as_ptr()),
                        PCWSTR(wide(&volume).as_ptr()),
                    )
                }
                .map_err(|error| error.to_string())?;
                result = Some((mount_point, volume_filesystem(&volume)));
            }
            break;
        }
        buffer.fill(0);
        // SAFETY: search is the live handle returned above and buffer is writable output storage.
        if unsafe { FindNextVolumeW(search, &mut buffer) }.is_err() {
            break;
        }
    }
    result.ok_or_else(|| "The requested removable volume is no longer available".into())
}

pub(crate) fn eject_drive(letter: char) -> Result<(), String> {
    let root = wide(&format!("\\\\.\\{letter}:"));
    // SAFETY: root is NUL-terminated and remains alive while CreateFileW reads it.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(root.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .map_err(|error| error.to_string())?;
    let result = (|| {
        let mut returned = 0;
        // SAFETY: handle is live and returned is a writable output counter.
        unsafe {
            DeviceIoControl(
                handle,
                FSCTL_LOCK_VOLUME,
                None,
                0,
                None,
                0,
                Some(&mut returned),
                None,
            )
        }
        .map_err(|error| error.to_string())?;
        // SAFETY: handle is live and returned is a writable output counter.
        unsafe {
            DeviceIoControl(
                handle,
                FSCTL_DISMOUNT_VOLUME,
                None,
                0,
                None,
                0,
                Some(&mut returned),
                None,
            )
        }
        .map_err(|error| error.to_string())?;
        // SAFETY: handle is live and returned is a writable output counter.
        unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_EJECT_MEDIA,
                None,
                0,
                None,
                0,
                Some(&mut returned),
                None,
            )
        }
        .map_err(|error| error.to_string())
    })();
    // SAFETY: handle is the owned handle returned by CreateFileW and is closed exactly once.
    let _ = unsafe { CloseHandle(handle) };
    result
}

fn parse_device(device: &str) -> Result<(u32, u32), String> {
    let (disk, partition) = device
        .split_once(':')
        .ok_or_else(|| "Invalid removable volume identity".to_owned())?;
    let disk = disk.parse().map_err(|_| "Invalid disk number".to_owned())?;
    let partition = partition
        .parse()
        .map_err(|_| "Invalid partition number".to_owned())?;
    Ok((disk, partition))
}
