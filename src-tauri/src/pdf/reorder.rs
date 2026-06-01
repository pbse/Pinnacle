use crate::pdf::utils::manual_deep_copy;
use lopdf::{dictionary, Document, Object};
use std::fs;
use std::path::Path;

#[tauri::command]
pub fn reorder_pages(path: &str, new_order: Vec<u32>, output_path: &str) -> Result<(), String> {
    if new_order.is_empty() {
        return Err("The new order cannot be empty.".to_string());
    }
    let input_path = Path::new(path);
    if !input_path.exists() || !input_path.is_file() {
        return Err(format!("Input file not found: {}", path));
    }

    let doc = Document::load(path).map_err(|e| format!("Failed to load PDF: {}", e))?;
    let mut new_doc = Document::with_version(doc.version.clone());
    let new_pages_id = new_doc.new_object_id();
    let new_catalog_id = new_doc.new_object_id();

    let source_pages_map = doc.get_pages();
    let mut page_ids_to_copy = Vec::with_capacity(new_order.len());

    for &page_num in &new_order {
        match source_pages_map.get(&page_num) {
            Some(&page_id) => page_ids_to_copy.push(page_id),
            None => return Err(format!("Page {} not found in document.", page_num)),
        }
    }

    if let Some(parent) = Path::new(output_path).parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).ok();
        }
    }

    let object_map = manual_deep_copy(&doc, &mut new_doc, &page_ids_to_copy)
        .map_err(|e| format!("Failed to copy objects: {}", e))?;

    let mut new_kids = Vec::with_capacity(new_order.len());
    for old_id in &page_ids_to_copy {
        let &new_id = object_map.get(old_id).unwrap();
        new_kids.push(Object::Reference(new_id));

        if let Ok(page_obj) = new_doc.get_object_mut(new_id) {
            if let Ok(page_dict) = page_obj.as_dict_mut() {
                page_dict.set("Parent", Object::Reference(new_pages_id));
            }
        }
    }

    new_doc.objects.insert(
        new_pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(new_kids),
            "Count" => Object::Integer(new_order.len() as i64),
        }),
    );
    new_doc.objects.insert(
        new_catalog_id,
        Object::Dictionary(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(new_pages_id),
        }),
    );
    new_doc
        .trailer
        .set("Root", Object::Reference(new_catalog_id));
    new_doc.compress();
    new_doc
        .save(output_path)
        .map_err(|e| format!("Failed to save: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_utils::{create_minimal_pdf, setup_unique_paths, teardown_unique_paths};

    #[test]
    fn test_reorder_pages_success() {
        let (test_dir, output_dir) = setup_unique_paths("reorder_success");
        let input_path = test_dir.join("input.pdf");
        create_minimal_pdf(input_path.to_str().unwrap(), 3, "Test Page").unwrap();

        let output_path = output_dir.join("output.pdf");
        // Reverse order
        let result = reorder_pages(
            input_path.to_str().unwrap(),
            vec![3, 2, 1],
            output_path.to_str().unwrap(),
        );

        assert!(result.is_ok());
        let output_doc = Document::load(output_path).unwrap();
        assert_eq!(output_doc.get_pages().len(), 3);
        teardown_unique_paths(&test_dir, &output_dir);
    }

    #[test]
    fn test_reorder_empty_order() {
        let result = reorder_pages("any.pdf", vec![], "out.pdf");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "The new order cannot be empty.");
    }
}
