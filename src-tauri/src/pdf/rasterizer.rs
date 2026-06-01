use image::ImageFormat;
use pdfium_render::prelude::*;
use std::fs;
use std::path::Path;

#[tauri::command]
pub fn pdf_to_images(path: &str, output_dir: &str, format: &str) -> Result<Vec<String>, String> {
    // Ensure output directory exists
    if !Path::new(output_dir).exists() {
        fs::create_dir_all(output_dir)
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    // Try to bind to pdfium. On many systems it might need to be downloaded/bundled.
    // For this implementation, we'll try system library first.
    let pdfium = Pdfium::new(Pdfium::bind_to_system_library().map_err(|e| {
        format!(
            "Failed to bind to Pdfium system library: {:?}. Please ensure Pdfium is installed.",
            e
        )
    })?);

    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("Failed to load PDF: {:?}", e))?;

    let render_config = PdfRenderConfig::new()
        .set_target_width(2000)
        .rotate_if_landscape(PdfPageRenderRotation::None, false);

    let mut output_files = Vec::new();
    let format_ext = format.to_lowercase();
    let format_enum = match format_ext.as_str() {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        _ => return Err("Unsupported format. Use 'png' or 'jpg'.".to_string()),
    };

    for (index, page) in document.pages().iter().enumerate() {
        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| format!("Failed to render page {}: {:?}", index, e))?;

        let file_name = format!("page_{}.{}", index + 1, format_ext);
        let output_path = Path::new(output_dir).join(file_name);
        let output_path_str = output_path
            .to_str()
            .ok_or("Invalid output path")?
            .to_string();

        bitmap
            .as_image()
            .save_with_format(&output_path_str, format_enum)
            .map_err(|e| format!("Failed to save image: {}", e))?;

        output_files.push(output_path_str);
    }

    Ok(output_files)
}

#[cfg(test)]
mod tests {
    // Note: Tests for rasterization might fail in CI if pdfium is not present.
    // We would typically mock this or skip if library is missing.
}
