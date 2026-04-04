//! Appearance transparency rules: load/save, effective rule resolution, Windows layered enforcement, and startup loop.

use std::collections::{BTreeSet, HashSet};
#[cfg(target_os = "windows")]
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::ports::{storage_keys, StoragePort};

use crate::AppearanceTransparencyRuleDto;

#[cfg(target_os = "windows")]
use windows::core::BOOL;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetLayeredWindowAttributes, GetWindowLongW, GetWindowThreadProcessId, IsWindowVisible,
    SetLayeredWindowAttributes, SetWindowLongW, GWL_EXSTYLE, LWA_ALPHA, WS_EX_LAYERED,
};

pub(crate) fn load_appearance_rules(storage: &JsonFileStorageAdapter) -> Vec<AppearanceTransparencyRuleDto> {
    storage
        .get(storage_keys::APPEARANCE_TRANSPARENCY_RULES_JSON)
        .and_then(|s| serde_json::from_str::<Vec<AppearanceTransparencyRuleDto>>(&s).ok())
        .unwrap_or_default()
}

pub(crate) fn normalize_process_key(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .trim_end_matches(".exe")
        .to_string()
}

pub(crate) fn sort_appearance_rules_deterministic(rules: &mut [AppearanceTransparencyRuleDto]) {
    rules.sort_by(|a, b| {
        b.enabled
            .cmp(&a.enabled)
            .then_with(|| {
                normalize_process_key(&a.app_process).cmp(&normalize_process_key(&b.app_process))
            })
            .then_with(|| a.app_process.to_ascii_lowercase().cmp(&b.app_process.to_ascii_lowercase()))
            .then_with(|| a.opacity.cmp(&b.opacity))
    });
}

pub(crate) fn effective_rules_for_enforcement(
    mut rules: Vec<AppearanceTransparencyRuleDto>,
) -> Vec<AppearanceTransparencyRuleDto> {
    sort_appearance_rules_deterministic(&mut rules);
    let mut seen = HashSet::new();
    let mut effective = Vec::new();
    for rule in rules {
        let key = normalize_process_key(&rule.app_process);
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        if rule.enabled {
            effective.push(rule);
        }
    }
    effective
}

pub(crate) fn save_appearance_rules(rules: &[AppearanceTransparencyRuleDto]) -> Result<(), String> {
    let mut storage = JsonFileStorageAdapter::load();
    let serialized = serde_json::to_string(rules).map_err(|e| e.to_string())?;
    storage.set(storage_keys::APPEARANCE_TRANSPARENCY_RULES_JSON, &serialized);
    storage
        .persist_if_safe()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub(crate) fn enforce_appearance_transparency_rules() {
    let now = std::time::Instant::now();
    let storage = JsonFileStorageAdapter::load();
    let rules = load_appearance_rules(&storage);
    let effective = effective_rules_for_enforcement(rules);
    if effective.is_empty() {
        return;
    }
    log::debug!("[Appearance] Starting enforcement for {} rules", effective.len());
    for rule in effective {
        let _ = apply_process_transparency(
            &rule.app_process,
            Some(rule.opacity.clamp(20, 255) as u8),
        );
    }
    log::debug!("[Appearance] Enforcement cycle completed in {:?}", now.elapsed());
}

#[cfg(target_os = "windows")]
fn transparency_cache() -> &'static Mutex<HashMap<isize, u8>> {
    static CACHE: OnceLock<Mutex<HashMap<isize, u8>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(target_os = "windows")]
struct TransparencyApplyContext {
    target_pids: std::collections::HashSet<u32>,
    alpha: Option<u8>,
    applied: u32,
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_apply_transparency(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if lparam.0 == 0 {
        return BOOL(1);
    }
    let ctx = &mut *(lparam.0 as *mut TransparencyApplyContext);

    use windows::Win32::UI::WindowsAndMessaging::IsWindow;
    if !IsWindow(Some(hwnd)).as_bool() {
        return BOOL(1);
    }

    let hwnd_key = hwnd.0 as isize;
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    let mut pid = 0u32;
    let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 || !ctx.target_pids.contains(&pid) {
        return BOOL(1);
    }
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
    if let Some(alpha) = ctx.alpha {
        if let Ok(cache) = transparency_cache().lock() {
            if cache.get(&hwnd_key).copied() == Some(alpha) {
                return BOOL(1);
            }
        }
        let mut next_style = ex_style;
        if (next_style & WS_EX_LAYERED.0 as i32) == 0 {
            next_style |= WS_EX_LAYERED.0 as i32;
            let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, next_style);
        }
        let mut current_color_key = COLORREF(0);
        let mut current_alpha: u8 = 0;
        let mut current_flags = windows::Win32::UI::WindowsAndMessaging::LAYERED_WINDOW_ATTRIBUTES_FLAGS(0);
        let has_current_alpha = GetLayeredWindowAttributes(
            hwnd,
            Some(&mut current_color_key),
            Some(&mut current_alpha),
            Some(&mut current_flags),
        )
        .is_ok()
            && (current_flags.0 & LWA_ALPHA.0) != 0;
        if !has_current_alpha || current_alpha != alpha {
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
        }
        if let Ok(mut cache) = transparency_cache().lock() {
            cache.insert(hwnd_key, alpha);
        }
    } else if (ex_style & WS_EX_LAYERED.0 as i32) != 0 {
        let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style & !(WS_EX_LAYERED.0 as i32));
        if let Ok(mut cache) = transparency_cache().lock() {
            cache.remove(&hwnd_key);
        }
    }
    ctx.applied = ctx.applied.saturating_add(1);
    BOOL(1)
}

#[cfg(target_os = "windows")]
pub(crate) fn process_name_matches(target: &str, name: &str) -> bool {
    let t = normalize_process_key(target);
    let n = normalize_process_key(name);
    !t.is_empty() && t == n
}

#[cfg(target_os = "windows")]
pub(crate) fn apply_process_transparency(app_process: &str, alpha: Option<u8>) -> Result<u32, String> {
    use sysinfo::{ProcessesToUpdate, System};
    let target = app_process.trim();
    if target.is_empty() {
        return Ok(0);
    }
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let target_pids: std::collections::HashSet<u32> = sys
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let name = process.name().to_string_lossy();
            if process_name_matches(target, &name) {
                Some(pid.as_u32())
            } else {
                None
            }
        })
        .collect();

    if target_pids.is_empty() {
        return Ok(0);
    }

    let mut ctx = TransparencyApplyContext {
        target_pids,
        alpha,
        applied: 0,
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_apply_transparency),
            LPARAM((&mut ctx as *mut TransparencyApplyContext) as isize),
        );
    }
    Ok(ctx.applied)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn apply_process_transparency(_app_process: &str, _alpha: Option<u8>) -> Result<u32, String> {
    Ok(0)
}

#[cfg(target_os = "windows")]
pub(crate) fn get_running_process_names() -> Vec<String> {
    use sysinfo::{ProcessesToUpdate, System};

    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let mut names = BTreeSet::new();
    for process in sys.processes().values() {
        let mut name = process.name().to_string_lossy().trim().to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        if !name.ends_with(".exe") {
            name.push_str(".exe");
        }
        names.insert(name);
    }
    names.into_iter().collect()
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn get_running_process_names() -> Vec<String> {
    Vec::new()
}
