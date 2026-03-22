use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use digicore_core::domain::entities::snippet::{Snippet, TriggerType};
use std::collections::HashMap;

/// TrieMatcher uses Aho-Corasick to match literal (Suffix) triggers in O(1) time.
pub struct TrieMatcher {
    /// Case-sensitive automaton for literal triggers
    sensitive_ac: Option<AhoCorasick>,
    /// Snippet IDs mapped to match indices in sensitive_ac
    sensitive_map: Vec<String>, // Index in Vec corresponds to AC match ID

    /// Case-insensitive automaton for literal triggers
    insensitive_ac: Option<AhoCorasick>,
    /// Snippet IDs mapped to match indices in insensitive_ac
    insensitive_map: Vec<String>,
}

impl TrieMatcher {
    pub fn new(snippets: &[Snippet]) -> Self {
        let mut sensitive_triggers = Vec::new();
        let mut sensitive_ids = Vec::new();
        let mut insensitive_triggers = Vec::new();
        let mut insensitive_ids = Vec::new();

        for snippet in snippets {
            if let TriggerType::Suffix = snippet.trigger_type {
                if snippet.case_sensitive {
                    sensitive_triggers.push(snippet.trigger.clone());
                    sensitive_ids.push(snippet.trigger.clone());
                } else {
                    insensitive_triggers.push(snippet.trigger.clone());
                    insensitive_ids.push(snippet.trigger.clone());
                }
            }
        }

        let sensitive_ac = if !sensitive_triggers.is_empty() {
            AhoCorasickBuilder::new()
                .match_kind(MatchKind::LeftmostLongest)
                .ascii_case_insensitive(false)
                .build(&sensitive_triggers)
                .ok()
        } else {
            None
        };

        let insensitive_ac = if !insensitive_triggers.is_empty() {
            AhoCorasickBuilder::new()
                .match_kind(MatchKind::LeftmostLongest)
                .ascii_case_insensitive(true)
                .build(&insensitive_triggers)
                .ok()
        } else {
            None
        };

        Self {
            sensitive_ac,
            sensitive_map: sensitive_ids,
            insensitive_ac,
            insensitive_map: insensitive_ids,
        }
    }

    /// Finds the best match in the given input.
    /// Returns the Snippet ID if matched.
    pub fn find_match(&self, input: &str) -> Option<String> {
        let mut best_match: Option<(usize, String)> = None; // (Match length, Snippet ID)

        // Check sensitive AC
        if let Some(ref ac) = self.sensitive_ac {
            if let Some(mat) = ac.find_iter(input).last() {
                // We want the match that ENDS at the end of the input (suffix)
                if mat.end() == input.len() {
                    let id = &self.sensitive_map[mat.pattern().as_usize()];
                    best_match = Some((mat.len(), id.clone()));
                }
            }
        }

        // Check insensitive AC
        if let Some(ref ac) = self.insensitive_ac {
            if let Some(mat) = ac.find_iter(input).last() {
                if mat.end() == input.len() {
                    let id = &self.insensitive_map[mat.pattern().as_usize()];
                    // If we have a sensitive match already, only override if this one is longer (more specific)
                    if let Some((best_len, _)) = best_match {
                        if mat.len() > best_len {
                            best_match = Some((mat.len(), id.clone()));
                        }
                    } else {
                        best_match = Some((mat.len(), id.clone()));
                    }
                }
            }
        }

        best_match.map(|(_, id)| id)
    }
}
