//! Основная часть документа `word/document.xml`. Оформление прямое, без отдельной
//! таблицы стилей: так файл короче и одинаково выглядит в Word, LibreOffice и Pages.

use crate::protocol::Block;

use super::xml_text::xml_text;

// Размеры в WordprocessingML: шрифт в половинах пункта, отступы и ширины в
// двадцатых долях пункта. Лист A4 с полями 2 см.
const TITLE_SIZE: u32 = 36;
const PAGE_WIDTH: u32 = 11906;
const PAGE_HEIGHT: u32 = 16838;
const MARGIN: u32 = 1134;
const COLUMN_WIDTHS: [u32; 3] = [4818, 2410, 2410];

pub fn document_xml(blocks: &[Block]) -> String {
    let mut body = String::new();
    for block in blocks {
        match block {
            Block::Title { text } => body.push_str(&format!(
                r#"<w:p><w:pPr><w:spacing w:after="240"/></w:pPr>{}</w:p>"#,
                run(text, &format!(r#"<w:b/><w:sz w:val="{TITLE_SIZE}"/>"#))
            )),
            Block::Fact { label, value } => body.push_str(&format!(
                "<w:p>{}{}</w:p>",
                run(&format!("{label}: "), ""),
                run(value, "")
            )),
            Block::Table { header, rows } => {
                body.push_str(&table(header, rows));
                // Пустой абзац отделяет таблицу от ходов; без абзаца после таблицы
                // в конце тела Word считает документ повреждённым.
                body.push_str("<w:p/>");
            }
            Block::Turn {
                speaker,
                time,
                text,
            } => {
                body.push_str(&format!(
                    r#"<w:p><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="60"/></w:pPr>{}{}</w:p>"#,
                    run(speaker, "<w:b/>"),
                    run(&format!(", {time}"), "")
                ));
                body.push_str(&format!("<w:p>{}</w:p>", run(text, "")));
            }
        }
    }
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
            "<w:body>{body}",
            r#"<w:sectPr><w:pgSz w:w="{w}" w:h="{h}"/>"#,
            r#"<w:pgMar w:top="{m}" w:right="{m}" w:bottom="{m}" w:left="{m}" w:header="709" w:footer="709" w:gutter="0"/>"#,
            "</w:sectPr></w:body></w:document>"
        ),
        body = body,
        w = PAGE_WIDTH,
        h = PAGE_HEIGHT,
        m = MARGIN
    )
}

fn run(text: &str, props: &str) -> String {
    let props = if props.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{props}</w:rPr>")
    };
    format!(
        r#"<w:r>{props}<w:t xml:space="preserve">{}</w:t></w:r>"#,
        xml_text(text)
    )
}

fn table(header: &[String], rows: &[Vec<String>]) -> String {
    let border = r#"w:val="single" w:sz="4" w:space="0" w:color="A0A0A0""#;
    let mut out = format!(
        concat!(
            r#"<w:tbl><w:tblPr><w:tblW w:w="{w}" w:type="dxa"/><w:tblBorders>"#,
            "<w:top {b}/><w:left {b}/><w:bottom {b}/><w:right {b}/>",
            "<w:insideH {b}/><w:insideV {b}/>",
            r#"</w:tblBorders><w:tblLayout w:type="fixed"/></w:tblPr><w:tblGrid>"#
        ),
        w = COLUMN_WIDTHS.iter().sum::<u32>(),
        b = border
    );
    for width in COLUMN_WIDTHS {
        out.push_str(&format!(r#"<w:gridCol w:w="{width}"/>"#));
    }
    out.push_str("</w:tblGrid>");
    out.push_str(&table_row(header, "<w:b/>"));
    for row in rows {
        out.push_str(&table_row(row, ""));
    }
    out.push_str("</w:tbl>");
    out
}

fn table_row(cells: &[String], props: &str) -> String {
    let mut out = String::from("<w:tr>");
    for (i, cell) in cells.iter().enumerate() {
        let width = COLUMN_WIDTHS.get(i).copied().unwrap_or(COLUMN_WIDTHS[0]);
        let align = if i == 0 {
            ""
        } else {
            r#"<w:pPr><w:jc w:val="right"/></w:pPr>"#
        };
        out.push_str(&format!(
            r#"<w:tc><w:tcPr><w:tcW w:w="{width}" w:type="dxa"/></w:tcPr><w:p>{align}{}</w:p></w:tc>"#,
            run(cell, props)
        ));
    }
    out.push_str("</w:tr>");
    out
}
