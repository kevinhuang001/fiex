use std::{
    ffi::c_void,
    mem::{size_of, size_of_val},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, GENERIC_READ, HANDLE},
        Storage::FileSystem::{
            CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
            GetVolumeInformationW, GetVolumePathNamesForVolumeNameW, OPEN_EXISTING,
        },
        System::{
            IO::DeviceIoControl,
            Ioctl::{
                IOCTL_STORAGE_GET_DEVICE_NUMBER, IOCTL_STORAGE_QUERY_PROPERTY,
                PropertyStandardQuery, STORAGE_DEVICE_DESCRIPTOR, STORAGE_DEVICE_NUMBER,
                STORAGE_PROPERTY_QUERY, StorageDeviceProperty,
            },
        },
    },
    core::PCWSTR,
};

pub(super) struct VolumeSearch(HANDLE);

impl VolumeSearch {
    pub(super) fn new(handle: HANDLE) -> Self {
        Self(handle)
    }
}

impl Drop for VolumeSearch {
    fn drop(&mut self) {
        // SAFETY: The handle is the owned volume search handle returned by FindFirstVolumeW.
        unsafe {
            let _ = windows::Win32::Storage::FileSystem::FindVolumeClose(self.0);
        }
    }
}

pub(super) fn device_number(path: &str) -> Option<STORAGE_DEVICE_NUMBER> {
    let path = wide(path);
    // SAFETY: path is NUL-terminated and remains alive for the duration of CreateFileW.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(path.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .ok()?;
    let mut number = STORAGE_DEVICE_NUMBER::default();
    let mut returned = 0;
    // SAFETY: number and returned are writable buffers with the sizes passed to DeviceIoControl.
    let result = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_GET_DEVICE_NUMBER,
            None,
            0,
            Some((&mut number as *mut STORAGE_DEVICE_NUMBER).cast()),
            size_of::<STORAGE_DEVICE_NUMBER>() as u32,
            Some(&mut returned),
            None,
        )
    };
    // SAFETY: handle is the owned handle returned above and is closed exactly once.
    let _ = unsafe { CloseHandle(handle) };
    result.ok().map(|_| number)
}

pub(super) fn is_removable_disk(disk: u32) -> bool {
    let path = wide(&format!("\\\\.\\PhysicalDrive{disk}"));
    // SAFETY: path is NUL-terminated and remains alive for the duration of CreateFileW.
    let Ok(handle) = (unsafe {
        CreateFileW(
            PCWSTR(path.as_ptr()),
            GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }) else {
        return false;
    };
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        ..Default::default()
    };
    let mut storage = [0u64; 64];
    let mut returned = 0;
    // SAFETY: query and storage are live, correctly sized buffers for the input and output lengths.
    let result = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some((&query as *const STORAGE_PROPERTY_QUERY).cast()),
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(storage.as_mut_ptr().cast::<c_void>()),
            size_of_val(&storage) as u32,
            Some(&mut returned),
            None,
        )
    };
    // SAFETY: handle is the owned handle returned above and is closed exactly once.
    let _ = unsafe { CloseHandle(handle) };
    if result.is_err() || (returned as usize) < size_of::<STORAGE_DEVICE_DESCRIPTOR>() {
        return false;
    }
    // SAFETY: successful query returned at least one full descriptor into aligned storage.
    let descriptor = unsafe { *storage.as_ptr().cast::<STORAGE_DEVICE_DESCRIPTOR>() };
    descriptor.BusType == windows::Win32::Storage::FileSystem::BusTypeUsb
        || descriptor.RemovableMedia
}

pub(super) fn volume_paths(volume: &str) -> Vec<String> {
    let volume = wide(volume);
    let mut required = 0;
    // SAFETY: volume is NUL-terminated and required is a valid writable length output.
    let _ =
        unsafe { GetVolumePathNamesForVolumeNameW(PCWSTR(volume.as_ptr()), None, &mut required) };
    if required == 0 {
        return Vec::new();
    }
    let mut paths = vec![0u16; required as usize];
    // SAFETY: paths has the capacity reported by the preceding call and remains writable here.
    if unsafe {
        GetVolumePathNamesForVolumeNameW(PCWSTR(volume.as_ptr()), Some(&mut paths), &mut required)
    }
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

pub(super) fn volume_filesystem(volume: &str) -> String {
    let volume = wide(volume);
    let mut name = [0u16; 260];
    let mut filesystem = [0u16; 64];
    // SAFETY: volume is NUL-terminated; both output arrays are writable and live through the call.
    if unsafe {
        GetVolumeInformationW(
            PCWSTR(volume.as_ptr()),
            Some(&mut name),
            None,
            None,
            None,
            Some(&mut filesystem),
        )
    }
    .is_ok()
    {
        wide_string(&filesystem)
    } else {
        String::new()
    }
}

pub(super) fn free_drive_letter() -> Option<char> {
    // SAFETY: GetLogicalDrives takes no pointers and has no caller-side preconditions.
    let drives = unsafe { windows::Win32::Storage::FileSystem::GetLogicalDrives() };
    ('E'..='Z').find(|letter| drives & (1 << (*letter as u32 - 'A' as u32)) == 0)
}

pub(super) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(super) fn wide_string(value: &[u16]) -> String {
    String::from_utf16_lossy(value.split(|unit| *unit == 0).next().unwrap_or_default())
}
