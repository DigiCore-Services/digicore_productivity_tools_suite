//! TriggerMatcher - decouples matching logic from the hook driver.
//! Supports Suffix and Regex triggers with capture group expansion.

use digicore_core::domain::entities::snippet::{Snippet, TriggerType};
use regex::Regex;
use std::collections::HashMap;

/// Result of a trigger match.
pub struct MatchResult<'a> {
    pub snippet: &'a Snippet,
    pub category: &'a str,
    pub captures: Option<Vec<String>>,
    pub trigger_length: usize,
}

pub struct TriggerMatcher;

impl TriggerMatcher {
    /// Find a snippet match in the library based on the current buffer and process.
    pub fn find_match<'a>(
        library: &'a HashMap<String, Vec<Snippet>>,
        buffer: &str,
        process_name: &str,
    ) -> Option<MatchResult<'a>> {
        let process = process_name.to_lowercase();

        for (category, snippets) in library {
            for snip in snippets {
                // 1. Check AppLock
                if !snip.app_lock.is_empty() {
                    let allowed: Vec<&str> = snip.app_lock.split(',').map(|s| s.trim()).collect();
                    if !allowed.is_empty() && !allowed.iter().any(|a| process.contains(&a.to_lowercase())) {
                        // Diagnostic logging for trace (optional, can be very verbose)
                        continue;
                    }
                }

                // 2. Match Trigger
                match snip.trigger_type {
                    TriggerType::Suffix => {
                        if buffer.len() >= snip.trigger.len()
                            && buffer[buffer.len() - snip.trigger.len()..].eq_ignore_ascii_case(&snip.trigger)
                        {
                            return Some(MatchResult {
                                snippet: snip,
                                category,
                                captures: None,
                                trigger_length: snip.trigger.len(),
                            });
                        }
                    }
                    TriggerType::Regex => {
                        let regex_str = if snip.trigger.ends_with('$') {
                            snip.trigger.clone()
                        } else {
                            format!("{}$", snip.trigger)
                        };

                        match Regex::new(&regex_str) {
                            Ok(re) => {
                                if let Some(caps) = re.captures(buffer) {
                                    if let Some(full_match) = caps.get(0) {
                                        let captures = caps
                                            .iter()
                                            .skip(1)
                                            .map(|m| m.map(|mat| mat.as_str().to_string()).unwrap_or_default())
                                            .collect();
                                        
                                        return Some(MatchResult {
                                            snippet: snip,
                                            category,
                                            captures: Some(captures),
                                            trigger_length: full_match.len(),
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                crate::application::expansion_diagnostics::push(
                                    "error",
                                    format!("Invalid regex in snippet '{}': {}", snip.trigger, e)
                                );
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Expands capture group placeholders (e.g. $1) in the expansion content.
    pub fn expand_captures(content: &str, captures: &[String]) -> String {
        let mut expanded = content.to_string();
        for (i, cap) in captures.iter().enumerate() {
            let placeholder = format!("${}", i + 1);
            expanded = expanded.replace(&placeholder, cap);
        }
        expanded
    }
}
