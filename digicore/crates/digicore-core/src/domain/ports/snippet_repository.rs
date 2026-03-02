//! SnippetRepository port - load/save/merge snippet library.

use crate::domain::Snippet;
use anyhow::Result;
use std::collections::HashMap;

/// Port for snippet library persistence.
///
/// Implementations: JsonLibraryAdapter, etc.
pub trait SnippetRepository: Send + Sync {
    /// Load library: category name -> snippets.
    fn load(&self, path: &std::path::Path) -> Result<HashMap<String, Vec<Snippet>>>;

    /// Save library.
    fn save(&self, path: &std::path::Path, library: &HashMap<String, Vec<Snippet>>) -> Result<()>;

    /// Merge incoming into existing by trigger; keep newer lastModified.
    fn merge(
        &self,
        existing: &mut HashMap<String, Vec<Snippet>>,
        incoming: HashMap<String, Vec<Snippet>>,
    ) {
        for (category, snippets) in incoming {
            let existing_snippets = existing.entry(category).or_default();
            for snip in snippets {
                if let Some(pos) = existing_snippets
                    .iter()
                    .position(|s| s.trigger == snip.trigger)
                {
                    let existing_lm = existing_snippets[pos].last_modified.as_str();
                    let incoming_lm = snip.last_modified.as_str();
                    if incoming_lm > existing_lm {
                        existing_snippets[pos] = snip;
                    }
                } else {
                    existing_snippets.push(snip);
                }
            }
        }
    }
}
