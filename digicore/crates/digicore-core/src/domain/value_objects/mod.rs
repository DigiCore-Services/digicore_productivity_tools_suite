//! Value objects - immutable domain values.

pub mod last_modified;
mod extraction;
mod corpus;


pub use last_modified::LastModified;
pub use extraction::{ExtractionSource, ExtractionMimeType, ExtractionResult, TableBlock, SemanticEntity};
pub use corpus::CorpusConfig;

