#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    Metadata,
    FoldersRead,
    FoldersCreate,
    FoldersDelete,
    FoldersRename,
    FilesRead,
    FilesWrite,
    FilesCreate,
    FilesDelete,
    FilesRename,
    Network,
    Commands,
    Hooks,
}

impl Capability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::FoldersRead => "folders.read",
            Self::FoldersCreate => "folders.create",
            Self::FoldersDelete => "folders.delete",
            Self::FoldersRename => "folders.rename",
            Self::FilesRead => "files.read",
            Self::FilesWrite => "files.write",
            Self::FilesCreate => "files.create",
            Self::FilesDelete => "files.delete",
            Self::FilesRename => "files.rename",
            Self::Network => "network",
            Self::Commands => "commands",
            Self::Hooks => "hooks",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "metadata" => Self::Metadata,
            "folders.read" => Self::FoldersRead,
            "folders.create" => Self::FoldersCreate,
            "folders.delete" => Self::FoldersDelete,
            "folders.rename" => Self::FoldersRename,
            "files.read" => Self::FilesRead,
            "files.write" => Self::FilesWrite,
            "files.create" => Self::FilesCreate,
            "files.delete" => Self::FilesDelete,
            "files.rename" => Self::FilesRename,
            "network" => Self::Network,
            "commands" => Self::Commands,
            "hooks" => Self::Hooks,
            _ => return None,
        })
    }
}
