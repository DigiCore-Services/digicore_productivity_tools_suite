//! Expansion engine - orchestrates snippet lookup and text injection.
//!
//! F1: Trigger-based text expansion
//! F3: App-lock (restrict snippets to specific applications)
//! F7: Global pause/resume
//!
//! Hotstring detection (keyboard hooks) is Phase 4; this engine performs
//! the expansion when given a matched trigger.

use digicore_core::domain::ports::{ClipboardPort, InputPort, WindowContextPort};
use digicore_core::domain::Snippet;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global pause flag (F7).
static GLOBAL_PAUSED: AtomicBool = AtomicBool::new(false);

/// Check if expansion is globally paused.
pub fn is_expansion_paused() -> bool {
    GLOBAL_PAUSED.load(Ordering::SeqCst)
}

/// Set global pause state (F7).
pub fn set_expansion_paused(paused: bool) {
    GLOBAL_PAUSED.store(paused, Ordering::SeqCst);
}

/// Expansion engine - performs expansion when trigger matches.
pub struct ExpansionEngine<I, C, W> {
    library: HashMap<String, Vec<Snippet>>,
    input: I,
    _clipboard: C,
    window: W,
}

impl<I, C, W> ExpansionEngine<I, C, W>
where
    I: InputPort,
    C: ClipboardPort,
    W: WindowContextPort,
{
    pub fn new(input: I, clipboard: C, window: W) -> Self {
        Self {
            library: HashMap::new(),
            input,
            _clipboard: clipboard,
            window,
        }
    }

    /// Load library (category -> snippets).
    pub fn load_library(&mut self, library: HashMap<String, Vec<Snippet>>) {
        self.library = library;
    }

    /// Check if expansion is globally paused (delegates to module fn).
    pub fn is_paused() -> bool {
        is_expansion_paused()
    }

    /// Find snippet by trigger. Returns (snippet, category) if found and app-lock passes.
    pub fn find_snippet(&self, trigger: &str) -> Option<(&Snippet, &str)> {
        if Self::is_paused() {
            return None;
        }

        let ctx = self.window.get_active().ok()?;
        let process = ctx.process_name.to_lowercase();

        for (category, snippets) in &self.library {
            for snip in snippets {
                if snip.trigger.eq_ignore_ascii_case(trigger) {
                    if !snip.app_lock.is_empty() {
                        let allowed: Vec<&str> = snip.app_lock.split(',').map(|s| s.trim()).collect();
                        if !allowed.is_empty() && !allowed.iter().any(|a| process.contains(&a.to_lowercase())) {
                            continue;
                        }
                    }
                    return Some((snip, category));
                }
            }
        }
        None
    }

    /// Perform expansion: delete trigger (backspaces) and type content.
    /// Caller is responsible for deleting the trigger; we only type the expansion.
    pub fn expand(&self, snippet: &Snippet) -> anyhow::Result<()> {
        if Self::is_paused() {
            return Ok(());
        }
        self.input.type_text(&snippet.content)
    }

    /// Expand by trigger - find snippet and expand. Returns Some(expanded_content) if expanded.
    pub fn expand_trigger(&self, trigger: &str) -> anyhow::Result<Option<String>> {
        if let Some((snippet, _)) = self.find_snippet(trigger) {
            let content = snippet.content.clone();
            self.expand(snippet)?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }
}
