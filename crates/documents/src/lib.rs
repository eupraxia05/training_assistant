use directories::ProjectDirs;
use latex::Document;
use std::io::Write;
use std::path::Path;
use std::process::Command;

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

    println!("temp dir: {:?}", temp_dir);

    let tex_path =
        temp_dir.join(format!("{}.tex", file_name));
    let pdf_path =
        temp_dir.join(format!("{}.pdf", file_name));
    let dest_path =
        out_folder.join(format!("{}.pdf", file_name));

    std::fs::create_dir_all(temp_dir.clone())
        .map_err(|e| e.to_string())?;

    println!("rendering latex document...");
    let rendered = latex::print(doc)
        .map_err(|e| e.to_string())?;
    
    println!(
        "writing latex document to {:?}...",
        tex_path
    );
    std::fs::write(tex_path.clone(), rendered)
        .map_err(|e| e.to_string())?;

    let mut cmd = Command::new("pdflatex");
    cmd.arg(format!(
        "-output-directory={}",
        temp_dir.display()
    ))
    .arg(tex_path);

    println!("Executing {:?}", cmd);

    let cmd_output =
        cmd.output().map_err(|e| e.to_string())?;

    println!("piping command output...");

    std::io::stdout()
        .write(&cmd_output.stdout)
        .map_err(|e| e.to_string())?;
    std::io::stderr()
        .write(&cmd_output.stderr)
        .map_err(|e| e.to_string())?;
    std::io::stdout()
        .flush()
        .map_err(|e| e.to_string())?;
    std::io::stderr()
        .flush()
        .map_err(|e| e.to_string())?;

    println!(
        "copying generated pdf at {:?} to {:?}",
        pdf_path, dest_path
    );

    std::fs::copy(pdf_path, dest_path)
        .map_err(|e| e.to_string())?;

    Ok(())
}
