use digicore_core::domain::ports::{CorpusBaselinePort, CorpusStoragePort};
use digicore_core::domain::value_objects::CorpusConfig;
use std::sync::Arc;

/// Service responsible for handling the "One-Click" Corpus Generation Utility workflow.
pub struct CorpusService {
    pub config: CorpusConfig,
    storage: Arc<dyn CorpusStoragePort>,
    baseline: Arc<dyn CorpusBaselinePort>,
}


impl CorpusService {
    pub fn new(
        config: CorpusConfig,
        storage: Arc<dyn CorpusStoragePort>,
        baseline: Arc<dyn CorpusBaselinePort>,
    ) -> Self {
        Self {
            config,
            storage,
            baseline,
        }
    }


    /// Try to capture the current clipboard image, save it, and generate a baseline.
    /// Returns true if an image was found and processed, false otherwise.
    pub async fn try_capture(&self, window_title: String) -> anyhow::Result<bool> {
        log::info!("[CorpusService] try_capture invoked. Enabled: {}", self.config.enabled);
        if !self.config.enabled {
            return Ok(false);
        }

        let mut cb = match arboard::Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                log::error!("[CorpusService] Failed to open clipboard: {}", e);
                return Ok(false);
            }
        };

        let img = match cb.get_image() {
            Ok(img) => img,
            Err(e) => {
                log::warn!("[CorpusService] No image found on clipboard: {}", e);
                self.show_toast_error("No Image on Clipboard", "Please copy an image first.");
                return Ok(false);
            }
        };

        use image::{ImageBuffer, Rgba};
        let buf: Option<ImageBuffer<Rgba<u8>, Vec<u8>>> = ImageBuffer::from_raw(
            img.width as u32,
            img.height as u32,
            img.bytes.into_owned(),
        );

        let mut png_data = std::io::Cursor::new(Vec::new());
        if let Some(b) = buf {
            // Arboard uses RGBA bytes natively. However, sometimes platforms differ.
            // Assuming standard RGBA formatting here for the test corpus.
            if let Err(e) = b.write_to(&mut png_data, image::ImageFormat::Png) {
                log::error!("[CorpusService] Failed to encode PNG: {}", e);
                return Ok(false);
            }
        } else {
            log::error!("[CorpusService] ImageBuffer::from_raw failed");
            return Ok(false);
        }
        let image_data = png_data.into_inner();

        // Sanitize window title
        let sanitized = window_title.chars().map(|c| {
            if c.is_alphanumeric() { c } else { '_' }
        }).collect::<String>();
        let re = regex::Regex::new(r"_+").unwrap();
        let sanitized_title = re.replace_all(&sanitized, "_").trim_matches('_').to_string();

        let prefix = format!("Example_xx_{}", sanitized_title);

        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let workspace_dir = std::path::PathBuf::from(manifest_dir).join("../../");
        let target_dir = workspace_dir.join(&self.config.output_dir);

        if !target_dir.exists() {
            let _ = std::fs::create_dir_all(&target_dir);
        }
        let target_dir_abs = target_dir.canonicalize().unwrap_or(target_dir);

        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow, GetWindowThreadProcessId};
            use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
            let fg_hwnd = GetForegroundWindow();
            let fg_thread = GetWindowThreadProcessId(fg_hwnd, None);
            let my_thread = GetCurrentThreadId();
            if fg_thread != 0 && fg_thread != my_thread {
                let _ = AttachThreadInput(fg_thread, my_thread, true);
                let _ = SetForegroundWindow(fg_hwnd);
                let _ = AttachThreadInput(fg_thread, my_thread, false);
            }
        }

        let chosen_path = match rfd::FileDialog::new()
            .set_directory(&target_dir_abs)
            .set_file_name(&format!("{}.png", prefix))
            .add_filter("PNG Image", &["png"])
            .save_file()
        {
            Some(p) => p,
            None => {
                log::info!("[CorpusService] User cancelled save dialog");
                return Ok(false);
            }
        };

        let final_prefix = chosen_path.file_stem().unwrap_or_default().to_string_lossy().to_string();

        // 1. Save Image
        log::info!("[CorpusService] Saving image to baseline...");
        let saved_path = match self.storage.save_image(&image_data, &final_prefix, "png") {
            Ok(p) => p,
            Err(e) => {
                log::error!("[CorpusService] save_image failed: {}", e);
                return Err(e.into());
            }
        };

        // 2. Generate Baseline
        log::info!("[CorpusService] Generating baseline OCR...");
        let _extracted_text = match self.baseline.generate_baseline(&saved_path, &final_prefix).await {
            Ok(text) => text,
            Err(e) => {
                log::error!("[CorpusService] generate_baseline failed: {}", e);
                return Err(e.into());
            }
        };

        // 3. Notify User
        self.show_toast(&final_prefix);

        log::info!("[CorpusService] Corpus generation successful!");
        Ok(true)
    }

    fn show_toast(&self, filename: &str) {
        #[cfg(target_os = "windows")]
        {
            use winrt_toast_reborn::{Toast, ToastManager};
            let manager = ToastManager::new("DigiCore.TextExpander");
            let mut toast = Toast::new();
            toast.text1("Corpus Added");
            toast.text2(&format!("Saved image and generated baseline for: {}", filename));
            let _ = manager.show(&toast);
        }
    }

    fn show_toast_error(&self, title: &str, msg: &str) {
        #[cfg(target_os = "windows")]
        {
            use winrt_toast_reborn::{Toast, ToastManager};
            let manager = ToastManager::new("DigiCore.TextExpander");
            let mut toast = Toast::new();
            toast.text1(title);
            toast.text2(msg);
            let _ = manager.show(&toast);
        }
    }
}
