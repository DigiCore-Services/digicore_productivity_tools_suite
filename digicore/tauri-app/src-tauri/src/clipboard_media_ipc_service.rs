//! Bounded inbound service for clipboard rich-text, image gallery, and image file helpers.

use super::*;

pub(crate) async fn get_clipboard_rich_text(host: ApiImpl) -> Result<RichTextDto, String> {
    let (plain, html, rtf) = host.clipboard.get_rich_text().map_err(|e| e.to_string())?;
    Ok(RichTextDto { plain, html, rtf })
}

pub(crate) async fn get_image_gallery(
    _host: ApiImpl,
    search: Option<String>,
    page: u32,
    page_size: u32,
) -> Result<(Vec<ClipEntryDto>, u32), String> {
    let (rows, total) =
        clipboard_repository::list_image_entries(search.as_deref(), page, page_size)?;
    let dtos = rows
        .into_iter()
        .map(|r| ClipEntryDto {
            id: r.id,
            content: r.content,
            process_name: r.process_name,
            window_title: r.window_title,
            length: r.char_count,
            word_count: r.word_count,
            created_at: r.created_at_unix_ms.to_string(),
            entry_type: r.entry_type,
            mime_type: r.mime_type,
            image_path: r.image_path,
            thumb_path: r.thumb_path,
            image_width: r.image_width,
            image_height: r.image_height,
            image_bytes: r.image_bytes,
            parent_id: r.parent_id,
            metadata: r.metadata,
            file_list: r.file_list,
        })
        .collect();
    Ok((dtos, total))
}

pub(crate) async fn get_child_entries(_host: ApiImpl, parent_id: u32) -> Result<Vec<ClipEntryDto>, String> {
    let rows = clipboard_repository::get_child_entries(parent_id)?;
    Ok(rows
        .into_iter()
        .map(|r| ClipEntryDto {
            id: r.id,
            content: r.content,
            process_name: r.process_name,
            window_title: r.window_title,
            length: r.char_count,
            word_count: r.word_count,
            created_at: r.created_at_unix_ms.to_string(),
            entry_type: r.entry_type,
            mime_type: r.mime_type,
            image_path: r.image_path,
            thumb_path: r.thumb_path,
            image_width: r.image_width,
            image_height: r.image_height,
            image_bytes: r.image_bytes,
            parent_id: r.parent_id,
            metadata: r.metadata,
            file_list: r.file_list,
        })
        .collect())
}

pub(crate) async fn copy_to_clipboard(host: ApiImpl, text: String) -> Result<(), String> {
    host.clipboard.set_text(&text).map_err(|e| e.to_string())?;
    super::diag_log("info", "[Clipboard][copy] copied text to system clipboard via adapter");
    Ok(())
}

pub(crate) async fn copy_clipboard_image_by_id(_host: ApiImpl, id: u32) -> Result<(), String> {
    let row = clipboard_repository::get_entry_by_id(id)?
        .ok_or_else(|| format!("Clipboard entry id={} was not found.", id))?;
    if row.entry_type != "image" {
        return Err("Selected clipboard entry is not an image.".to_string());
    }
    let image_path = row
        .image_path
        .ok_or_else(|| "Image file path is missing.".to_string())?;
    let img = image::open(&image_path).map_err(|e| e.to_string())?.to_rgba8();
    let width = img.width() as usize;
    let height = img.height() as usize;
    let bytes = img.into_raw();
    arboard::Clipboard::new()
        .map_err(|e| e.to_string())?
        .set_image(arboard::ImageData {
            width,
            height,
            bytes: std::borrow::Cow::Owned(bytes),
        })
        .map_err(|e| e.to_string())?;
    super::diag_log("info", format!("[Clipboard][copy.image] copied image id={id}"));
    Ok(())
}

pub(crate) async fn save_clipboard_image_by_id(_host: ApiImpl, id: u32, path: String) -> Result<(), String> {
    let row = clipboard_repository::get_entry_by_id(id)?
        .ok_or_else(|| format!("Clipboard entry id={} was not found.", id))?;
    if row.entry_type != "image" {
        return Err("Selected clipboard entry is not an image.".to_string());
    }
    let src = row
        .image_path
        .ok_or_else(|| "Image file path is missing.".to_string())?;
    std::fs::copy(src, &path).map_err(|e| e.to_string())?;
    super::diag_log("info", format!("[Clipboard][save.image] saved image id={} to {}", id, path));
    Ok(())
}

pub(crate) async fn open_clipboard_image_by_id(_host: ApiImpl, id: u32) -> Result<(), String> {
    let row = clipboard_repository::get_entry_by_id(id)?
        .ok_or_else(|| format!("Clipboard entry id={} was not found.", id))?;
    if row.entry_type != "image" {
        return Err("Selected clipboard entry is not an image.".to_string());
    }
    let image_path = row
        .image_path
        .ok_or_else(|| "Image file path is missing.".to_string())?;
    open_file_in_default_app(&image_path)?;
    super::diag_log("info", format!("[Clipboard][open.image] opened image id={id}"));
    Ok(())
}

pub(crate) async fn get_clip_entry_by_id(_host: ApiImpl, id: u32) -> Result<Option<ClipEntryDto>, String> {
    let entry_opt = clipboard_repository::get_entry_by_id(id)?;
    let dto_opt = entry_opt.map(|r| ClipEntryDto {
        id: r.id,
        content: r.content,
        process_name: r.process_name,
        window_title: r.window_title,
        length: r.char_count,
        word_count: r.word_count,
        created_at: r.created_at_unix_ms.to_string(),
        entry_type: r.entry_type,
        mime_type: r.mime_type,
        image_path: r.image_path,
        thumb_path: r.thumb_path,
        image_width: r.image_width,
        image_height: r.image_height,
        image_bytes: r.image_bytes,
        parent_id: r.parent_id,
        metadata: r.metadata,
        file_list: r.file_list,
    });
    Ok(dto_opt)
}

