use lopdf::{Document, Error as LopdfError, Object, ObjectId};
use std::fs;
use std::path::Path;

#[tauri::command]
pub fn rotate_pdf(
    path: &str,
    pages: Vec<u32>,
    rotation: i32,
    output_path: &str,
) -> Result<(), String> {
    if ![0, 90, 180, 270, -90, -180, -270].contains(&rotation) {
        return Err("Invalid rotation angle. Must be one of 0, 90, 180, 270.".to_string());
    }

    let input_path = Path::new(path);
    if !input_path.exists() {
        return Err(format!("Input file not found: {}", path));
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

    let mut doc =
        Document::load(path).map_err(|e| format!("Failed to load PDF '{}': {}", path, e))?;

    let page_ids = doc.get_pages();
    let pages_to_rotate: Vec<ObjectId> = if pages.is_empty() {
        page_ids.values().cloned().collect()
    } else {
        pages
            .iter()
            .map(|p| {
                page_ids
                    .get(p)
                    .cloned()
                    .ok_or_else(|| format!("Page {} not found in document.", p))
            })
            .collect::<Result<Vec<ObjectId>, String>>()?
    };

    for page_id in pages_to_rotate {
        let page_dict = doc
            .get_object_mut(page_id)
            .and_then(|obj| obj.as_dict_mut())
            .map_err(|e: LopdfError| {
                format!(
                    "Failed to get page dictionary for page {:?}: {}",
                    page_id, e
                )
            })?;

        let current_rotation = page_dict
            .get(b"Rotate")
            .and_then(|obj| obj.as_i64())
            .unwrap_or(0) as i32;

        let new_rotation = (current_rotation + rotation) % 360;

        page_dict.set("Rotate", Object::Integer(new_rotation as i64));
    }

    doc.save(output_path)
        .map_err(|e| format!("Failed to save rotated PDF to '{}': {}", output_path, e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_utils::create_minimal_pdf;
    use lopdf::Document;
    use std::path::PathBuf;

    struct TestEnvironment {
        test_dir: PathBuf,
        output_dir: PathBuf,
        input_pdf_path: PathBuf,
    }

    impl TestEnvironment {
        fn new(test_name: &str) -> Self {
            use crate::pdf::test_utils::setup_unique_paths;
            let (test_dir, output_dir) = setup_unique_paths(test_name);

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

        fn output_path(&self, filename: &str) -> PathBuf {
            self.output_dir.join(filename)
        }

        fn input_path_str(&self) -> &str {
            self.input_pdf_path.to_str().unwrap()
        }
    }

    impl Drop for TestEnvironment {
        fn drop(&mut self) {
            use crate::pdf::test_utils::teardown_unique_paths;
            teardown_unique_paths(&self.test_dir, &self.output_dir);
        }
    }

    #[test]
    fn test_rotate_pdf_success() {
        let env = TestEnvironment::new("rotate_success");
        let output_path = env.output_path("rotated_90.pdf");
        let pages_to_rotate = vec![1, 3];

        let result = rotate_pdf(
            env.input_path_str(),
            pages_to_rotate.clone(),
            90,
            output_path.to_str().unwrap(),
        );

        assert!(result.is_ok(), "rotate_pdf failed: {:?}", result.err());
        assert!(
            output_path.exists(),
            "Output file was not created at {}",
            output_path.display()
        );

        match Document::load(&output_path) {
            Ok(output_doc) => {
                let pages = output_doc.get_pages();
                let page1_id = pages.get(&1).unwrap();
                let page1_dict = output_doc.get_object(*page1_id).unwrap().as_dict().unwrap();
                let rotation1 = page1_dict.get(b"Rotate").unwrap().as_i64().unwrap();
                assert_eq!(rotation1, 90);

                let page2_id = pages.get(&2).unwrap();
                let page2_dict = output_doc.get_object(*page2_id).unwrap().as_dict().unwrap();
                assert!(page2_dict.get(b"Rotate").is_err());

                let page3_id = pages.get(&3).unwrap();
                let page3_dict = output_doc.get_object(*page3_id).unwrap().as_dict().unwrap();
                let rotation3 = page3_dict.get(b"Rotate").unwrap().as_i64().unwrap();
                assert_eq!(rotation3, 90);
            }
            Err(e) => panic!(
                "Failed to load the generated output PDF '{}': {}",
                output_path.display(),
                e
            ),
        }
    }

    #[test]
    fn test_rotate_pdf_invalid_angle() {
        let env = TestEnvironment::new("rotate_invalid_angle");
        let output_path = env.output_path("rotate_invalid.pdf");
        let pages_to_rotate = vec![1];

        let result = rotate_pdf(
            env.input_path_str(),
            pages_to_rotate,
            45,
            output_path.to_str().unwrap(),
        );

        assert!(result.is_err(), "Function should fail for invalid angle");
        if let Err(e) = result {
            assert!(
                e.contains("Invalid rotation angle"),
                "Error message mismatch"
            );
        }
        assert!(!output_path.exists());
    }

    #[test]
    fn test_rotate_pdf_all_pages() {
        let env = TestEnvironment::new("rotate_all_pages");
        let output_path = env.output_path("rotated_all_180.pdf");

        let result = rotate_pdf(
            env.input_path_str(),
            vec![],
            180,
            output_path.to_str().unwrap(),
        );

        assert!(result.is_ok(), "rotate_pdf failed: {:?}", result.err());
        assert!(
            output_path.exists(),
            "Output file was not created at {}",
            output_path.display()
        );

        match Document::load(&output_path) {
            Ok(output_doc) => {
                let pages = output_doc.get_pages();
                for page_num in 1..=pages.len() {
                    let page_id = pages.get(&(page_num as u32)).unwrap();
                    let page_dict = output_doc.get_object(*page_id).unwrap().as_dict().unwrap();
                    let rotation = page_dict.get(b"Rotate").unwrap().as_i64().unwrap();
                    assert_eq!(rotation, 180);
                }
            }
            Err(e) => panic!(
                "Failed to load the generated output PDF '{}': {}",
                output_path.display(),
                e
            ),
        }
    }
}
