use super::common::resource_kind;
use crate::Localization;
use fiex_provider_contracts::{
    InfoCompute, InfoFieldDescriptor, InfoProviderPresentation, InfoSubsectionDescriptor,
    InfoSubsectionMatch, ResourceMetadata,
};
use std::{fs, path::Path};

fn field(id: &str, label: String, compute: InfoCompute) -> InfoFieldDescriptor {
    InfoFieldDescriptor {
        id: id.into(),
        label,
        compute,
    }
}

fn subsection(
    id: &str,
    fields: impl IntoIterator<Item = InfoFieldDescriptor>,
) -> InfoSubsectionDescriptor {
    InfoSubsectionDescriptor {
        id: id.into(),
        r#match: None,
        fields: fields.into_iter().collect(),
    }
}

pub fn metadata_descriptors(
    display: &str,
    is_local: bool,
    stat: Option<&fs::Metadata>,
    projected: Option<&ResourceMetadata>,
    localization: &Localization,
) -> InfoProviderPresentation {
    let kind = resource_kind(stat, projected);
    let name = projected
        .map(|value| value.name.clone())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| super::common::fallback_name(display));
    let mut basic = vec![
        field("name", localization.text("名称"), InfoCompute::OnOpen),
        field("kind", localization.text("种类"), InfoCompute::OnOpen),
        field("location", localization.text("位置"), InfoCompute::OnOpen),
    ];
    match kind {
        "directory" => {}
        "symlink" | "link" => {
            if is_local {
                basic.push(field(
                    "linkTarget",
                    localization.text("链接目标"),
                    InfoCompute::OnOpen,
                ));
                basic.push(field(
                    "targetExists",
                    localization.text("目标存在"),
                    InfoCompute::OnOpen,
                ));
            }
        }
        _ => {
            if projected
                .and_then(|value| value.extension.as_deref())
                .is_some_and(|value| !value.is_empty())
                || Path::new(&name)
                    .extension()
                    .is_some_and(|value| !value.is_empty())
            {
                basic.push(field(
                    "extension",
                    localization.text("扩展名"),
                    InfoCompute::OnOpen,
                ));
            }
            if projected
                .and_then(|value| value.mime.as_deref())
                .is_some_and(|value| !value.is_empty())
            {
                basic.push(field(
                    "mime",
                    localization.text("MIME 类型"),
                    InfoCompute::OnOpen,
                ));
            }
            if projected.and_then(|value| value.size).is_some()
                || stat.is_some_and(|value| value.is_file())
            {
                basic.push(field(
                    "size",
                    localization.text("大小"),
                    InfoCompute::OnOpen,
                ));
            }
        }
    }
    let mut subsections = vec![subsection("basic", basic)];

    let mut times = Vec::new();
    if stat.is_some_and(|metadata| metadata.created().is_ok()) {
        times.push(field(
            "created",
            localization.text("创建时间"),
            InfoCompute::OnOpen,
        ));
    }
    if projected.and_then(|metadata| metadata.modified).is_some()
        || stat.is_some_and(|metadata| metadata.modified().is_ok())
    {
        times.push(field(
            "modified",
            localization.text("修改时间"),
            InfoCompute::OnOpen,
        ));
    }
    if stat.is_some_and(|metadata| metadata.accessed().is_ok()) {
        times.push(field(
            "accessed",
            localization.text("上次访问时间"),
            InfoCompute::OnOpen,
        ));
    }
    #[cfg(unix)]
    if is_local && stat.is_some() {
        times.push(field(
            "changed",
            localization.text("元数据变更时间"),
            InfoCompute::OnOpen,
        ));
    }
    if !times.is_empty() {
        subsections.push(subsection("times", times));
    }

    let mut filesystem = Vec::new();
    if stat.is_some() {
        filesystem.push(field(
            "readonly",
            localization.text("只读"),
            InfoCompute::OnOpen,
        ));
    }
    if is_local {
        filesystem.push(field(
            "hidden",
            localization.text("隐藏"),
            InfoCompute::OnOpen,
        ));
    }
    if is_local && stat.is_some() {
        filesystem.push(field(
            "permissions",
            localization.text("权限"),
            InfoCompute::OnOpen,
        ));
        #[cfg(unix)]
        filesystem.extend([
            field("owner", localization.text("所有者"), InfoCompute::OnOpen),
            field("group", localization.text("用户组"), InfoCompute::OnOpen),
            field("fileId", localization.text("文件标识"), InfoCompute::OnOpen),
            field(
                "hardLinks",
                localization.text("硬链接数"),
                InfoCompute::OnOpen,
            ),
            field(
                "allocatedSize",
                localization.text("实际占用空间"),
                InfoCompute::OnOpen,
            ),
        ]);
    }
    if !filesystem.is_empty() {
        subsections.push(subsection("filesystem", filesystem));
    }

    if kind == "directory" && is_local {
        let mut directory_stats = subsection(
            "directoryStats",
            [
                field(
                    "directFiles",
                    localization.text("直接文件数"),
                    InfoCompute::OnDemand,
                ),
                field(
                    "directDirectories",
                    localization.text("直接文件夹数"),
                    InfoCompute::OnDemand,
                ),
                field(
                    "directOther",
                    localization.text("直接其他节点数"),
                    InfoCompute::OnDemand,
                ),
                field(
                    "recursiveFiles",
                    localization.text("递归文件总数"),
                    InfoCompute::OnDemand,
                ),
                field(
                    "recursiveDirectories",
                    localization.text("递归文件夹总数"),
                    InfoCompute::OnDemand,
                ),
                field(
                    "recursiveSize",
                    localization.text("递归文件总大小"),
                    InfoCompute::OnDemand,
                ),
                field(
                    "recursiveAllocatedSize",
                    localization.text("递归实际占用空间"),
                    InfoCompute::OnDemand,
                ),
                field(
                    "symbolicLinks",
                    localization.text("符号链接数"),
                    InfoCompute::OnDemand,
                ),
            ],
        );
        directory_stats.r#match = Some(InfoSubsectionMatch {
            kinds: vec!["directory".into()],
            ..Default::default()
        });
        subsections.push(directory_stats);
    }
    InfoProviderPresentation {
        title: localization.text("元数据"),
        subsections,
    }
}
