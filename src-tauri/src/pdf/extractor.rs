use crate::pdf::utils::manual_deep_copy;
use lopdf::{dictionary, Document, Object};
use std::fs;
use std::path::Path;

// --- extract_pdf_page Function using Manual Deep Copy ---

#[tauri::command]
pub fn extract_pdf_page(path: &str, page_number: u32, output_path: &str) -> Result<(), String> {
    // --- Input Validation & Dir Creation ---
    if page_number == 0 {
        return Err("Page number must be 1-based (greater than 0).".to_string());
    }
    let input_path = Path::new(path);
    if !input_path.exists() {
        return Err(format!("Input file not found: {}", path));
    }
    if !input_path.is_file() {
        return Err(format!("Input path is not a file: {}", path));
    }
    if let Some(parent_dir) = Path::new(output_path).parent() {
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir).map_err(|e| {
                format!(
                    "Failed to create output directory '{}': {}",
                    parent_dir.display(),
                    e
                )
            })?;
        }
    }

    // --- Load Original Document ---
    // Map potential LopdfError to String for the command's return type
    let doc = Document::load(path).map_err(|e| format!("Failed to load PDF '{}': {}", path, e))?;

    // --- Find Target Page Object ID ---
    let source_pages_map = doc.get_pages();
    let target_page_id = *source_pages_map.get(&page_number).ok_or_else(|| {
        format!(
            "Page number {} not found in document '{}' (which has {} pages).",
            page_number,
            path,
            source_pages_map.len()
        )
    })?;

    // --- Prepare New Document ---
    let mut new_doc = Document::with_version(doc.version.clone());
    let new_pages_id = new_doc.new_object_id(); // Placeholder for the new Pages node ID
    let new_catalog_id = new_doc.new_object_id(); // Placeholder for the new Catalog node ID

    // --- Perform Manual Deep Copy ---
    // Map potential LopdfError from deep copy to String
    let object_map = manual_deep_copy(&doc, &mut new_doc, &[target_page_id]).map_err(|e| {
        format!(
            "Failed to deep copy page {} from '{}': {}",
            page_number, path, e
        )
    })?;

    // --- Find New Page ID ---
    // If the target page wasn't mapped (shouldn't happen if deep copy succeeded), return error
    let new_page_id = *object_map.get(&target_page_id).ok_or_else(|| {
        format!(
            "Internal error: Copied page ObjectId {:?} not found in mapping.",
            target_page_id
        )
    })?;

    // --- Update Parent Pointer in Copied Page ---
    // Use a block to limit the mutable borrow of new_doc
    {
        let page_obj = new_doc.get_object_mut(new_page_id).map_err(|e| {
            format!(
                "Failed to retrieve copied page object {:?} to update Parent: {}",
                new_page_id, e
            )
        })?;

        // lopdf 0.34.0 as_dict_mut() returns Result
        let page_dict = page_obj.as_dict_mut().map_err(|_| {
            format!(
                "Internal error: Copied page object {:?} is not a dictionary.",
                new_page_id
            )
        })?; // Map error

        page_dict.set("Parent", Object::Reference(new_pages_id));

        // Note: We rely on the second pass of manual_deep_copy (update_references_recursive)
        // to have correctly updated the /Resources reference (if it was a reference)
        // or the references *within* the /Resources dictionary (if it was inline).
        // Explicitly checking/setting Resources here proved complex and potentially redundant
        // if the deep copy worked correctly. We keep the previous useful log messages though.
        match doc
            .get_object(target_page_id)
            .ok()
            .and_then(|o| o.as_dict().ok())
            .and_then(|d| d.get(b"Resources").ok())
        {
            Some(Object::Dictionary(_)) => println!(
                "Note: Page {} uses an inline /Resources dictionary.",
                page_number
            ),
            Some(Object::Reference(_)) => (), // Normal case, handled by deep copy
            _ => println!(
                "Warning: Page {} is missing /Resources or it's not a Reference/Dictionary.",
                page_number
            ),
        }
    } // Mutable borrow of new_doc ends here

    // --- Build New Document Structure ---
    new_doc.objects.insert(
        new_pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![Object::Reference(new_page_id)]), // Array with single page ref
            "Count" => Object::Integer(1), // Only one page
        }),
    );
    new_doc.objects.insert(
        new_catalog_id,
        Object::Dictionary(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(new_pages_id), // Reference the new Pages node
        }),
    );
    new_doc
        .trailer
        .set("Root", Object::Reference(new_catalog_id));

    // --- Compress and Save ---
    new_doc.compress();
    new_doc.save(output_path).map_err(|e| {
        format!(
            "Failed to save extracted page PDF to '{}': {}",
            output_path, e
        )
    })?;

    Ok(())
}

// --- Tests ---
#[cfg(test)]
mod tests {
    use super::*; // Imports extract_pdf_page and helpers if they are in the parent mod
    use crate::pdf::test_utils::create_minimal_pdf;
    use lopdf::Document;
    use std::fs;
    use std::io::Write; // For creating non-pdf file test
    use std::path::PathBuf;

    // --- RAII Guard for Test Environment ---
    struct TestEnvironment {
        test_dir: PathBuf,
        output_dir: PathBuf,
        // Store the primary input path created by setup
        input_pdf_path: PathBuf,
    }

    impl TestEnvironment {
        fn new(test_name: &str) -> Self {
            use crate::pdf::test_utils::setup_unique_paths;
            let (test_dir, output_dir) = setup_unique_paths(test_name);

            // Define and create the primary input PDF
            let input_pdf_path = test_dir.join("sample.pdf");
            create_minimal_pdf(input_pdf_path.to_str().unwrap(), 3, "Sample")
                .expect("Setup: Failed to create dummy sample PDF");
            assert!(
                input_pdf_path.exists(),
                "Setup: Dummy PDF does not exist after creation!"
            );

            TestEnvironment {
                test_dir,
                output_dir,
                input_pdf_path,
            }
        }

        // Helper to get the full path for an output file
        fn output_path(&self, filename: &str) -> PathBuf {
            self.output_dir.join(filename)
        }

        // Helper to get the primary input path as str
        fn input_path_str(&self) -> &str {
            self.input_pdf_path.to_str().unwrap()
        }
    }

    // Implement Drop for automatic cleanup
    impl Drop for TestEnvironment {
        fn drop(&mut self) {
            use crate::pdf::test_utils::teardown_unique_paths;
            teardown_unique_paths(&self.test_dir, &self.output_dir);
        }
    }

    // --- Updated Tests ---
    #[test]
    fn test_extract_pdf_page_success() {
        let env = TestEnvironment::new("extract_success");
        let output_path = env.output_path("page_1.pdf");
        let page_to_extract = 1;

        let result = extract_pdf_page(
            env.input_path_str(),
            page_to_extract,
            output_path.to_str().unwrap(),
        );

        assert!(
            result.is_ok(),
            "extract_pdf_page failed: {:?}",
            result.err()
        );
        assert!(
            output_path.exists(),
            "Output file was not created at {}",
            output_path.display()
        );

        // This check should now reflect a non-empty, valid single-page PDF
        match Document::load(&output_path) {
            Ok(output_doc) => {
                assert_eq!(
                    output_doc.get_pages().len(),
                    1,
                    "Output PDF does not have exactly one page."
                );
                // Ideally, add a content check here if possible, but length=1 is a good start.
            }
            Err(e) => panic!(
                "Failed to load the generated output PDF '{}': {}",
                output_path.display(),
                e
            ),
        }
    }

    #[test]
    fn test_extract_pdf_page_not_found() {
        let env = TestEnvironment::new("extract_not_found");
        let output_path = env.output_path("page_5.pdf");
        let page_to_extract = 5;

        let result = extract_pdf_page(
            env.input_path_str(),
            page_to_extract,
            output_path.to_str().unwrap(),
        );

        assert!(result.is_err());
        if let Err(e) = result {
            println!("Expected error: {}", e);
            // extract_pages returns PageNumberNotFound
            assert!(
                e.contains(&format!("Page number {} not found", page_to_extract)),
                "Error message mismatch"
            );
        }
        assert!(!output_path.exists() || Document::load(&output_path).is_err());
    }

    #[test]
    fn test_extract_page_zero() {
        let env = TestEnvironment::new("extract_page_zero");
        let output_path = env.output_path("page_0.pdf");
        let page_to_extract = 0;

        let result = extract_pdf_page(
            env.input_path_str(),
            page_to_extract,
            output_path.to_str().unwrap(),
        );

        assert!(result.is_err());
        if let Err(e) = result {
            println!("Expected error: {}", e);
            assert!(
                e.contains("Page number must be 1-based"),
                "Error message mismatch"
            );
        }
        // Don't check output file existence here, the validation fails early
        assert!(!output_path.exists());
    }

    #[test]
    fn test_extract_pdf_input_not_found() {
        use crate::pdf::test_utils::{setup_unique_paths, teardown_unique_paths};
        let (test_dir, output_dir) = setup_unique_paths("extract_not_found_input");

        let bad_input_path = test_dir.join("no_way_this_exists.pdf");
        let output_path = output_dir.join("output_for_bad_input.pdf");

        let page_to_extract = 1;
        let result = extract_pdf_page(
            bad_input_path.to_str().unwrap(),
            page_to_extract,
            output_path.to_str().unwrap(),
        );

        assert!(result.is_err());
        if let Err(e) = result {
            println!("Expected error: {}", e);
            assert!(e.contains("Input file not found"), "Error message mismatch");
        }

        teardown_unique_paths(&test_dir, &output_dir);
    }

    #[test]
    fn test_extract_pdf_input_not_a_pdf() {
        let env = TestEnvironment::new("extract_not_a_pdf");
        let not_pdf_path = env.test_dir.join("not_a_pdf.txt");
        let output_path = env.output_path("output_for_not_pdf.pdf");

        let mut file = fs::File::create(&not_pdf_path).expect("Failed to create dummy text file");
        writeln!(file, "This is text.").expect("Failed to write to text file");
        assert!(not_pdf_path.exists());

        let page_to_extract = 1;
        let result = extract_pdf_page(
            not_pdf_path.to_str().unwrap(),
            page_to_extract,
            output_path.to_str().unwrap(),
        );

        assert!(result.is_err());
        if let Err(e) = result {
            println!("Expected error: {}", e);
            assert!(
                e.contains("Failed to load PDF")
                    || e.contains("invalid PDF header")
                    || e.contains("cannot find trailer"),
                "Error message mismatch"
            );
        }
        assert!(!output_path.exists() || Document::load(&output_path).is_err());
    }
}
