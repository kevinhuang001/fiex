use super::super::item;
use fiex_provider_contracts::{MenuItem, MenuItemKind};

pub(super) fn new_file_items() -> Vec<MenuItem> {
    let mut root = item("newFile", "新建文件", "file-plus", "newFile");
    root.kind = MenuItemKind::Submenu;
    let mut empty = item("newFile.empty", "空白文件 (.txt)", "file-text", "");
    empty.parent_id = Some(root.id.clone());
    let mut office = item("newFile.office", "Office", "application", "");
    office.kind = MenuItemKind::Submenu;
    office.parent_id = Some(root.id.clone());
    let mut docx = item("newFile.docx", "Word 文档 (.docx)", "file-text", "");
    docx.parent_id = Some(office.id.clone());
    let mut xlsx = item(
        "newFile.xlsx",
        "Excel 工作簿 (.xlsx)",
        "file-spreadsheet",
        "",
    );
    xlsx.parent_id = Some(office.id.clone());
    let mut pptx = item(
        "newFile.pptx",
        "PowerPoint 演示文稿 (.pptx)",
        "presentation",
        "",
    );
    pptx.parent_id = Some(office.id.clone());
    vec![root, empty, office, docx, xlsx, pptx]
}
