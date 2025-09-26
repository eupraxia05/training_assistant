use std::path::PathBuf;
use latex::{Document, DocumentClass, Element};

pub fn create_invoice(out_path: PathBuf) {
    println!("creating invoice at {:?}", out_path);
    let mut doc = Document::new(DocumentClass::Article);
    doc.preamble.title("Invoice!");

    doc.push(Element::TitlePage);
}
