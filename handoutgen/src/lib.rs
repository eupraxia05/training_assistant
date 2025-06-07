use std::{fmt::Error, fs, path::PathBuf, process::Command};
use std::io::Write;

use latex::{Document, DocumentClass, Element, Section};

use directories::ProjectDirs;

use serde::{Deserialize, Serialize};

pub fn generate_document(handout_path: PathBuf, out_path: PathBuf) -> Result<(), String> {
  let handout_file = fs::File::open(handout_path).map_err(|e| e.to_string())?;

  let handout_config = serde_json::from_reader::<_, HandoutConfig>(handout_file).map_err(|e| e.to_string())?;

  let mut doc = Document::new(DocumentClass::Article);
  let mut section_1 = Section::new(&format!("{} Session Handout for {}", handout_config.date, handout_config.client_name));
  section_1.push("Wow! Look at this fuckin thing my guy");
  doc.push(section_1);
  let rendered = latex::print(&doc).map_err(|e| e.to_string())?;

  let project_dirs = ProjectDirs::from("", "training_assistant", "handoutgen").unwrap();
  let temp_dir = project_dirs.cache_dir();
  let tex_path = temp_dir.join("handout.tex");
  let pdf_path = temp_dir.join("handout.pdf");
  let dest_path = out_path.join("handout.pdf");

  std::fs::create_dir_all(temp_dir).map_err(|e| e.to_string())?;

  fs::write(tex_path.clone(), rendered).map_err(|e| e.to_string())?;

  let mut cmd = Command::new("pdflatex");
  
  cmd.arg(tex_path)
    .arg(format!("-output-directory={}", temp_dir.display()))
    .arg("-verbose");

  println!("Executing {:?}", cmd);

  let cmd_output = cmd.output()
    .map_err(|e| e.to_string())?;

  std::io::stdout().write(&cmd_output.stdout).map_err(|e| e.to_string())?;
  std::io::stderr().write(&cmd_output.stderr).map_err(|e| e.to_string())?;

  std::io::stdout().flush().map_err(|e| e.to_string())?;
  std::io::stderr().flush().map_err(|e| e.to_string())?;

  std::fs::copy(pdf_path, dest_path).map_err(|e| e.to_string())?;

  Ok(())
}

pub fn init_handout(out_path: PathBuf) -> Result<(), String> {
  println!("opening {}", out_path.display());
  let file = std::fs::File::create(out_path).map_err(|e| e.to_string())?;
  serde_json::to_writer_pretty(file, &HandoutConfig::default()).map_err(|e| e.to_string())?;

  Ok(())
}

#[derive(Default, Serialize, Deserialize)]
struct HandoutConfig {
  client_name: String,
  date: String,
}