use std::{fmt::Error, fs, process::Command};

use latex::{Document, DocumentClass, Element, Section};

fn main() {
  match generate_document() {
    Err(e) => {
      println!("Error generating document: {:?}", e);
    },
    Ok(()) => {
      println!("Successfully generated document.");
    }
  }
}

fn generate_document() -> Result<(), String> {
  let mut doc = Document::new(DocumentClass::Article);
  doc.preamble.title("2025-06-02 Session Handout");
  let mut section_1 = Section::new("Section");
  section_1.push("Wow! Look at this fuckin thing my guy");
  doc.push(section_1);
  let rendered = latex::print(&doc).map_err(|e| e.to_string())?;
  fs::write("out.tex", rendered).map_err(|e| e.to_string())?;
  Command::new("pdflatex").arg("out.tex").output().map_err(|e| e.to_string())?;
  Ok(())
}