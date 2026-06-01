use liteparse::{LiteParse, LiteParseConfig, OutputFormat};
use std::fs;
use std::path::Path;
use tauri::{AppHandle, Manager};

pub async fn parse_pdf_layout_internal(
    app_handle: Option<&AppHandle>,
    path: &str,
) -> Result<String, String> {
    let tessdata_path = if let Some(app) = app_handle {
        app.path()
            .resource_dir()
            .map(|p: std::path::PathBuf| p.join("resources").join("tessdata"))
            .map_err(|e| format!("Failed to resolve resource directory: {}", e))?
    } else {
        // Fallback for tests running inside the src-tauri folder
        let local_path = Path::new("resources").join("tessdata");
        if local_path.exists() {
            local_path
        } else {
            // Fallback for tests if run from workspace root
            Path::new("src-tauri").join("resources").join("tessdata")
        }
    };

    if !tessdata_path.exists() {
        return Err(format!(
            "Tessdata directory not found at: {}",
            tessdata_path.display()
        ));
    }

    let config = LiteParseConfig {
        ocr_language: "eng".to_string(),
        ocr_enabled: true,
        ocr_server_url: None,
        tessdata_path: Some(tessdata_path.to_string_lossy().into_owned()),
        max_pages: 1000,
        target_pages: None,
        dpi: 150.0,
        output_format: OutputFormat::Text,
        preserve_very_small_text: false,
        password: None,
        quiet: true,
        num_workers: 1,
    };

    let parser = LiteParse::new(config);
    let result = parser
        .parse(path)
        .await
        .map_err(|e| format!("LiteParse extraction failed: {:?}", e))?;

    Ok(result.text)
}

#[tauri::command]
pub async fn pdf_to_layout_json(app_handle: AppHandle, path: String) -> Result<String, String> {
    let tessdata_path = app_handle
        .path()
        .resource_dir()
        .map(|p: std::path::PathBuf| p.join("resources").join("tessdata"))
        .map_err(|e| format!("Failed to resolve resource directory: {}", e))?;

    if !tessdata_path.exists() {
        return Err(format!(
            "Tessdata directory not found at: {}",
            tessdata_path.display()
        ));
    }

    let config = LiteParseConfig {
        ocr_language: "eng".to_string(),
        ocr_enabled: true,
        ocr_server_url: None,
        tessdata_path: Some(tessdata_path.to_string_lossy().into_owned()),
        max_pages: 1000,
        target_pages: None,
        dpi: 150.0,
        output_format: OutputFormat::Json,
        preserve_very_small_text: false,
        password: None,
        quiet: true,
        num_workers: 1,
    };

    let parser = LiteParse::new(config);
    let result = parser
        .parse(&path)
        .await
        .map_err(|e| format!("LiteParse extraction failed: {:?}", e))?;

    let json_str = liteparse::output::json::format_json(&result.pages)
        .map_err(|e| format!("Failed to format JSON: {}", e))?;

    Ok(json_str)
}

#[tauri::command]
pub async fn pdf_to_text(
    app_handle: AppHandle,
    path: String,
    output_path: String,
) -> Result<(), String> {
    let input_path = Path::new(&path);
    if !input_path.exists() || !input_path.is_file() {
        return Err(format!("Input file not found: {}", path));
    }
    if let Some(parent_dir) = Path::new(&output_path).parent() {
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)
                .map_err(|e| format!("Failed to create output directory: {}", e))?;
        }
    }

    let out_text = parse_pdf_layout_internal(Some(&app_handle), &path).await?;
    fs::write(output_path, out_text).map_err(|e| format!("Failed to write to file: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn pdf_to_text_string(app_handle: AppHandle, path: String) -> Result<String, String> {
    let input_path = Path::new(&path);
    if !input_path.exists() || !input_path.is_file() {
        return Err(format!("Input file not found: {}", path));
    }

    parse_pdf_layout_internal(Some(&app_handle), &path).await
}

#[tauri::command]
pub fn write_text_file(path: &str, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|e| format!("Failed to write text to file: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_utils::{create_minimal_pdf, setup_unique_paths, teardown_unique_paths};

    #[tokio::test]
    async fn test_pdf_to_layout_json_structure() {
        let (test_dir, output_dir) = setup_unique_paths("layout_json");
        let path = test_dir.join("sample.pdf");
        create_minimal_pdf(path.to_str().unwrap(), 1, "Layout").unwrap();

        let tessdata_path = std::path::Path::new("resources").join("tessdata");
        let tessdata_path = if tessdata_path.exists() {
            tessdata_path
        } else {
            std::path::Path::new("src-tauri")
                .join("resources")
                .join("tessdata")
        };

        let config = LiteParseConfig {
            ocr_language: "eng".to_string(),
            ocr_enabled: true,
            ocr_server_url: None,
            tessdata_path: if tessdata_path.exists() {
                Some(tessdata_path.to_string_lossy().into_owned())
            } else {
                None
            },
            max_pages: 10,
            target_pages: None,
            dpi: 150.0,
            output_format: OutputFormat::Json,
            preserve_very_small_text: false,
            password: None,
            quiet: true,
            num_workers: 1,
        };

        let parser = LiteParse::new(config);
        let result = parser.parse(path.to_str().unwrap()).await.unwrap();
        let json_str = liteparse::output::json::format_json(&result.pages).unwrap();
        println!("LAYOUT JSON RESULT: {}", json_str);
        assert!(json_str.contains("\"pages\""));
        assert!(json_str.contains("\"text_items\""));

        teardown_unique_paths(&test_dir, &output_dir);
    }
}
