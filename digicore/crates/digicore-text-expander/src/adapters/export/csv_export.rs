use digicore_core::domain::ports::export::TableExportPort;
use digicore_core::domain::value_objects::TableBlock;
use std::path::Path;
use std::fs::File;
use csv::Writer;

pub struct CsvTableExportAdapter;

impl CsvTableExportAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CsvTableExportAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TableExportPort for CsvTableExportAdapter {
    fn export_to_csv(&self, table: &TableBlock, output_path: &Path) -> anyhow::Result<()> {
        let file = File::create(output_path)?;
        let mut wtr = Writer::from_writer(file);

        // Export headers
        for row in &table.headers {
            wtr.write_record(row)?;
        }

        // Export body
        for row in &table.body {
            wtr.write_record(row)?;
        }

        // Export footers
        for row in &table.footers {
            wtr.write_record(row)?;
        }

        wtr.flush()?;
        Ok(())
    }
}
