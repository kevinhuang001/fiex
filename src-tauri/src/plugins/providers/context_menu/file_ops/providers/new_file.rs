use super::super::item;
use fiex_provider_contracts::{MenuItem, MenuItemKind};

pub(super) fn new_file_items(parent_id: Option<&str>) -> Vec<MenuItem> {
    let mut root = item("newFile", "新建文件", "file-plus", "newFile");
    root.kind = MenuItemKind::Submenu;
    match parent_id {
        None => vec![root],
        Some("newFile") => {
            let empty = item("newFile.empty", "空白文件 (.txt)", "file-text", "");
            let mut office = item("newFile.office", "Office", "application", "");
            office.kind = MenuItemKind::Submenu;
            vec![empty, office]
        }
        Some("newFile.office") => vec![
            item("newFile.docx", "Word 文档 (.docx)", "file-text", ""),
            item(
                "newFile.xlsx",
                "Excel 工作簿 (.xlsx)",
                "file-spreadsheet",
                "",
            ),
            item(
                "newFile.pptx",
                "PowerPoint 演示文稿 (.pptx)",
                "presentation",
                "",
            ),
        ],
        Some(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::new_file_items;
    use fiex_provider_contracts::MenuItemKind;

    #[test]
    fn menu_resolves_one_level_at_a_time() {
        let root = new_file_items(None);
        assert_eq!(root.len(), 1);
        assert_eq!(root[0].id, "newFile");
        assert_eq!(root[0].kind, MenuItemKind::Submenu);

        let children = new_file_items(Some("newFile"));
        assert_eq!(
            children
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["newFile.empty", "newFile.office"]
        );
        let office = new_file_items(Some("newFile.office"));
        assert_eq!(
            office
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["newFile.docx", "newFile.xlsx", "newFile.pptx"]
        );
        assert!(new_file_items(Some("unknown")).is_empty());
    }
}
