//! Offset reads for every driver. The caller supplies a bounded buffer; file
//! size never determines the allocation size.
use super::*;
pub fn read_at(route: &Route, offset: u64, output: &mut [u8]) -> Result<usize, String> {
    use std::io::{Read, Seek, SeekFrom};
    ensure_access(route)?;
    let path = clean_path(&route.rel);
    match route.kind {
        FsKind::Ntfs => {
            let fs = fs_ntfs::facade::Filesystem::mount(&route.driver_device)
                .map_err(|e| e.to_string())?;
            fs.read_file(&path, offset, output)
                .map_err(|e| e.to_string())
        }
        FsKind::Ext4 => {
            let fs = ext4_mount(&route.driver_device, false)?;
            let inode = fs
                .lookup_path_bytes(path.as_bytes())
                .map_err(|e| e.to_string())?;
            fs.read_ino(inode, offset, output)
                .map_err(|e| e.to_string())
        }
        FsKind::Btrfs => {
            let fs = btrfs_fs(&route.driver_device)?;
            let inode = fs.lookup_path(&path).map_err(|e| e.to_string())?;
            fs.read_at(inode.ino, offset, output)
                .map_err(|e| e.to_string())
        }
        FsKind::Apfs => {
            let device = std::io::BufReader::new(
                std::fs::File::open(&route.driver_device).map_err(|e| e.to_string())?,
            );
            let mut fs = apfs::ApfsVolume::open(device).map_err(|e| e.to_string())?;
            let mut file = fs.open_file(&path).map_err(|e| e.to_string())?;
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| e.to_string())?;
            file.read(output).map_err(|e| e.to_string())
        }
        FsKind::Hfs => {
            let device = std::io::BufReader::new(
                std::fs::File::open(&route.driver_device).map_err(|e| e.to_string())?,
            );
            let mut fs = hfsplus::HfsVolume::open(device).map_err(|e| e.to_string())?;
            let mut file = fs.open_file(&path).map_err(|e| e.to_string())?;
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| e.to_string())?;
            file.read(output).map_err(|e| e.to_string())
        }
    }
}
pub fn copy_file_to(route: &Route, output: &mut impl std::io::Write) -> Result<u64, String> {
    let mut buffer = vec![0; 256 * 1024];
    let mut offset = 0u64;
    loop {
        let size = read_at(route, offset, &mut buffer)?;
        if size == 0 {
            return Ok(offset);
        }
        output
            .write_all(&buffer[..size])
            .map_err(|e| e.to_string())?;
        offset = offset
            .checked_add(size as u64)
            .ok_or("USB offset overflow")?;
    }
}
