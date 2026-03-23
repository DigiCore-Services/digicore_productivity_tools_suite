//! HotstringDriver - listens for trigger keystrokes and invokes expansion.
//!
//! F1: Trigger-based text expansion
//! F43-F47: Ghost Suggestor integration (Tab accept, Ctrl+Tab cycle)
//! Uses Windows low-level keyboard hook (rdev has a bug: only processes one event).
//!
//! Set DIGICORE_DEBUG=1 to log key events to %TEMP%\digicore_debug.log

use crate::application::clipboard_history;
use crate::application::discovery;
use crate::application::expansion_diagnostics;
use crate::application::expansion_logger;
use crate::application::expansion_stats;
use crate::application::expansion_engine::is_expansion_paused;
use crate::application::ghost_follower;
use crate::application::ghost_suggestor;
use crate::application::template_processor;
use crate::application::variable_input;
use crate::platform::windows_keyboard;
use digicore_core::adapters::platform::input::EnigoInputAdapter;
use digicore_core::adapters::platform::window::WindowsWindowAdapter;
use digicore_core::domain::ports::{ClipboardPort, InputPort, Key, WindowContextPort};
use digicore_core::domain::Snippet;
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Maximum buffer size to prevent unbounded growth.
const MAX_BUFFER_LEN: usize = 256;

const VK_BACK: u16 = 0x08;
const VK_TAB: u16 = 0x09;
const VK_RETURN: u16 = 0x0D;
const VK_SPACE: u16 = 0x20;
const VK_ESCAPE: u16 = 0x1B;
const VK_V: u16 = 0x56;

/// Shared state for hotstring listener.
struct HotstringState {
    library: HashMap<String, Vec<Snippet>>,
    buffer: String,
    input: Arc<EnigoInputAdapter>,
    clipboard: Arc<dyn ClipboardPort>,
    window: WindowsWindowAdapter,
    corpus_service: Option<Arc<crate::application::corpus_generator::CorpusService>>,
    transformer_service: crate::application::transformer_service::TransformerService,
    trie_matcher: Option<crate::application::trie_matcher::TrieMatcher>,
}

static HOTSTRING_STATE: Mutex<Option<Arc<Mutex<HotstringState>>>> = Mutex::new(None);

/// Start the hotstring listener in a background thread.
/// Call with the initial library; use update_library() to refresh.
pub fn start_listener(
    library: HashMap<String, Vec<Snippet>>,
    corpus_service: Option<Arc<crate::application::corpus_generator::CorpusService>>,
    clipboard_repo: Option<Arc<dyn digicore_core::domain::ports::clipboard_repository::ClipboardRepository>>,
) -> anyhow::Result<()> {
    if windows_keyboard::is_listener_running() {
        return Ok(());
    }

    let input = Arc::new(EnigoInputAdapter::new()?);
    #[cfg(target_os = "windows")]
    let clipboard: Arc<dyn ClipboardPort> = Arc::new(digicore_core::adapters::platform::clipboard_windows::WindowsRichClipboardAdapter::new());
    #[cfg(not(target_os = "windows"))]
    let clipboard: Arc<dyn ClipboardPort> = Arc::new(ArboardClipboardAdapter::new()?);
    
    let transformer_service = crate::application::transformer_service::TransformerService::new(
        clipboard.clone(),
        input.clone(),
    );

    let library_clone = library.clone();
    let snippets: Vec<Snippet> = library_clone.values().flatten().cloned().collect();
    let state = Arc::new(Mutex::new(HotstringState {
        library,
        buffer: String::new(),
        input,
        clipboard,
        window: WindowsWindowAdapter::new(),
        corpus_service,
        transformer_service,
        trie_matcher: Some(crate::application::trie_matcher::TrieMatcher::new(&snippets)),
    }));

    *HOTSTRING_STATE.lock().unwrap() = Some(state.clone());

    // Start Ghost Suggestor (F43-F47), Ghost Follower (F48-F59), Clipboard History (F38-F42)
    ghost_suggestor::start(ghost_suggestor::GhostSuggestorConfig::default(), library_clone.clone());
    ghost_follower::start(ghost_follower::GhostFollowerConfig::default(), library_clone);
    crate::application::clipboard_history::start(
        crate::application::clipboard_history::ClipboardHistoryConfig::default(),
        clipboard_repo,
    );

    let callback = Box::new(move |vk_code: u16, ch: Option<char>| on_key(state.clone(), vk_code, ch));

    windows_keyboard::start_listener(callback)?;

    Ok(())
}

/// Check if the hotstring listener is running.
pub fn is_listener_running() -> bool {
    windows_keyboard::is_listener_running()
}

/// Request expansion of content (F52: Ghost Follower double-click).
/// If content has {var:} or {choice:}, sets pending for VariableInputModal instead.
/// Call from UI thread.
pub fn request_expansion(content: String) {
    if variable_input::has_interactive_vars(&content) {
        variable_input::set_pending_from_ghost(content);
        return;
    }
    do_request_expansion(content, None);
}

/// Request expansion with optional target window for restore-before-paste.
pub fn request_expansion_with_target(content: String, target_hwnd: Option<isize>) {
    if variable_input::has_interactive_vars(&content) {
        variable_input::set_pending_from_ghost_with_target(content, target_hwnd);
        return;
    }
    do_request_expansion(content, target_hwnd);
}

/// Request expansion from Ghost Follower double-click. Restores focus to the target
/// window (Sublime, Outlook, etc.) before pasting so content inserts at cursor.
pub fn request_expansion_from_ghost_follower(content: String) {
    let mut target_hwnd = ghost_follower::take_target_hwnd_global();
    #[cfg(target_os = "windows")]
    if let Some(hwnd) = target_hwnd {
        if !crate::platform::windows_window::is_valid_external_hwnd(hwnd) {
            log::debug!(
                "[QuickSearchInsert] rejecting stored target as non-external: {}",
                crate::platform::windows_window::describe_hwnd(hwnd)
            );
            target_hwnd = None;
        }
    }
    let mut target_source = "stored";
    #[cfg(target_os = "windows")]
    if target_hwnd.is_none() {
        // Fallback for tray/overlay timing races: recover latest external foreground window.
        target_hwnd = crate::platform::windows_window::capture_recent_external_foreground_hwnd(500);
        target_source = "fallback-capture";
        if target_hwnd.is_none() {
            target_hwnd = crate::platform::windows_window::capture_recent_external_foreground_hwnd(1500);
            target_source = "fallback-capture-extended";
        }
    }
    #[cfg(target_os = "windows")]
    {
        let target_desc = target_hwnd
            .map(crate::platform::windows_window::describe_hwnd)
            .unwrap_or_else(|| "<none>".to_string());
        let fg = crate::platform::windows_window::describe_foreground_window();
        let msg = format!(
            "[QuickSearchInsert] request_expansion_from_ghost_follower source={} target={} foreground={}",
            target_source, target_desc, fg
        );
        log::debug!("{msg}");
        if target_hwnd.is_none() {
            expansion_diagnostics::push(
                "warn",
                "[QuickSearchInsert] No target window captured; attempting paste in current foreground.".to_string(),
            );
        }
    }
    if variable_input::has_interactive_vars(&content) {
        variable_input::set_pending_from_ghost_with_target(content, target_hwnd);
        return;
    }
    do_request_expansion(content, target_hwnd);
}

/// Perform expansion (no interactive vars). Used after VariableInputModal OK.
/// target_hwnd: when Some, restore focus to this window before paste (for insert-at-cursor).
fn do_request_expansion(content: String, target_hwnd: Option<isize>) {
    #[cfg(target_os = "windows")]
    {
        let target_desc = target_hwnd
            .map(crate::platform::windows_window::describe_hwnd)
            .unwrap_or_else(|| "<none>".to_string());
        let fg = crate::platform::windows_window::describe_foreground_window();
        let msg = format!(
            "[QuickSearchInsert] do_request_expansion start target={} foreground={}",
            target_desc, fg
        );
        log::debug!("{msg}");
    }
    crate::application::clipboard_history::suppress_for_duration(std::time::Duration::from_secs(2));
    if let Ok(guard) = HOTSTRING_STATE.lock() {
        if let Some(ref state) = *guard {
            let state = state.clone();
            std::thread::spawn(move || {
                if let Ok(g) = state.lock() {
                    #[cfg(target_os = "windows")]
                    if let Some(hwnd) = target_hwnd {
                        let before = crate::platform::windows_window::describe_foreground_window();
                        crate::platform::windows_window::restore_foreground_window(hwnd);
                        let after = crate::platform::windows_window::describe_foreground_window();
                        let msg = format!(
                            "[QuickSearchInsert] restore_foreground_window target={} before={} after={}",
                            crate::platform::windows_window::describe_hwnd(hwnd),
                            before,
                            after
                        );
                        log::debug!("{msg}");
                    }
                    let current_clip = g.clipboard.get_text().ok();
                    let clip_history: Vec<String> = clipboard_history::get_entries()
                        .iter()
                        .map(|e| e.content.clone())
                        .collect();
                    let content = template_processor::process(
                        &content,
                        current_clip.as_deref(),
                        &clip_history,
                    );
                    crate::application::expansion_stats::record_expansion(
                        Some("ghost_follower"),
                        content.len(),
                        0,
                    );
                    let saved = g.clipboard.get_text().ok();
                    // Note: request_expansion currently only passes plain text 'content'. 
                    // Future: pass rich text if available from Ghost Follower.
                    if g.clipboard.set_multi(&content, None, None).is_ok() {
                        if g.input.send_ctrl_v().is_ok() {
                            let _ = saved.as_ref().map(|s| g.clipboard.set_text(s));
                        } else {
                            expansion_diagnostics::push(
                                "warn",
                                "[QuickSearchInsert] send_ctrl_v failed; fallback type_text".to_string(),
                            );
                            let _ = g.input.type_text(&content);
                            let _ = saved.as_ref().map(|s| g.clipboard.set_text(s));
                        }
                    } else {
                        expansion_diagnostics::push(
                            "warn",
                            "[QuickSearchInsert] clipboard set_multi failed; fallback type_text".to_string(),
                        );
                        let _ = g.input.type_text(&content);
                    }
                }
                debug_log("follower expand: done");
            });
        }
    }
}

/// Update the library used by the hotstring listener, Ghost Suggestor, and Ghost Follower.
pub fn update_library(library: HashMap<String, Vec<Snippet>>) {
    ghost_suggestor::update_library(library.clone());
    ghost_follower::update_library(library.clone());
    if let Ok(guard) = HOTSTRING_STATE.lock() {
        if let Some(ref state) = *guard {
            if let Ok(mut s) = state.lock() {
                let snippets: Vec<Snippet> = library.values().flatten().cloned().collect();
                s.trie_matcher = Some(crate::application::trie_matcher::TrieMatcher::new(&snippets));
                s.library = library;
            }
        }
    }
}

/// Update the Corpus service config to apply live hotkey mapping updates.
pub fn update_corpus_service(service: Option<Arc<crate::application::corpus_generator::CorpusService>>) {
    if let Ok(guard) = HOTSTRING_STATE.lock() {
        if let Some(ref state) = *guard {
            if let Ok(mut s) = state.lock() {
                s.corpus_service = service;
            }
        }
    }
}

/// Ghost config for sync from host (e.g. Tauri AppState).
#[derive(Clone)]
pub struct GhostConfig {
    pub suggestor_enabled: bool,
    pub suggestor_debounce_ms: u64,
    pub suggestor_display_secs: u64,
    pub suggestor_snooze_duration_mins: u64,
    pub suggestor_offset_x: i32,
    pub suggestor_offset_y: i32,
    pub follower_enabled: bool,
    pub follower_edge_right: bool,
    pub follower_monitor_anchor: usize,
    pub follower_search: String,
    pub follower_hover_preview: bool,
    pub follower_collapse_delay_secs: u64,
}

impl Default for GhostConfig {
    fn default() -> Self {
        Self {
            suggestor_enabled: true,
            suggestor_debounce_ms: 50,
            suggestor_display_secs: 10,
            suggestor_snooze_duration_mins: 5,
            suggestor_offset_x: 0,
            suggestor_offset_y: 20,
            follower_enabled: true,
            follower_edge_right: true,
            follower_monitor_anchor: 0,
            follower_search: String::new(),
            follower_hover_preview: true,
            follower_collapse_delay_secs: 5,
        }
    }
}

/// Sync Discovery config from host state. Starts/stops Discovery.
/// Suggestion callback is set by Tauri setup (notification toast flow).
pub fn sync_discovery_config(enabled: bool, config: discovery::DiscoveryConfig) {
    log::info!(
        "[Hotstring] sync_discovery_config: enabled={} threshold={} lookback={}",
        enabled,
        config.threshold,
        config.lookback_minutes
    );
    if enabled {
        discovery::start(config);
        log::info!("[Hotstring] sync_discovery_config: Discovery started");
    } else {
        discovery::stop();
        log::info!("[Hotstring] sync_discovery_config: Discovery stopped");
    }
}

/// Sync Ghost Suggestor and Ghost Follower config from host state.
pub fn sync_ghost_config(config: GhostConfig) {
    ghost_suggestor::update_config(ghost_suggestor::GhostSuggestorConfig {
        enabled: config.suggestor_enabled,
        debounce_ms: config.suggestor_debounce_ms,
        display_duration_secs: config.suggestor_display_secs,
        snooze_duration_mins: config.suggestor_snooze_duration_mins,
        offset_x: config.suggestor_offset_x,
        offset_y: config.suggestor_offset_y,
    });
    ghost_follower::update_config(ghost_follower::GhostFollowerConfig {
        enabled: config.follower_enabled,
        edge: if config.follower_edge_right {
            ghost_follower::FollowerEdge::Right
        } else {
            ghost_follower::FollowerEdge::Left
        },
        monitor_anchor: match config.follower_monitor_anchor {
            1 => ghost_follower::MonitorAnchor::Secondary,
            2 => ghost_follower::MonitorAnchor::Current,
            _ => ghost_follower::MonitorAnchor::Primary,
        },
        hover_preview: config.follower_hover_preview,
        collapse_delay_secs: config.follower_collapse_delay_secs,
        ..ghost_follower::GhostFollowerConfig::default()
    });
}

fn debug_log(msg: &str) {
    if std::env::var("DIGICORE_DEBUG").as_deref() != Ok("1") {
        return;
    }
    let path = std::path::PathBuf::from(r"C:\Users\pinea\Scripts\AHK_AutoHotKey\digicore\digicore_debug.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{}", msg);
    }
}

/// Returns true if key should be consumed (not passed to app).
fn on_key(state: Arc<Mutex<HotstringState>>, vk_code: u16, ch: Option<char>) -> bool {
    // Top level trace for corpus hotkey debugging
    debug_log(&format!("Raw hook received vk_code={:?}", vk_code));

    // F44: Debounced suggestions - tick to recompute when timer elapsed
    let _ = ghost_suggestor::tick_debounce();

    let mut guard = match state.lock() {
        Ok(g) => g,
        Err(e) => {
            debug_log(&format!("Failed to lock HotstringState: {}", e));
            return false;
        }
    };

    if let Ok(ctx) = guard.window.get_active() {
        discovery::set_window_context(&ctx.process_name, &ctx.title);
    }
    if vk_code == VK_RETURN || vk_code == VK_TAB {
        log::info!(
            "[Hotstring] on_key: Enter/Tab vk={} calling discovery::on_key (buffer len={})",
            vk_code,
            guard.buffer.len()
        );
    }
    discovery::on_key(vk_code, ch);

    // F45: Ghost Suggestor - Tab to accept, Ctrl+Tab to cycle
    if ghost_suggestor::is_enabled() && ghost_suggestor::has_suggestions() {
        let ctrl = windows_keyboard::is_ctrl_pressed();

        if vk_code == VK_ESCAPE {
            ghost_suggestor::dismiss();
            return false; // Pass Escape through
        }
        if vk_code == VK_TAB {
            if ctrl {
                ghost_suggestor::cycle_selection_forward();
                return true; // Consume Ctrl+Tab
            }
            // Tab to accept - backspace typed buffer, then paste expansion
            if let Some((_, content)) = ghost_suggestor::accept_selected() {
                let backspace_len = guard.buffer.len();
                drop(guard);
                expand_suggestion(state.clone(), backspace_len, content);
                return true; // Consume Tab
            }
        }
    }

    if is_expansion_paused() {
        return false;
    }

    let is_ctrl = windows_keyboard::is_ctrl_pressed();
    let is_shift = windows_keyboard::is_shift_pressed();
    let is_alt = windows_keyboard::is_alt_pressed();

    // F22: Clip Transformer Service - Ctrl+Shift+V for plain text paste
    if vk_code == VK_V && is_ctrl && is_shift && !is_alt {
        let _ = guard.transformer_service.paste_plain_text();
        return true; // Consume
    }

    if let Some(ref service) = guard.corpus_service {
        let is_ctrl = windows_keyboard::is_ctrl_pressed();
        let is_alt = windows_keyboard::is_alt_pressed();
        let is_shift = windows_keyboard::is_shift_pressed();

        // Check if Ctrl+Alt+Shift+S is pressed
        let mut mods = service.config.shortcut_modifiers;
        // Legacy upgrade: 0x11 | 0x12 | 0x10 = 0x13 (19)
        if mods == 0x13 {
            mods = 1 | 2 | 4;
        }

        let target_ctrl = (mods & 1) != 0;
        let target_alt = (mods & 2) != 0;
        let target_shift = (mods & 4) != 0;

        let cur_state = format!("ctrl={} alt={} shift={} vk={}", is_ctrl, is_alt, is_shift, vk_code);
        let tgt_state = format!("ctrl={} alt={} shift={} vk={}", target_ctrl, target_alt, target_shift, service.config.shortcut_key);
        debug_log(&format!("Corpus Hook Check -> CURRENT: {} | TARGET: {}", cur_state, tgt_state));

        if is_ctrl == target_ctrl && is_alt == target_alt && is_shift == target_shift {
            if vk_code == service.config.shortcut_key {
                debug_log("Corpus Hook MATCH! Spawning capture thread.");
                
                let window_title = if let Ok(ctx) = guard.window.get_active() {
                    ctx.title
                } else {
                    "UnknownWindow".to_string()
                };

                let svc = service.clone();
                std::thread::spawn(move || {
                    if let Ok(rt) = tokio::runtime::Runtime::new() {
                        let _ = rt.block_on(svc.try_capture(window_title));
                    }
                });
                return true; // Consume hotkey
            }
        }
    }


    // Backspace: remove from buffer
    if vk_code == VK_BACK {
        guard.buffer.pop();
        ghost_suggestor::on_buffer_changed(&guard.buffer, &guard.window.get_active().map(|c| c.process_name).unwrap_or_default());
        debug_log(&format!("key: backspace, buffer={:?}", guard.buffer));
        return false;
    }

    // Printable character - use ch from ToUnicodeEx or vk fallback
    let ch = ch
        .filter(|c| !c.is_control())
        .or_else(|| vk_to_char_fallback(vk_code));
    if let Some(ch) = ch {
        guard.buffer.push(ch);
        if guard.buffer.len() > MAX_BUFFER_LEN {
            guard.buffer.remove(0);
        }
        let process = guard.window.get_active().map(|c| c.process_name).unwrap_or_default();
        ghost_suggestor::on_buffer_changed(&guard.buffer, &process);
        debug_log(&format!("key: vk={} ch={:?} buffer={:?}", vk_code, ch, guard.buffer));

        // Check for trigger match using TriggerMatcher
        let process_name = guard.window.get_active().map(|c| c.process_name.clone()).unwrap_or_default();
        if let Some(res) = crate::application::trigger_matcher::TriggerMatcher::find_match(&guard.library, guard.trie_matcher.as_ref(), &guard.buffer, &process_name) {
            let snippet = res.snippet;
            expansion_diagnostics::push(
                "info",
                format!("Expanded: trigger '{}' in process '{}' (type: {:?})", snippet.trigger, process_name, snippet.trigger_type),
            );
            debug_log(&format!("MATCH: trigger={} expanding", snippet.trigger));
            let trigger_len = res.trigger_length;
            let mut content = snippet.content.clone();
            
            // 1. Expand regex captures if present
            if let Some(caps) = res.captures {
                content = crate::application::trigger_matcher::TriggerMatcher::expand_captures(&content, &caps);
            }

            // 2. Apply case-adaptive transformation if enabled
            if snippet.case_adaptive {
                content = crate::application::trigger_matcher::TriggerMatcher::apply_case(&content, res.matched_case);
            }

            if variable_input::has_interactive_vars(&content) {
                // Defer to main thread for VariableInputModal; do not block hook
                #[cfg(target_os = "windows")]
                let target_hwnd = crate::platform::windows_window::get_foreground_hwnd();
                #[cfg(not(target_os = "windows"))]
                let target_hwnd: Option<isize> = None;

                let trigger = snippet.trigger.clone();
                let expected_title = guard.window.get_active().map(|c| c.title.clone()).unwrap_or_default();

                let (tx, rx) = std::sync::mpsc::channel();
                variable_input::set_pending_from_hotstring(content, trigger_len, target_hwnd, tx);
                drop(guard);
                let state_clone = state.clone();
                std::thread::spawn(move || {
                    if let Ok((Some(expansion), target_hwnd)) = rx.recv() {
                        #[cfg(target_os = "windows")]
                        if let Some(hwnd) = target_hwnd {
                            crate::platform::windows_window::restore_foreground_window(hwnd);
                        }
                        do_expand(state_clone, trigger_len, &expansion, Some(&trigger), None, None, Some(&expected_title));
                    }
                });
                return false;
            }

            let current_clip = guard.clipboard.get_text().ok();
            let clip_history: Vec<String> = clipboard_history::get_entries()
                .iter()
                .map(|e| e.content.clone())
                .collect();
            let expansion = template_processor::process(
                &content,
                current_clip.as_deref(),
                &clip_history,
            );
            let trigger = snippet.trigger.clone();
            let html = snippet.html_content.clone();
            let rtf = snippet.rtf_content.clone();
            let expected_title = guard.window.get_active().map(|c| c.title.clone()).unwrap_or_default();
            drop(guard);

            let state_clone = state.clone();
            std::thread::spawn(move || {
                crate::application::clipboard_history::suppress_for_duration(std::time::Duration::from_secs(2));
                do_expand(state_clone, trigger_len, &expansion, Some(&trigger), html.as_deref(), rtf.as_deref(), Some(&expected_title));
            });
        } else {
            expansion_diagnostics::push(
                "debug",
                format!(
                    "No match for buffer suffix in process '{}' (buffer len={})",
                    process_name,
                    guard.buffer.len()
                ),
            );
        }
    } else {
        // Non-printable (space, enter, etc.) - clear buffer on word boundary
        if vk_code == VK_SPACE || vk_code == VK_RETURN || vk_code == VK_TAB {
            guard.buffer.clear();
            ghost_suggestor::on_buffer_changed("", "");
            ghost_suggestor::dismiss();
        }
    }

    false
}

fn do_expand(
    state: Arc<Mutex<HotstringState>>,
    trigger_len: usize,
    expansion: &str,
    trigger: Option<&str>,
    html: Option<&str>,
    rtf: Option<&str>,
    expected_window_title: Option<&str>,
) {
    if let Some(expected) = expected_window_title {
        // Safety check: verify window hasn't changed since match
        let current = if let Ok(g) = state.lock() {
            g.window.get_active().map(|c| c.title.clone()).unwrap_or_default()
        } else {
            String::new()
        };
        if !current.is_empty() && !expected.is_empty() && current != expected {
            expansion_diagnostics::push(
                "warn",
                format!("Expansion aborted: window changed from '{}' to '{}'", expected, current),
            );
            let process = if let Ok(g) = state.lock() {
                g.window.get_active().map(|c| c.process_name.clone()).unwrap_or_default()
            } else {
                String::new()
            };
            expansion_logger::log_failure(trigger, &current, &process, "window_changed_aborted");
            return;
        }
    }

    expansion_stats::record_expansion(
        trigger,
        expansion.len(),
        trigger_len,
    );
    crate::application::clipboard_history::suppress_for_duration(std::time::Duration::from_secs(2));
    if let Ok(mut g) = state.lock() {
        g.buffer.clear();
        ghost_suggestor::on_buffer_changed("", "");
        for _ in 0..trigger_len {
            let _ = g.input.key_sequence(&[Key::Backspace]);
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        let saved = g.clipboard.get_text().ok();
        let (process, title) = g.window.get_active().map(|c| (c.process_name.clone(), c.title.clone())).unwrap_or_default();
        let mut method = "type";

        if g.clipboard.set_multi(expansion, html, rtf).is_ok() {
            if g.input.send_ctrl_v().is_ok() {
                method = "paste";
                let _ = saved.as_ref().map(|s| g.clipboard.set_text(s));
            } else {
                method = "type_fallback";
                let _ = g.input.type_text(expansion);
                let _ = saved.as_ref().map(|s| g.clipboard.set_text(s));
            }
        } else {
            let _ = g.input.type_text(expansion);
        }
        expansion_logger::log_success(trigger, expansion.len(), &title, &process, method);
    }
    debug_log("expand: done");
}

fn expand_suggestion(state: Arc<Mutex<HotstringState>>, trigger_len: usize, content: String) {
    if variable_input::has_interactive_vars(&content) {
        #[cfg(target_os = "windows")]
        let target_hwnd = crate::platform::windows_window::get_foreground_hwnd();
        #[cfg(not(target_os = "windows"))]
        let target_hwnd: Option<isize> = None;

        let (tx, rx) = std::sync::mpsc::channel();
        variable_input::set_pending_from_hotstring(content, trigger_len, target_hwnd, tx);
        std::thread::spawn(move || {
            if let Ok((Some(expansion), target_hwnd)) = rx.recv() {
                #[cfg(target_os = "windows")]
                if let Some(hwnd) = target_hwnd {
                    crate::platform::windows_window::restore_foreground_window(hwnd);
                }
                do_expand(state, trigger_len, &expansion, None, None, None, None);
            }
        });
        return;
    }
    let current_clip = if let Ok(g) = state.lock() {
        g.clipboard.get_text().ok()
    } else {
        None
    };
    let clip_history: Vec<String> = clipboard_history::get_entries()
        .iter()
        .map(|e| e.content.clone())
        .collect();
    let expansion = template_processor::process(&content, current_clip.as_deref(), &clip_history);
    // Suggestion expansion happens after external interaction; window title may remain the same or change.
    // For now, allow expansion even if title changes slightly if it's a suggestion.
    std::thread::spawn(move || do_expand(state, trigger_len, &expansion, None, None, None, None));
}

/// Find snippet by trigger. Returns (snippet, category) if found and app-lock passes.
pub fn find_snippet<'a>(
    library: &'a HashMap<String, Vec<Snippet>>,
    buffer: &str,
    window: &dyn digicore_core::domain::ports::WindowContextPort,
) -> Option<crate::application::trigger_matcher::MatchResult<'a>> {
    let ctx = window.get_active().ok()?;
    // For simple UI/test calls we don't always have the trie, so pass None
    crate::application::trigger_matcher::TriggerMatcher::find_match(library, None, buffer, &ctx.process_name)
}

/// Fallback: map virtual key code to char for US QWERTY (no Shift).
pub(crate) fn vk_to_char_fallback(vk: u16) -> Option<char> {
    Some(match vk {
        0x20 => ' ',
        0x30 => '0',
        0x31 => '1',
        0x32 => '2',
        0x33 => '3',
        0x34 => '4',
        0x35 => '5',
        0x36 => '6',
        0x37 => '7',
        0x38 => '8',
        0x39 => '9',
        0x41..=0x5A => (vk as u8 + 32) as char, // A-Z -> a-z
        0xBA => ';',
        0xBB => '=',
        0xBC => ',',
        0xBD => '-',
        0xBE => '.',
        0xBF => '/',
        0xC0 => '`',
        0xDB => '[',
        0xDC => '\\',
        0xDD => ']',
        0xDE => '\'',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{find_snippet, vk_to_char_fallback};
    use digicore_core::domain::Snippet;
    use digicore_core::adapters::platform::window::WindowsWindowAdapter;
    use std::collections::HashMap;

    #[test]
    fn test_find_snippet_match_no_app_lock() {
        let mut library = HashMap::new();
        library.insert(
            "Cat".to_string(),
            vec![Snippet::new("dyf", "Did you find")],
        );

        let window = WindowsWindowAdapter::mock("sublime_text.exe", "test");
        let result = find_snippet(&library, "dyf", &window);
        assert!(result.is_some());
        let res = result.unwrap();
        assert_eq!(res.snippet.trigger, "dyf");
        assert_eq!(res.snippet.content, "Did you find");
        assert_eq!(res.category, "Cat");
    }

    #[test]
    fn test_find_snippet_match_buffer_suffix() {
        let mut library = HashMap::new();
        library.insert(
            "Cat".to_string(),
            vec![Snippet::new("sig", "Best regards")],
        );

        let window = WindowsWindowAdapter::mock("notepad.exe", "test");
        let result = find_snippet(&library, "prefix sig", &window);
        assert!(result.is_some());
        assert_eq!(result.unwrap().snippet.trigger, "sig");
    }

    #[test]
    fn test_find_snippet_case_insensitive() {
        let mut library = HashMap::new();
        library.insert(
            "Cat".to_string(),
            vec![Snippet::new("DYF", "Did you find")],
        );

        let window = WindowsWindowAdapter::mock("sublime_text.exe", "test");
        let result = find_snippet(&library, "dyf", &window);
        assert!(result.is_some());
        assert_eq!(result.unwrap().snippet.trigger, "DYF");
    }

    #[test]
    fn test_find_snippet_app_lock_allowed() {
        let mut snip = Snippet::new("sig", "Best regards");
        snip.app_lock = "notepad.exe".to_string();

        let mut library = HashMap::new();
        library.insert("Cat".to_string(), vec![snip]);

        let window = WindowsWindowAdapter::mock("notepad.exe", "test");
        let result = find_snippet(&library, "sig", &window);
        assert!(result.is_some());
    }

    #[test]
    fn test_find_snippet_for_process_app_lock_denied() {
        let mut snip = Snippet::new("sig", "Best regards");
        snip.app_lock = "notepad.exe".to_string();

        let mut library = HashMap::new();
        library.insert("Cat".to_string(), vec![snip]);

        let result = find_snippet(&library, "sig", &WindowsWindowAdapter::mock("chrome.exe", ""));
        assert!(result.is_none());
    }

    #[test]
    fn test_find_snippet_app_lock_multi_allowed() {
        let mut snip = Snippet::new("sig", "Best regards");
        snip.app_lock = "notepad.exe, sublime_text.exe".to_string();

        let mut library = HashMap::new();
        library.insert("Cat".to_string(), vec![snip]);

        assert!(find_snippet(&library, "sig", &WindowsWindowAdapter::mock("sublime_text.exe", "test")).is_some());
        assert!(find_snippet(&library, "sig", &WindowsWindowAdapter::mock("notepad.exe", "test")).is_some());
    }

    #[test]
    fn test_find_snippet_no_match() {
        let mut library = HashMap::new();
        library.insert(
            "Cat".to_string(),
            vec![Snippet::new("dyf", "Did you find")],
        );

        let result = find_snippet(&library, "xyz", &WindowsWindowAdapter::mock("sublime_text.exe", "test"));
        assert!(result.is_none());
    }

    #[test]
    fn test_find_snippet_buffer_shorter_than_trigger() {
        let mut library = HashMap::new();
        library.insert(
            "Cat".to_string(),
            vec![Snippet::new("dyf", "Did you find")],
        );

        let result = find_snippet(&library, "dy", &WindowsWindowAdapter::mock("sublime_text.exe", "test"));
        assert!(result.is_none());
    }

    #[test]
    fn test_vk_to_char_fallback_letters() {
        assert_eq!(vk_to_char_fallback(0x41), Some('a'));
        assert_eq!(vk_to_char_fallback(0x5A), Some('z'));
        assert_eq!(vk_to_char_fallback(0x44), Some('d'));
    }

    #[test]
    fn test_vk_to_char_fallback_digits() {
        assert_eq!(vk_to_char_fallback(0x30), Some('0'));
        assert_eq!(vk_to_char_fallback(0x39), Some('9'));
    }

    #[test]
    fn test_vk_to_char_fallback_special() {
        assert_eq!(vk_to_char_fallback(0x20), Some(' '));
        assert_eq!(vk_to_char_fallback(0xBA), Some(';'));
        assert_eq!(vk_to_char_fallback(0xBF), Some('/'));
    }

    #[test]
    fn test_vk_to_char_fallback_unknown_returns_none() {
        assert_eq!(vk_to_char_fallback(0x01), None);
    }
}
