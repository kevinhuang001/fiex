//! Platform Shell access for the Windows Recycle Bin. Stable identity and URI
//! validation remain in the owning Trash module.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecycleBinItemMetadata {
    pub id: String,
    pub name: String,
    pub original: String,
    pub deleted_unix: i64,
    pub folder: bool,
}

pub trait RecycleBinItemHandle: Send {
    fn metadata(&self) -> &RecycleBinItemMetadata;
    fn restore(self: Box<Self>) -> Result<(), String>;
    fn purge(self: Box<Self>) -> Result<(), String>;
}

pub trait RecycleBinBackend: Send + Sync {
    fn enumerate(&self) -> Result<Vec<Box<dyn RecycleBinItemHandle>>, String>;
    fn empty(&self) -> Result<usize, String>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemRecycleBinBackend;

#[cfg(windows)]
mod windows {
    use super::{
        RecycleBinBackend, RecycleBinItemHandle, RecycleBinItemMetadata, SystemRecycleBinBackend,
    };
    use trash::TrashItem;

    struct ShellItem {
        metadata: RecycleBinItemMetadata,
        item: TrashItem,
    }

    impl RecycleBinItemHandle for ShellItem {
        fn metadata(&self) -> &RecycleBinItemMetadata {
            &self.metadata
        }

        fn restore(self: Box<Self>) -> Result<(), String> {
            let id = self.item.id.clone();
            trash::os_limited::restore_all([self.item]).map_err(|error| error.to_string())?;
            if trash::os_limited::list()
                .map_err(|error| error.to_string())?
                .iter()
                .any(|current| current.id == id)
            {
                return Err("Windows did not restore the selected Recycle Bin item".into());
            }
            Ok(())
        }

        fn purge(self: Box<Self>) -> Result<(), String> {
            let id = self.item.id.clone();
            trash::os_limited::purge_all([&self.item]).map_err(|error| error.to_string())?;
            if trash::os_limited::list()
                .map_err(|error| error.to_string())?
                .iter()
                .any(|current| current.id == id)
            {
                return Err(
                    "Windows did not permanently delete the selected Recycle Bin item".into(),
                );
            }
            Ok(())
        }
    }

    fn shell_item(item: TrashItem) -> Result<Box<dyn RecycleBinItemHandle>, String> {
        let id = item
            .id
            .to_str()
            .ok_or("Recycle Bin item identity is not valid Unicode")?
            .to_owned();
        let name = item
            .name
            .to_str()
            .ok_or("Recycle Bin item name is not valid Unicode")?
            .to_owned();
        let original = item
            .original_parent
            .to_str()
            .ok_or("Recycle Bin source path is not valid Unicode")?
            .to_owned();
        let folder = matches!(
            trash::os_limited::metadata(&item)
                .map_err(|error| error.to_string())?
                .size,
            trash::TrashItemSize::Entries(_)
        );
        let metadata = RecycleBinItemMetadata {
            id,
            name,
            original,
            deleted_unix: item.time_deleted,
            folder,
        };
        Ok(Box::new(ShellItem { metadata, item }))
    }

    impl RecycleBinBackend for SystemRecycleBinBackend {
        fn enumerate(&self) -> Result<Vec<Box<dyn RecycleBinItemHandle>>, String> {
            trash::os_limited::list()
                .map_err(|error| error.to_string())?
                .into_iter()
                .map(shell_item)
                .collect()
        }

        fn empty(&self) -> Result<usize, String> {
            let items = trash::os_limited::list().map_err(|error| error.to_string())?;
            let count = items.len();
            if count == 0 {
                return Ok(0);
            }
            trash::os_limited::purge_all(items).map_err(|error| error.to_string())?;
            if !trash::os_limited::is_empty().map_err(|error| error.to_string())? {
                return Err("Windows did not empty the Recycle Bin".into());
            }
            Ok(count)
        }
    }
}

#[cfg(not(windows))]
impl RecycleBinBackend for SystemRecycleBinBackend {
    fn enumerate(&self) -> Result<Vec<Box<dyn RecycleBinItemHandle>>, String> {
        Ok(Vec::new())
    }

    fn empty(&self) -> Result<usize, String> {
        Err("Unsupported: the system Recycle Bin is unavailable".into())
    }
}
