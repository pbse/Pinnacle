// Necessary imports
use crate::pdf::utils::manual_deep_copy;
use lopdf::{dictionary, Document, Object};
use std::fs;
use std::path::Path;

#[tauri::command]
pub fn merge_pdfs(paths: Vec<&str>, output_path: &str) -> Result<(), String> {
    // --- Input Validation & Dir Creation (as before) ---
    if paths.is_empty() {
        return Err("No PDF files provided for merging.".to_string());
    }
    if paths.len() == 1 {
        let source_path = paths[0];
        let p = Path::new(source_path);
        if !p.exists() {
            return Err(format!("Input file not found: {}", source_path));
        }
        if !p.is_file() {
            return Err(format!("Input path is not a file: {}", source_path));
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
        fs::copy(source_path, output_path)
            .map(|_| ())
            .map_err(|e| {
                format!(
                    "Failed to copy single PDF from '{}' to '{}': {}",
                    source_path, output_path, e
                )
            })?;
        return Ok(());
    }
    for path in &paths {
        let p = Path::new(path);
        if !p.exists() {
            return Err(format!("Input file not found: {}", path));
        }
        if !p.is_file() {
            return Err(format!("Input path is not a file: {}", path));
        }
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

    // --- Build a fresh document using deep copy for every page ---
    let first_path = paths[0];
    let first_doc = Document::load(first_path)
        .map_err(|e| format!("Failed to load base PDF '{}': {}", first_path, e))?;

    let mut target_doc = Document::with_version(first_doc.version.clone());
    let target_pages_id = target_doc.new_object_id();
    let target_catalog_id = target_doc.new_object_id();
    let mut kids = Vec::new();

    for path in paths {
        let src_doc = Document::load(path)
            .map_err(|e| format!("Failed to load source PDF '{}': {}", path, e))?;

        let page_ids: Vec<_> = src_doc.get_pages().values().cloned().collect();
        if page_ids.is_empty() {
            continue;
        }

        let id_map = manual_deep_copy(&src_doc, &mut target_doc, &page_ids)
            .map_err(|e| format!("Failed to copy pages from '{}': {}", path, e))?;

        for old_page_id in page_ids {
            let new_page_id = *id_map.get(&old_page_id).ok_or_else(|| {
                format!(
                    "Internal error: mapped page id for {:?} from '{}' missing",
                    old_page_id, path
                )
            })?;

            {
                let page_obj = target_doc
                    .get_object_mut(new_page_id)
                    .map_err(|e| format!("Failed to fetch copied page {:?}: {}", new_page_id, e))?;
                let page_dict = page_obj
                    .as_dict_mut()
                    .map_err(|_| format!("Copied page {:?} is not a dictionary", new_page_id))?;
                page_dict.set("Parent", Object::Reference(target_pages_id));
            }

            kids.push(Object::Reference(new_page_id));
        }
    }

    target_doc.objects.insert(
        target_pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(kids.clone()),
            "Count" => Object::Integer(kids.len() as i64),
        }),
    );
    target_doc.objects.insert(
        target_catalog_id,
        Object::Dictionary(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(target_pages_id),
        }),
    );
    target_doc
        .trailer
        .set("Root", Object::Reference(target_catalog_id));

    target_doc.compress();
    target_doc
        .save(output_path)
        .map_err(|e| format!("Failed to save merged PDF to '{}': {}", output_path, e))?;

    Ok(())
}

// --- Tests ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_utils::create_minimal_pdf;
    use lopdf::Document;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf}; // Use PathBuf
    use std::sync::atomic::{AtomicUsize, Ordering}; // For unique IDs

    // --- Use unique directory names ---
    // Base names
    const BASE_TEST_DIR: &str = "target/test_data_merge"; // Put test artifacts in target/
    const BASE_OUTPUT_DIR: &str = "target/test_output_merge";

    // Counter for unique test run IDs
    static TEST_RUN_ID: AtomicUsize = AtomicUsize::new(0);

    // Helper to get unique paths for a test run
    fn get_unique_paths(test_name: &str) -> (PathBuf, PathBuf) {
        let run_id = TEST_RUN_ID.fetch_add(1, Ordering::SeqCst);
        let unique_suffix = format!("{}_{}", test_name, run_id);

        let test_dir = PathBuf::from(BASE_TEST_DIR).join(&unique_suffix);
        let output_dir = PathBuf::from(BASE_OUTPUT_DIR).join(&unique_suffix);

        // Cleanup previous runs *of this specific test* (optional but good practice)
        if test_dir.exists() {
            fs::remove_dir_all(&test_dir).ok();
        }
        if output_dir.exists() {
            fs::remove_dir_all(&output_dir).ok();
        }

        // Create fresh dirs for this run
        fs::create_dir_all(&test_dir).expect("Failed to create unique test data directory");
        fs::create_dir_all(&output_dir).expect("Failed to create unique test output directory");

        (test_dir, output_dir)
    }

    // Teardown helper (optional, target/ is usually cleaned by `cargo clean`)
    fn teardown_unique_paths(test_dir: &Path, output_dir: &Path) {
        fs::remove_dir_all(test_dir).ok();
        fs::remove_dir_all(output_dir).ok();
    }

    #[test]
    fn test_merge_two_pdfs_success() {
        let (test_dir, output_dir) = get_unique_paths("merge_two_pdfs"); // Get unique dirs

        let path1 = test_dir.join("doc1_2pages.pdf"); // Use unique test_dir
        let path2 = test_dir.join("doc2_1page.pdf"); // Use unique test_dir
        let output_path = output_dir.join("merged_2_1.pdf"); // Use unique output_dir

        create_minimal_pdf(path1.to_str().unwrap(), 2, "Doc1").expect("Create doc1");
        // Add a check to be absolutely sure
        assert!(path1.exists(), "doc1 should exist after creation");

        create_minimal_pdf(path2.to_str().unwrap(), 1, "Doc2").expect("Create doc2");
        assert!(path2.exists(), "doc2 should exist after creation");

        let paths_vec = vec![path1.to_str().unwrap(), path2.to_str().unwrap()];
        let result = merge_pdfs(paths_vec, output_path.to_str().unwrap());

        // Assertions remain the same
        assert!(result.is_ok(), "merge_pdfs failed: {:?}", result.err());
        assert!(output_path.exists(), "Output file was not created");
        match Document::load(output_path.to_str().unwrap()) {
            Ok(merged_doc) => {
                assert_eq!(
                    merged_doc.get_pages().len(),
                    3,
                    "Merged PDF should have 3 pages"
                );
                let catalog = merged_doc.catalog().expect("Merged catalog error");
                let pages_ref = catalog
                    .get(b"Pages")
                    .expect("Pages entry")
                    .as_reference()
                    .expect("Pages ref");
                let pages_dict = merged_doc
                    .get_dictionary(pages_ref)
                    .expect("Merged pages dict");
                let count = pages_dict
                    .get(b"Count")
                    .ok()
                    .and_then(|o| o.as_i64().ok())
                    .expect("Count");
                assert_eq!(count, 3, "Pages Count field mismatch");
            }
            Err(e) => panic!("Failed to load merged PDF: {}", e),
        }

        // Optional cleanup
        teardown_unique_paths(&test_dir, &output_dir);
    }

    #[test]
    fn test_merge_three_pdfs_success() {
        let (test_dir, output_dir) = get_unique_paths("merge_three_pdfs");

        let path1 = test_dir.join("d1.pdf");
        let path2 = test_dir.join("d2.pdf");
        let path3 = test_dir.join("d3.pdf");
        let output_path = output_dir.join("merged_1_2_3.pdf");

        create_minimal_pdf(path1.to_str().unwrap(), 1, "D1").expect("Create d1");
        create_minimal_pdf(path2.to_str().unwrap(), 2, "D2").expect("Create d2");
        create_minimal_pdf(path3.to_str().unwrap(), 3, "D3").expect("Create d3");
        assert!(path1.exists());
        assert!(path2.exists());
        assert!(path3.exists());

        let paths_vec = vec![
            path1.to_str().unwrap(),
            path2.to_str().unwrap(),
            path3.to_str().unwrap(),
        ];
        let result = merge_pdfs(paths_vec, output_path.to_str().unwrap());

        assert!(result.is_ok(), "merge_pdfs failed: {:?}", result.err());
        assert!(output_path.exists(), "Output file was not created");
        match Document::load(output_path.to_str().unwrap()) {
            Ok(merged_doc) => {
                assert_eq!(merged_doc.get_pages().len(), 6);
                // ... (verify count as before) ...
            }
            Err(e) => panic!("Failed to load merged PDF: {}", e),
        }
        teardown_unique_paths(&test_dir, &output_dir);
    }

    #[test]
    fn test_merge_input_not_a_pdf() {
        let (test_dir, output_dir) = get_unique_paths("test_merge_input_not_a_pdf");
        let path1 = test_dir.join("real.pdf");
        let not_pdf_path = test_dir.join("fake.txt");
        let output_path = output_dir.join("merged_fake_input.pdf");
        create_minimal_pdf(path1.to_str().unwrap(), 1, "Real").expect("Create real doc");
        // Corrected line:
        let mut file = fs::File::create(&not_pdf_path).expect("Failed to create dummy text file");
        writeln!(file, "This is text, not PDF.").expect("Failed to write to text file");
        let paths_vec = vec![path1.to_str().unwrap(), not_pdf_path.to_str().unwrap()];
        let result = merge_pdfs(paths_vec, output_path.to_str().unwrap());
        assert!(result.is_err());
        let err_msg = result.err().unwrap();
        assert!(err_msg.contains("Failed to load source PDF"));
        assert!(err_msg.contains(not_pdf_path.to_str().unwrap()));
        assert!(!output_path.exists());
        teardown_unique_paths(&test_dir, &output_dir);
    }

    #[test]
    fn test_merge_output_dir_not_found() {
        let (test_dir, output_dir) = get_unique_paths("test_merge_output_dir_not_found");
        fs::remove_dir_all(&output_dir).ok();
        let bad_output_dir = output_dir.join("subdir");
        let output_path = bad_output_dir.join("merged_bad_output.pdf");

        let path1 = test_dir.join("out_test1.pdf");
        let path2 = test_dir.join("out_test2.pdf");
        create_minimal_pdf(path1.to_str().unwrap(), 1, "Out1").expect("Failed to create out1");
        create_minimal_pdf(path2.to_str().unwrap(), 1, "Out2").expect("Failed to create out2");

        let paths_vec = vec![path1.to_str().unwrap(), path2.to_str().unwrap()];
        let result = merge_pdfs(paths_vec, output_path.to_str().unwrap());

        assert!(
            result.is_ok(),
            "merge failed creating dir: {:?}",
            result.err()
        );
        assert!(output_path.exists());

        match Document::load(output_path.to_str().unwrap()) {
            Ok(merged_doc) => assert_eq!(merged_doc.get_pages().len(), 2),
            Err(e) => panic!("Failed to load PDF from created dir: {}", e),
        }
        teardown_unique_paths(&test_dir, &output_dir);
    }
}
