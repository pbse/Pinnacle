use lopdf::{Document, Object};
use std::fs;
use std::path::Path;

#[tauri::command]
pub fn sanitize_pdf(path: &str, output_path: &str) -> Result<(), String> {
    let input_path = Path::new(path);
    if !input_path.exists() || !input_path.is_file() {
        return Err(format!("Input file not found: {}", path));
    }
    if let Some(parent_dir) = Path::new(output_path).parent() {
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)
                .map_err(|e| format!("Failed to create output directory: {}", e))?;
        }
    }

    let mut doc = Document::load(path).map_err(|e| format!("Failed to load PDF: {}", e))?;

    // 1. Remove Info dictionary from Trailer
    doc.trailer.remove(b"Info");

    // 2. Remove Metadata from Catalog
    if let Ok(Object::Reference(root_id)) = doc.trailer.get(b"Root") {
        if let Ok(Object::Dictionary(catalog)) = doc.get_object(*root_id) {
            let mut new_catalog = catalog.clone();
            new_catalog.remove(b"Metadata");
            // Set the object, ignore if it fails
            let _ = doc.set_object(*root_id, Object::Dictionary(new_catalog));
        }
    }

    doc.save(output_path)
        .map_err(|e| format!("Failed to save sanitized PDF: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_utils::{create_minimal_pdf, setup_unique_paths, teardown_unique_paths};
    use lopdf::{Document, Object};

    #[test]
    fn test_sanitize_pdf_success() {
        let (test_dir, output_dir) = setup_unique_paths("sanitize");
        let input_path = test_dir.join("input.pdf");
        let output_path = output_dir.join("output.pdf");

        create_minimal_pdf(input_path.to_str().unwrap(), 1, "SanitizeTest").unwrap();

        // Add some metadata to be removed
        let mut doc = Document::load(input_path.to_str().unwrap()).unwrap();
        doc.trailer.set(
            "Info",
            Object::Dictionary(lopdf::dictionary! { "Title" => "Dirty PDF" }),
        );
        doc.save(input_path.to_str().unwrap()).unwrap();

        let result = sanitize_pdf(input_path.to_str().unwrap(), output_path.to_str().unwrap());

        assert!(result.is_ok());
        assert!(output_path.exists());

        // Verify metadata is gone
        let doc_after = Document::load(output_path.to_str().unwrap()).unwrap();
        assert!(doc_after.trailer.get(b"Info").is_err());

        teardown_unique_paths(&test_dir, &output_dir);
    }
}
