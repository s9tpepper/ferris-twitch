use std::path::PathBuf;

use anyhow::bail;
use directories::ProjectDirs;

pub fn get_data_directory(path: Option<&str>) -> anyhow::Result<PathBuf> {
    // TODO: Fix the namespace
    if let Some(project_directories) = ProjectDirs::from("com", "s9tpepper", "FerrisTwitch") {
        let mut data_directory = project_directories.data_dir().to_path_buf();
        if let Some(path) = path {
            data_directory.push(path);
        }

        if !data_directory.exists() {
            std::fs::create_dir_all(&data_directory)?;
        }

        return Ok(data_directory);
    }

    bail!("Could not get data directory")
}
