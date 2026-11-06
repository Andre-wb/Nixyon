//! Протокол в формате Word (DOCX).

mod document_xml;
mod package;
mod xml_text;

use crate::protocol::Block;

pub use document_xml::document_xml;

pub fn render_docx(blocks: &[Block]) -> std::io::Result<Vec<u8>> {
    package::package(&document_xml(blocks))
}
