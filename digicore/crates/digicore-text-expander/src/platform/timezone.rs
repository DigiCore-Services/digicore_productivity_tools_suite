//! Timezone helpers for {tz} and {timezone} placeholders.
//!
//! Windows: Registry HKLM\...\TimeZoneKeyName for full name; derive abbreviation.

pub fn get_timezone_full() -> String {
    #[cfg(target_os = "windows")]
    {
        return get_windows_timezone();
    }
    #[cfg(not(target_os = "windows"))]
    {
        chrono::Local::now().format("%Z").to_string()
    }
}

pub fn get_timezone_abbrev() -> String {
    #[cfg(target_os = "windows")]
    {
        let full = get_windows_timezone();
        timezone_full_to_abbrev(&full)
    }
    #[cfg(not(target_os = "windows"))]
    {
        chrono::Local::now().format("%Z").to_string()
    }
}

#[cfg(target_os = "windows")]
fn get_windows_timezone() -> String {
    use winreg::RegKey;

    let key_path = "SYSTEM\\CurrentControlSet\\Control\\TimeZoneInformation";
    if let Ok(hklm) = RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE).open_subkey(key_path) {
        if let Ok(val) = hklm.get_value::<String, _>("TimeZoneKeyName") {
            return val;
        }
    }
    chrono::Local::now().format("%:z").to_string()
}

/// Map Windows timezone full name to abbreviation (common mappings).
fn timezone_full_to_abbrev(full: &str) -> String {
    let mapping: &[(&str, &str)] = &[
        ("Pacific Standard Time", "PST"),
        ("Pacific Daylight Time", "PDT"),
        ("Mountain Standard Time", "MST"),
        ("Mountain Daylight Time", "MDT"),
        ("Central Standard Time", "CST"),
        ("Central Daylight Time", "CDT"),
        ("Eastern Standard Time", "EST"),
        ("Eastern Daylight Time", "EDT"),
        ("UTC", "UTC"),
        ("Coordinated Universal Time", "UTC"),
        ("GMT Standard Time", "GMT"),
        ("GMT Daylight Time", "BST"),
        ("British Summer Time", "BST"),
        ("W. Europe Standard Time", "CET"),
        ("W. Europe Daylight Time", "CEST"),
        ("Central European Standard Time", "CET"),
        ("Central European Summer Time", "CEST"),
    ];
    for (f, a) in mapping {
        if full.eq_ignore_ascii_case(f) {
            return (*a).to_string();
        }
    }
    // Fallback: take first letter of each word
    full.split_whitespace()
        .filter_map(|w| w.chars().next())
        .filter(|c| c.is_ascii_alphabetic())
        .collect::<String>()
        .to_uppercase()
        .chars()
        .take(3)
        .collect()
}
