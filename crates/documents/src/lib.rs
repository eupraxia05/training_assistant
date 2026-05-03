//! A utility library for exporting LaTeX documents.
use directories::ProjectDirs;
use latex::Document;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Exports a LaTeX document to PDF in a given directory.
///
/// * `out_folder` - The directory to put the PDF in.
/// * `file_name` - The file name to use (excluding the .pdf extension)
/// * `doc` - The document to export.
pub fn write_document(
    out_folder: &Path,
    file_name: &str,
    doc: &Document,
) -> Result<(), String> {
    let project_dirs = ProjectDirs::from(
        "",
        "training_assistant",
        "training_assistant",
    )
    .unwrap();
    let temp_dir =
        project_dirs.cache_dir().join("documents");

    let tex_path =
        temp_dir.join(format!("{}.tex", file_name));
    let pdf_path =
        temp_dir.join(format!("{}.pdf", file_name));
    let dest_path =
        out_folder.join(format!("{}.pdf", file_name));

    std::fs::create_dir_all(temp_dir.clone())
        .map_err(|e| e.to_string())?;

    let rendered = latex::print(doc)
        .map_err(|e| e.to_string())?;

    std::fs::write(tex_path.clone(), rendered)
        .map_err(|e| e.to_string())?;

    let mut cmd = Command::new("pdflatex");
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    cmd.arg(format!(
        "-output-directory={}",
        temp_dir.display()
    ))
    .arg(tex_path)
    .arg("-interaction=nonstopmode");

    println!("executing command: {:?}", cmd);

    let cmd_output = cmd.output();

    match cmd_output {
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound
            {
                return Err(
                    "pdflatex not found".into()
                );
            } else {
                return Err(e.to_string());
            }
        }
        Ok(output) => {
            // TODO: make an argument to enable this
            println!("piping command output...");

            std::io::stdout()
                .write(&output.stdout)
                .map_err(|e| e.to_string())?;
            std::io::stderr()
                .write(&output.stderr)
                .map_err(|e| e.to_string())?;
            std::io::stdout()
                .flush()
                .map_err(|e| e.to_string())?;
            std::io::stderr()
                .flush()
                .map_err(|e| e.to_string())?;

            // TODO: only attempt copy if no errors were returned
            println!(
                "copying {:?} to {:?}",
                pdf_path, dest_path
            );

            std::fs::copy(
                pdf_path.clone(),
                dest_path.clone(),
            )
            .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}
