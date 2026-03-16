//! Build script for digicore-text-expander.
//!
//! When building on Windows, embeds a DPI-aware manifest for egui and Tauri binaries
//! to fix mouse hover/click alignment on high-DPI displays.

fn main() {
    #[cfg(windows)]
    embed_dpi_manifest();
}

#[cfg(windows)]
fn embed_dpi_manifest() {
    use embed_manifest::manifest::DpiAwareness;
    use embed_manifest::{embed_manifest, new_manifest};

    let manifest = new_manifest("DigiCore Text Expander")
        .dpi_awareness(DpiAwareness::PerMonitorV2);

    if let Err(e) = embed_manifest(manifest) {
        eprintln!("Warning: Could not embed DPI manifest: {}", e);
        eprintln!("Mouse hover alignment may be incorrect on high-DPI displays.");
    }
}
