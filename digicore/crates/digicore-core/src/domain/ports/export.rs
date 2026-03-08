use crate::domain::value_objects::TableBlock;
use std::path::Path;

/// Port for exporting tabular data.
pub trait TableExportPort: Send + Sync {
    /// Export the table block to a structured file (e.g. CSV).
    fn export_to_csv(&self, table: &TableBlock, output_path: &Path) -> anyhow::Result<()>;
}
