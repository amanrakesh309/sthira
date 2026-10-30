use std::fs;
use std::path::{Path, PathBuf};

pub fn get_mirror_dir(vault_dir: &Path) -> PathBuf {
    let base = if let Some(parent) = vault_dir.parent() {
        if parent.file_name().is_some_and(|n| n == ".sthira_vault") {
            parent.parent().unwrap_or(parent)
        } else {
            parent
        }
    } else {
        vault_dir
    };

    base.join(".sthira_meta").join("memory_mirrors")
}

pub fn export_project_memory_mirror(
    vault_dir: &Path,
    project_id: &str,
    fingerprint: &str,
    markdown_content: &str,
) -> std::io::Result<PathBuf> {
    let mirror_dir = get_mirror_dir(vault_dir).join("projects");
    fs::create_dir_all(&mirror_dir)?;

    let target_path = mirror_dir.join(format!("{}.md", project_id));
    let header = format!(
        "<!-- STHIRA_DERIVED_MEMORY: DO_NOT_INGEST | basis_fingerprint: {} -->\n",
        fingerprint
    );
    let full_content = format!("{}{}", header, markdown_content);

    fs::write(&target_path, full_content)?;
    Ok(target_path)
}

pub fn export_global_memory_mirror(
    vault_dir: &Path,
    fingerprint: &str,
    markdown_content: &str,
) -> std::io::Result<PathBuf> {
    let mirror_dir = get_mirror_dir(vault_dir);
    fs::create_dir_all(&mirror_dir)?;

    let target_path = mirror_dir.join("global_working_memory.md");
    let header = format!(
        "<!-- STHIRA_DERIVED_MEMORY: DO_NOT_INGEST | basis_fingerprint: {} -->\n",
        fingerprint
    );
    let full_content = format!("{}{}", header, markdown_content);

    fs::write(&target_path, full_content)?;
    Ok(target_path)
}
