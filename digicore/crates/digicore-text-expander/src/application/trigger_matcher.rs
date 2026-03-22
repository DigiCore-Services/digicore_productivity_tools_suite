//! TriggerMatcher - decouples matching logic from the hook driver.
//! Supports Suffix and Regex triggers with capture group expansion.

use digicore_core::domain::entities::snippet::{Snippet, TriggerType};
use regex::Regex;
use std::collections::HashMap;

/// Detected casing of the input trigger.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputCase {
    Lower,   // hello
    Upper,   // HELLO
    Title,   // Hello
    Mixed,   // HeLLo
}

/// Result of a trigger match.
pub struct MatchResult<'a> {
    pub snippet: &'a Snippet,
    pub category: &'a str,
    pub captures: Option<Vec<String>>,
    pub trigger_length: usize,
    pub matched_case: InputCase,
}

pub struct TriggerMatcher;

impl TriggerMatcher {
    /// Find a snippet match in the library based on the current buffer and process.
    pub fn find_match<'a>(
        library: &'a HashMap<String, Vec<Snippet>>,
        trie: Option<&crate::application::trie_matcher::TrieMatcher>,
        buffer: &str,
        process_name: &str,
    ) -> Option<MatchResult<'a>> {
        let process = process_name.to_lowercase();

        // 1. Try TrieMatcher for fast literal suffix matching
        if let Some(trie) = trie {
            if let Some(matched_trigger) = trie.find_match(buffer) {
                // Find the snippet in the library
                for (category, snippets) in library {
                    if let Some(snip) = snippets.iter().find(|s| s.trigger == matched_trigger) {
                        // Check AppLock and Smart Suffix
                        if !snip.app_lock.is_empty() {
                            let allowed: Vec<&str> = snip.app_lock.split(',').map(|s| s.trim()).collect();
                            if !allowed.is_empty() && !allowed.iter().any(|a| process.contains(&a.to_lowercase())) {
                                // If app lock fails, we fall back to linear search to see if other matches exist
                                break; 
                            }
                        }

                        if snip.smart_suffix {
                            let trigger_start_idx = buffer.len().saturating_sub(snip.trigger.len());
                            if trigger_start_idx > 0 {
                                if let Some(prev_char) = buffer[..trigger_start_idx].chars().next_back() {
                                    if prev_char.is_alphanumeric() || prev_char == '_' {
                                        break; // Fails smart suffix, fall back
                                    }
                                }
                            }
                        }

                        let matched_text = &buffer[buffer.len() - snip.trigger.len()..];
                        return Some(MatchResult {
                            snippet: snip,
                            category,
                            captures: None,
                            trigger_length: snip.trigger.len(),
                            matched_case: detect_case(matched_text),
                        });
                    }
                }
            }
        }

        // 2. Fallback to linear search for Regex triggers and Trie misses/exclusions
        for (category, snippets) in library {
            for snip in snippets {
                // Skip literal suffixes if Trie was available (already checked or missed)
                if trie.is_some() && snip.trigger_type == TriggerType::Suffix {
                    continue;
                }

                // 1. Check AppLock
                if !snip.app_lock.is_empty() {
                    let allowed: Vec<&str> = snip.app_lock.split(',').map(|s| s.trim()).collect();
                    if !allowed.is_empty() && !allowed.iter().any(|a| process.contains(&a.to_lowercase())) {
                        continue;
                    }
                }

                // 2. Match Trigger
                match snip.trigger_type {
                    TriggerType::Suffix => {
                        let trigger_len = snip.trigger.len();
                        if buffer.len() >= trigger_len {
                            let matched_text = &buffer[buffer.len() - trigger_len..];
                            let matches = if snip.case_sensitive {
                                matched_text == snip.trigger
                            } else {
                                matched_text.eq_ignore_ascii_case(&snip.trigger)
                            };

                            if matches {
                                // Smart Suffix
                                if snip.smart_suffix {
                                    let trigger_start_idx = buffer.len().saturating_sub(snip.trigger.len());
                                    if trigger_start_idx > 0 {
                                        if let Some(prev_char) = buffer[..trigger_start_idx].chars().next_back() {
                                            if prev_char.is_alphanumeric() || prev_char == '_' {
                                                continue;
                                            }
                                        }
                                    }
                                }

                                let matched_text = &buffer[buffer.len() - snip.trigger.len()..];
                                return Some(MatchResult {
                                    snippet: snip,
                                    category,
                                    captures: None,
                                    trigger_length: snip.trigger.len(),
                                    matched_case: detect_case(matched_text),
                                });
                            }
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
                                            trigger_length: full_match.as_str().len(),
                                            matched_case: detect_case(full_match.as_str()),
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

    /// Applies the detected case to the content if case_adaptive is enabled.
    pub fn apply_case(content: &str, case: InputCase) -> String {
        match case {
            InputCase::Lower => content.to_lowercase(),
            InputCase::Upper => content.to_uppercase(),
            InputCase::Title => {
                let mut chars = content.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        let mut result = first.to_uppercase().to_string();
                        result.push_str(&chars.as_str().to_lowercase());
                        result
                    }
                }
            }
            InputCase::Mixed => content.to_string(),
        }
    }
}

/// Detects the casing of a string.
pub fn detect_case(s: &str) -> InputCase {
    if s.is_empty() {
        return InputCase::Mixed;
    }

    let has_upper = s.chars().any(|c| c.is_uppercase());
    let has_lower = s.chars().any(|c| c.is_lowercase());

    if !has_upper {
        InputCase::Lower
    } else if !has_lower {
        InputCase::Upper
    } else {
        // Potential Title Case: First char is upper, rest contains no upper?
        // Actually, let's check if the first letter is upper and the rest are lower.
        let mut chars = s.chars();
        if let Some(first) = chars.next() {
            if first.is_uppercase() && chars.all(|c| !c.is_uppercase()) {
                return InputCase::Title;
            }
        }
        InputCase::Mixed
    }
}
