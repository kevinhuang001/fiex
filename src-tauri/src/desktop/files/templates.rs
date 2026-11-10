//! Small, valid Office Open XML starter documents embedded in the app.

pub(super) fn bytes(template: &str) -> Option<&'static [u8]> {
    match template {
        "docx" => Some(include_bytes!("../../../resources/templates/blank.docx")),
        "xlsx" => Some(include_bytes!("../../../resources/templates/blank.xlsx")),
        "pptx" => Some(include_bytes!("../../../resources/templates/blank.pptx")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::bytes;
    use std::io::{Cursor, Read};

    fn check_package(format: &str, required: &[&str]) {
        let data = bytes(format).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(data)).unwrap();
        for name in required {
            let mut part = archive
                .by_name(name)
                .unwrap_or_else(|_| panic!("missing {name}"));
            let mut contents = Vec::new();
            part.read_to_end(&mut contents).unwrap();
            assert!(!contents.is_empty(), "empty OOXML part: {name}");
        }
    }

    #[test]
    fn blank_office_templates_are_readable_ooxml_packages() {
        check_package(
            "docx",
            &["[Content_Types].xml", "_rels/.rels", "word/document.xml"],
        );
        check_package(
            "xlsx",
            &[
                "[Content_Types].xml",
                "_rels/.rels",
                "xl/workbook.xml",
                "xl/worksheets/sheet1.xml",
            ],
        );
        check_package(
            "pptx",
            &[
                "[Content_Types].xml",
                "_rels/.rels",
                "ppt/presentation.xml",
                "ppt/slides/slide1.xml",
            ],
        );
        assert!(bytes("txt").is_none());
    }
}
