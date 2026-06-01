use lopdf::{Dictionary, Document, Object};
use std::collections::BTreeMap;
use std::path::Path;

fn decode_pdf_string(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        // UTF-16BE with BOM
        let utf16_data: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16(&utf16_data).unwrap_or_else(|_| decode_pdfdoc_encoding(bytes))
    } else {
        decode_pdfdoc_encoding(bytes)
    }
}

fn decode_pdfdoc_encoding(bytes: &[u8]) -> String {
    bytes.iter().map(|&byte| byte as char).collect()
}

#[tauri::command]
pub fn parse_pdf(path: &str) -> Result<BTreeMap<String, String>, String> {
    // --- Input Validation (as before) ---
    let input_path = Path::new(path);
    if !input_path.exists() {
        return Err(format!("Input file not found: {}", path));
    }
    if !input_path.is_file() {
        return Err(format!("Input path is not a file: {}", path));
    }

    // --- Load Document (as before) ---
    let doc = Document::load(path)
        .map_err(|e| format!("Failed to load or parse PDF '{}': {}", path, e))?;

    // --- Get Info Dictionary (explicit logic as before) ---
    let info_dict_result: Result<Option<&Dictionary>, String> = match doc.trailer.get(b"Info") {
        Ok(info_obj) => {
            match info_obj.as_reference() {
                Ok(info_ref) => {
                    match doc.get_object(info_ref) {
                        Ok(Object::Dictionary(dict)) => Ok(Some(dict)),
                        Ok(_) => Ok(None), // Ref points to wrong type
                        Err(e) => {
                            eprintln!(
                                "Warning: Failed to resolve Info reference {:?}: {}",
                                info_ref, e
                            );
                            Ok(None) // Treat invalid ref as None
                        }
                    }
                }
                Err(_) => Ok(None), // Info object is not a Reference
            }
        }
        Err(_) => Ok(None), // Info key not found
    };
    let maybe_info_dict = info_dict_result?;

    // --- Process Dictionary if Found ---
    let metadata = match maybe_info_dict {
        Some(dict) => dict
            .iter()
            .filter_map(|(key_bytes, value_obj)| {
                if let Object::String(ref value_bytes, _format) = value_obj {
                    let decoded_value = decode_pdf_string(value_bytes);
                    let key_str = String::from_utf8_lossy(key_bytes).to_string();
                    Some((key_str, decoded_value))
                } else {
                    None // Skip non-string values
                }
            })
            .collect(),
        None => BTreeMap::new(),
    };

    Ok(metadata)
}

// --- Tests ---
#[cfg(test)]
mod tests {
    // Import the function being tested
    use super::parse_pdf;

    // Imports needed for testing
    use lopdf::{dictionary, Dictionary, Document, Object}; // Added Dictionary
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    // --- RAII Guard for Test Environment ---
    struct TestEnvironment {
        test_dir: PathBuf,
        // Output dir might not be strictly needed for parser, but keep pattern consistent
        output_dir: PathBuf,
    }

    impl TestEnvironment {
        /// Creates unique directories for a test run.
        fn new(test_name: &str) -> Self {
            use crate::pdf::test_utils::setup_unique_paths;
            let (test_dir, output_dir) = setup_unique_paths(test_name);

            TestEnvironment {
                test_dir,
                output_dir,
            }
        }

        /// Gets the path to the unique test data directory.
        fn test_dir(&self) -> &Path {
            &self.test_dir
        }
    }

    // Implement Drop for automatic cleanup
    impl Drop for TestEnvironment {
        fn drop(&mut self) {
            use crate::pdf::test_utils::teardown_unique_paths;
            teardown_unique_paths(&self.test_dir, &self.output_dir);
        }
    }

    // --- PDF Creation Helper ---
    fn create_test_pdf(
        file_path: &str,
        info_data: Option<BTreeMap<&str, &str>>,
        make_info_invalid: Option<&str>,
    ) -> std::io::Result<()> {
        let mut doc = Document::with_version("1.5");

        // Minimal valid structure (Catalog + empty Pages)
        let pages_id = doc.add_object(Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => 0_i64,
            "Kids" => Object::Array(vec![]),
        }));
        let catalog_id = doc.add_object(Object::Dictionary(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages_id),
        }));
        doc.trailer.set("Root", Object::Reference(catalog_id));

        // Add Info dictionary if requested
        if let Some(data) = info_data {
            let mut info_dict = Dictionary::new();
            for (key, value) in data {
                // --- FIX: Explicitly encode as UTF-16BE with BOM ---
                let mut bytes: Vec<u8> = vec![0xFE, 0xFF]; // UTF-16BE BOM
                for c in value.encode_utf16() {
                    // Encode Rust string to UTF-16
                    bytes.extend_from_slice(&c.to_be_bytes()); // Append big-endian bytes
                }
                // Create the PDF String object with these bytes and Literal format
                // lopdf::save should handle escaping non-printable chars if needed for Literal.
                info_dict.set(
                    key.as_bytes(),
                    Object::String(bytes, lopdf::StringFormat::Literal),
                );
                // --- End FIX ---
            }
            info_dict.set(b"NumericValue", Object::Integer(123)); // Keep non-string test

            match make_info_invalid {
                Some("not_a_ref") => {
                    doc.trailer.set("Info", Object::Dictionary(info_dict));
                }
                Some("bad_ref_type") => {
                    let dummy_array_id = doc.add_object(Object::Array(vec![]));
                    doc.trailer.set("Info", Object::Reference(dummy_array_id));
                }
                _ => {
                    let info_id = doc.add_object(Object::Dictionary(info_dict)); // Wrap Dictionary
                    doc.trailer.set("Info", Object::Reference(info_id));
                }
            }
        }

        doc.save(file_path)?;
        Ok(())
    }

    // --- Updated Tests ---

    #[test]
    fn test_parse_pdf_with_metadata() {
        let env = TestEnvironment::new("parse_with_meta");
        let file_path = env.test_dir().join("meta_test.pdf");
        let mut info = BTreeMap::new();
        info.insert("Title", "My Test Document");
        info.insert("Author", "Test Author");
        info.insert("Subject", "Testing\u{2013}Metadata"); // En dash

        create_test_pdf(file_path.to_str().unwrap(), Some(info), None).expect("Create");
        assert!(file_path.exists());

        let result = parse_pdf(file_path.to_str().unwrap());
        assert!(result.is_ok(), "parse_pdf failed: {:?}", result.err());
        let metadata = result.unwrap();

        assert_eq!(metadata.len(), 3, "Count");
        assert_eq!(metadata.get("Title"), Some(&"My Test Document".to_string()));
        assert_eq!(metadata.get("Author"), Some(&"Test Author".to_string()));
        // THIS ASSERTION SHOULD NOW PASS
        assert_eq!(
            metadata.get("Subject"),
            Some(&"Testing\u{2013}Metadata".to_string())
        );
        assert!(metadata.get("NumericValue").is_none());
    }

    #[test]
    fn test_parse_pdf_no_metadata_entry() {
        let env = TestEnvironment::new("parse_no_meta");
        let file_path = env.test_dir().join("no_meta_test.pdf");

        // Create PDF without providing info_data
        create_test_pdf(file_path.to_str().unwrap(), None, None)
            .expect("Failed to create test PDF");
        assert!(
            file_path.exists(),
            "Test PDF does not exist after creation!"
        );

        let result = parse_pdf(file_path.to_str().unwrap());

        assert!(result.is_ok(), "parse_pdf failed: {:?}", result.err());
        let metadata = result.unwrap();
        assert!(
            metadata.is_empty(),
            "Metadata map should be empty when Info dict is missing"
        );
    }

    #[test]
    fn test_parse_pdf_info_not_a_reference() {
        let env = TestEnvironment::new("parse_info_not_ref");
        let file_path = env.test_dir().join("meta_not_ref_test.pdf");
        let mut info = BTreeMap::new();
        info.insert("Title", "Invalid Structure");

        // Create PDF with Info as direct dictionary (invalid trailer structure)
        create_test_pdf(file_path.to_str().unwrap(), Some(info), Some("not_a_ref"))
            .expect("Failed to create test PDF");
        assert!(
            file_path.exists(),
            "Test PDF does not exist after creation!"
        );

        let result = parse_pdf(file_path.to_str().unwrap());

        // Expect Ok with empty map, as the function should handle this gracefully
        assert!(
            result.is_ok(),
            "parse_pdf should not error if Info is not a ref: {:?}",
            result.err()
        );
        let metadata = result.unwrap();
        assert!(
            metadata.is_empty(),
            "Metadata map should be empty when Info is not a valid reference"
        );
    }

    #[test]
    fn test_parse_pdf_info_bad_reference_type() {
        let env = TestEnvironment::new("parse_info_bad_ref_type");
        let file_path = env.test_dir().join("meta_bad_ref_type_test.pdf");
        let mut info = BTreeMap::new();
        info.insert("Title", "Invalid Structure");

        // Create PDF with Info pointing to an array (invalid type)
        create_test_pdf(
            file_path.to_str().unwrap(),
            Some(info),
            Some("bad_ref_type"),
        )
        .expect("Failed to create test PDF");
        assert!(
            file_path.exists(),
            "Test PDF does not exist after creation!"
        );

        let result = parse_pdf(file_path.to_str().unwrap());

        // Expect Ok with empty map, as the function should handle this gracefully
        assert!(
            result.is_ok(),
            "parse_pdf should not error if Info ref points to wrong type: {:?}",
            result.err()
        );
        let metadata = result.unwrap();
        assert!(
            metadata.is_empty(),
            "Metadata map should be empty when Info ref points to wrong type"
        );
    }

    #[test]
    fn test_parse_pdf_file_not_found() {
        use crate::pdf::test_utils::{setup_unique_paths, teardown_unique_paths};
        let (test_dir, output_dir) = setup_unique_paths("parser_not_found");
        let bad_path_buf = test_dir.join("no_way_this_exists.pdf");
        let bad_path = bad_path_buf.to_str().unwrap();

        let result = parse_pdf(bad_path);

        assert!(
            result.is_err(),
            "parse_pdf should fail for non-existent file"
        );
        let err_msg = result.err().unwrap();
        println!("Actual error for not found: {}", err_msg); // Debug print

        assert!(
            err_msg.contains("Input file not found"),
            "Error message should indicate missing file/dir: {}",
            err_msg
        );
        assert!(
            err_msg.contains(bad_path),
            "Error message should contain path: {}",
            err_msg
        );

        teardown_unique_paths(&test_dir, &output_dir);
    }

    #[test]
    fn test_parse_pdf_not_a_pdf() {
        let env = TestEnvironment::new("parse_not_a_pdf");
        let file_path = env.test_dir().join("not_a_pdf.txt");

        // Create a simple text file
        let mut file = fs::File::create(&file_path).expect("Failed to create dummy text file");
        writeln!(file, "This is not a PDF file.").expect("Failed to write to text file");
        assert!(
            file_path.exists(),
            "Non-PDF Test file does not exist after creation!"
        );

        let result = parse_pdf(file_path.to_str().unwrap());

        assert!(result.is_err(), "parse_pdf should fail for non-PDF file");
        let err_msg = result.err().unwrap();
        // Check for a plausible error message from lopdf::Document::load
        assert!(
            err_msg.contains("Failed to load or parse PDF"),
            "Error message mismatch: {}",
            err_msg
        );
        // Examples: "invalid PDF header", "cannot find trailer", "failed to read" etc.
    }
}
