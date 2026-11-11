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
