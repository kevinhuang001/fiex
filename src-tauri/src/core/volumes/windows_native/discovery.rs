use windows::{
    Win32::Storage::FileSystem::{
        FindFirstVolumeW, FindNextVolumeW, GetDiskFreeSpaceExW, GetVolumeInformationW,
    },
    core::PCWSTR,
};

use super::{
    RemovableVolume,
    shell::{VolumeSearch, device_number, is_removable_disk, volume_paths, wide, wide_string},
};

pub(crate) fn enumerate_unmounted_removable() -> Vec<RemovableVolume> {
    let mut buffer = [0u16; 1024];
    // SAFETY: buffer is writable storage for the NUL-terminated volume name.
    let Ok(search) = (unsafe { FindFirstVolumeW(&mut buffer) }) else {
        return Vec::new();
    };
    let _search = VolumeSearch::new(search);
    let mut output = Vec::new();
    loop {
        let volume = wide_string(&buffer);
        if let Some(record) = removable_volume(&volume) {
            output.push(record);
        }
        buffer.fill(0);
        // SAFETY: search is the live handle returned above and buffer is writable output storage.
        if unsafe { FindNextVolumeW(search, &mut buffer) }.is_err() {
            break;
        }
    }
    output
}

fn removable_volume(volume: &str) -> Option<RemovableVolume> {
    if !volume_paths(volume).is_empty() {
        return None;
    }
    let number = device_number(volume)?;
    if !is_removable_disk(number.DeviceNumber) {
        return None;
    }
    let volume = wide(volume);
    let mut label = [0u16; 260];
    let mut filesystem = [0u16; 64];
    // SAFETY: volume is NUL-terminated; output arrays are writable and remain alive through the call.
    let info = unsafe {
        GetVolumeInformationW(
            PCWSTR(volume.as_ptr()),
            Some(&mut label),
            None,
            None,
            None,
            Some(&mut filesystem),
        )
    };
    let mut available = 0;
    let mut total = 0;
    // SAFETY: volume is NUL-terminated; output counters are writable for this call.
    let _ = unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(volume.as_ptr()),
            Some(&mut available),
            Some(&mut total),
            None,
        )
    };
    let fs = if info.is_ok() {
        wide_string(&filesystem)
    } else {
        String::new()
    };
    Some(RemovableVolume {
        device: format!("{}:{}", number.DeviceNumber, number.PartitionNumber),
        name: if info.is_ok() && !wide_string(&label).trim().is_empty() {
            wide_string(&label)
        } else {
            format!("USB {}", number.PartitionNumber)
        },
        filesystem: fs,
        total,
        available,
    })
}
