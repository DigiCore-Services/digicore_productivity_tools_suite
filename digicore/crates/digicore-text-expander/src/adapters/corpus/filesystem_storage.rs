use digicore_core::domain::ports::CorpusStoragePort;
use std::fs;
use std::path::PathBuf;

pub struct FileSystemCorpusStorageAdapter {
    output_dir: String,
}

impl FileSystemCorpusStorageAdapter {
    pub fn new(output_dir: String) -> Self {
        Self { output_dir }
    }
}

impl CorpusStoragePort for FileSystemCorpusStorageAdapter {
    fn save_image(&self, data: &[u8], filename_prefix: &str, extension: &str) -> anyhow::Result<PathBuf> {
        // Ensure the directory exists relative to the workspace root or current dir
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let workspace_dir = PathBuf::from(manifest_dir).join("../../");
        let target_dir = workspace_dir.join(&self.output_dir);

        if !target_dir.exists() {
            fs::create_dir_all(&target_dir)?;
        }

        let filename = format!("{}.{}", filename_prefix, extension);
        let dst_path = target_dir.join(&filename);

        fs::write(&dst_path, data)?;
        
        Ok(dst_path)
    }
}
