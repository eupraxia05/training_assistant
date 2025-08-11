use std::{fmt::Error, fs, path::{Path, PathBuf}, process::Command};
use std::io::Write;

use latex::{Document, DocumentClass, Element, ParagraphElement, PreambleElement, Section};

use directories::ProjectDirs;

use serde::{Deserialize, Serialize};

use std::io::{Read};

fn generate_document_inner(handout_config_read: impl Read, temp_dir: &Path, out_path: &Path)
  -> Result<(), String>
{
  let tex_path = temp_dir.join("handout.tex");
  let pdf_path = temp_dir.join("handout.pdf");
  let dest_path = out_path.join("handout.pdf");

  let handout_config = 
    serde_json::from_reader::<_, HandoutConfig>(handout_config_read)
    .map_err(|e| e.to_string())?;

  // create a new document, article class
  let mut doc = Document::new(DocumentClass::Article);

  // configure document preamble
  doc.preamble.title(&format!("{} Session Handout for {}", handout_config.date, 
    handout_config.client_name));

  doc.preamble.push(
    PreambleElement::UsePackage {
      package: "handout".into(), 
      argument: None
    }
  );

  // add the title
  doc.push(Element::TitlePage);

  // add the summary section
  let mut summary_section = Section::new("Summary").numbered(false);
  summary_section.push(handout_config.summary.as_str());
  doc.push(summary_section);

  // add the exercises section
  let mut exercises_section = Section::new("Exercises").numbered(false);
  exercises_section.push("\\exercise{Lateral Band Walks}");
  doc.push(exercises_section);

  // ensure all directories exist to the temp directory
  std::fs::create_dir_all(temp_dir).map_err(|e| e.to_string())?;

  // copy the style package to the temp directory
  std::fs::write(temp_dir.join("handout.sty"), include_str!("handout.sty"))
    .map_err(|e| e.to_string())?;

  // generate LaTeX and write it to file
  let rendered = latex::print(&doc).map_err(|e| e.to_string())?;
  fs::write(tex_path.clone(), rendered).map_err(|e| e.to_string())?;

  // configure pdflatex command
  let mut cmd = Command::new("pdflatex");
  cmd.arg(tex_path)
    .arg(format!("-output-directory={}", temp_dir.display()));

  // announce invoking pdflatex
  println!("Executing {:?}", cmd);

  // execute pdflatex command
  let cmd_output = cmd.output()
    .map_err(|e| e.to_string())?;

  // write results to stdio and stderr and flush
  std::io::stdout().write(&cmd_output.stdout).map_err(|e| e.to_string())?;
  std::io::stderr().write(&cmd_output.stderr).map_err(|e| e.to_string())?;
  std::io::stdout().flush().map_err(|e| e.to_string())?;
  std::io::stderr().flush().map_err(|e| e.to_string())?;

  // copy the generated pdf to the destination filepath
  std::fs::copy(pdf_path, dest_path).map_err(|e| e.to_string())?;

  Ok(())
}

pub fn generate_document(handout_path: PathBuf, out_path: PathBuf) 
  -> Result<(), String> 
{
  // determine file paths
  let project_dirs = ProjectDirs::from("", "training_assistant", "handoutgen").unwrap();
  let temp_dir = project_dirs.cache_dir();

  // open and deserialize the handout file
  let handout_file = fs::File::open(handout_path).map_err(|e| e.to_string())?;

  generate_document_inner(handout_file, temp_dir, &out_path)?;

  Ok(())
}

pub fn init_handout(out_path: PathBuf) -> Result<(), String> {
  let file = std::fs::File::create(out_path).map_err(|e| e.to_string())?;
  serde_json::to_writer_pretty(file, &HandoutConfig::default()).map_err(|e| e.to_string())?;

  Ok(())
}

#[derive(Default, Serialize, Deserialize)]
pub struct HandoutConfig {
  pub client_name: String,
  pub date: String,
  pub summary: String,
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

use super::*;

  #[test]
  fn generate_document_test()
  {
    // open and deserialize the handout file
    let handout_config = "{\"client_name\": \"Jane Doe\",\"date\": \"2025-06-06\",\"summary\": \"Good work today!t\"}";

    let temp_dir = tempdir().expect("Failed to create temp dir for test!");
    let temp_dir_2 = tempdir().expect("Failed to create temp dir for test!");

    let result_path = temp_dir_2.path().join("handout.pdf");

    generate_document_inner(handout_config.as_bytes(), temp_dir.path(), temp_dir_2.path()).expect("Failed to generate document!");
    assert!(fs::exists(result_path).expect("Failed to check if file exists!"));
  }
}