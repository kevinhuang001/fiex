//! Native volume enumeration, drive-letter assignment, and safe media eject.

use std::{
    ffi::c_void,
    mem::{size_of, size_of_val},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE},
        Storage::FileSystem::{
            CreateFileW, FindFirstVolumeW, FindNextVolumeW, FindVolumeClose, GetDiskFreeSpaceExW,
            GetVolumeInformationW, GetVolumePathNamesForVolumeNameW, SetVolumeMountPointW,
            FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::{
            Ioctl::{
                PropertyStandardQuery, StorageDeviceProperty, FSCTL_DISMOUNT_VOLUME,
                FSCTL_LOCK_VOLUME, IOCTL_STORAGE_EJECT_MEDIA, IOCTL_STORAGE_GET_DEVICE_NUMBER,
                IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_DEVICE_DESCRIPTOR, STORAGE_DEVICE_NUMBER,
                STORAGE_PROPERTY_QUERY,
            },
            IO::DeviceIoControl,
        },
    },
};

#[derive(Clone, Debug)]
pub(crate) struct RemovableVolume {
    pub device: String,
    pub name: String,
    pub filesystem: String,
    pub total: u64,
    pub available: u64,
}

struct VolumeSearch(HANDLE);

impl Drop for VolumeSearch {
    fn drop(&mut self) {
        unsafe {
            let _ = FindVolumeClose(self.0);
        }
    }
}

pub(crate) fn enumerate_unmounted_removable() -> Vec<RemovableVolume> {
    unsafe {
        let mut buffer = [0u16; 1024];
        let Ok(search) = FindFirstVolumeW(&mut buffer) else {
            return Vec::new();
        };
        let _search = VolumeSearch(search);
        let mut output = Vec::new();
        loop {
            let volume = wide_string(&buffer);
            if let Some(record) = removable_volume(&volume) {
                output.push(record);
            }
            buffer.fill(0);
            if FindNextVolumeW(search, &mut buffer).is_err() {
                break;
            }
        }
        output
    }
}

unsafe fn removable_volume(volume: &str) -> Option<RemovableVolume> {
    if !volume_paths(volume).is_empty() {
        return None;
    }
    let number = device_number(volume)?;
    if !is_removable_disk(number.DeviceNumber) {
        return None;
    }
    let mut label = [0u16; 260];
    let mut filesystem = [0u16; 64];
    let info = GetVolumeInformationW(
        PCWSTR(wide(volume).as_ptr()),
        Some(&mut label),
        None,
        None,
        None,
        Some(&mut filesystem),
    );
    let mut available = 0;
    let mut total = 0;
    let _ = GetDiskFreeSpaceExW(
        PCWSTR(wide(volume).as_ptr()),
        Some(&mut available),
        Some(&mut total),
        None,
    );
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

pub(crate) fn mount_volume(device: &str) -> Result<(String, String), String> {
    let (disk, partition) = parse_device(device)?;
    unsafe {
        let mut buffer = [0u16; 1024];
        let search = FindFirstVolumeW(&mut buffer).map_err(|error| error.to_string())?;
        let _search = VolumeSearch(search);
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
                    SetVolumeMountPointW(
                        PCWSTR(wide(&mount_point).as_ptr()),
                        PCWSTR(wide(&volume).as_ptr()),
                    )
                    .map_err(|error| error.to_string())?;
                    result = Some((mount_point, volume_filesystem(&volume)));
                }
                break;
            }
            buffer.fill(0);
            if FindNextVolumeW(search, &mut buffer).is_err() {
                break;
            }
        }
        result.ok_or_else(|| "The requested removable volume is no longer available".into())
    }
}

pub(crate) fn eject_drive(letter: char) -> Result<(), String> {
    let root = format!("\\\\.\\{letter}:");
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide(&root).as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .map_err(|error| error.to_string())?;
    let result = unsafe {
        let mut returned = 0;
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
        .map_err(|error| error.to_string())?;
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
        .map_err(|error| error.to_string())?;
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
        .map_err(|error| error.to_string())
    }
    .map_err(|error| error.to_string());
    let _ = unsafe { CloseHandle(handle) };
    result
}

unsafe fn device_number(path: &str) -> Option<STORAGE_DEVICE_NUMBER> {
    let handle = CreateFileW(
        PCWSTR(wide(path).as_ptr()),
        0,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_FLAGS_AND_ATTRIBUTES(0),
        None,
    )
    .ok()?;
    let mut number = STORAGE_DEVICE_NUMBER::default();
    let mut returned = 0;
    let result = DeviceIoControl(
        handle,
        IOCTL_STORAGE_GET_DEVICE_NUMBER,
        None,
        0,
        Some((&mut number as *mut STORAGE_DEVICE_NUMBER).cast()),
        size_of::<STORAGE_DEVICE_NUMBER>() as u32,
        Some(&mut returned),
        None,
    );
    let _ = CloseHandle(handle);
    result.ok().map(|_| number)
}

unsafe fn is_removable_disk(disk: u32) -> bool {
    let path = format!("\\\\.\\PhysicalDrive{disk}");
    let Ok(handle) = CreateFileW(
        PCWSTR(wide(&path).as_ptr()),
        GENERIC_READ.0,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_FLAGS_AND_ATTRIBUTES(0),
        None,
    ) else {
        return false;
    };
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        ..Default::default()
    };
    let mut storage = [0u64; 64];
    let mut returned = 0;
    let result = DeviceIoControl(
        handle,
        IOCTL_STORAGE_QUERY_PROPERTY,
        Some((&query as *const STORAGE_PROPERTY_QUERY).cast()),
        size_of::<STORAGE_PROPERTY_QUERY>() as u32,
        Some(storage.as_mut_ptr().cast::<c_void>()),
        size_of_val(&storage) as u32,
        Some(&mut returned),
        None,
    );
    let _ = CloseHandle(handle);
    result.is_ok()
        && returned as usize >= size_of::<STORAGE_DEVICE_DESCRIPTOR>()
        && ((*storage.as_ptr().cast::<STORAGE_DEVICE_DESCRIPTOR>()).BusType
            == windows::Win32::Storage::FileSystem::BusTypeUsb
            || (*storage.as_ptr().cast::<STORAGE_DEVICE_DESCRIPTOR>()).RemovableMedia)
}

unsafe fn volume_paths(volume: &str) -> Vec<String> {
    let mut required = 0;
    let _ = GetVolumePathNamesForVolumeNameW(PCWSTR(wide(volume).as_ptr()), None, &mut required);
    if required == 0 {
        return Vec::new();
    }
    let mut paths = vec![0u16; required as usize];
    if GetVolumePathNamesForVolumeNameW(
        PCWSTR(wide(volume).as_ptr()),
        Some(&mut paths),
        &mut required,
    )
    .is_err()
    {
        return Vec::new();
    }
    paths
        .split(|value| *value == 0)
        .filter(|value| !value.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

unsafe fn volume_filesystem(volume: &str) -> String {
    let mut name = [0u16; 260];
    let mut filesystem = [0u16; 64];
    if GetVolumeInformationW(
        PCWSTR(wide(volume).as_ptr()),
        Some(&mut name),
        None,
        None,
        None,
        Some(&mut filesystem),
    )
    .is_ok()
    {
        wide_string(&filesystem)
    } else {
        String::new()
    }
}

unsafe fn free_drive_letter() -> Option<char> {
    let drives = windows::Win32::Storage::FileSystem::GetLogicalDrives();
    ('E'..='Z').find(|letter| drives & (1 << (*letter as u32 - 'A' as u32)) == 0)
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

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_string(value: &[u16]) -> String {
    String::from_utf16_lossy(value.split(|unit| *unit == 0).next().unwrap_or_default())
}
