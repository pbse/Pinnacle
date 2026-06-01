use lopdf::{dictionary, Document, Object};
use std::fs;
use std::path::Path;

fn normalize_rect(mut rect: [f32; 4]) -> [f32; 4] {
    // Ensure x1<=x2 and y1<=y2
    if rect[0] > rect[2] {
        rect.swap(0, 2);
    }
    if rect[1] > rect[3] {
        rect.swap(1, 3);
    }
    rect
}

fn color_array(color: Option<[f32; 3]>) -> Object {
    match color {
        Some([r, g, b]) => {
            let clamp = |v: f32| v.max(0.0).min(1.0);
            Object::Array(vec![clamp(r).into(), clamp(g).into(), clamp(b).into()])
        }
        None => Object::Array(vec![1f32.into(), 1f32.into(), 0f32.into()]), // default yellow
    }
}

#[tauri::command]
pub fn add_annotation(
    path: &str,
    page: u32,
    rect: [f32; 4],
    kind: String,
    contents: Option<String>,
    color: Option<[f32; 3]>,
    output_path: &str,
) -> Result<(), String> {
    if page == 0 {
        return Err("Page number must be 1-based.".to_string());
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

    let mut doc =
        Document::load(path).map_err(|e| format!("Failed to load PDF '{}': {}", path, e))?;

    let pages = doc.get_pages();
    let page_id = *pages.get(&page).ok_or_else(|| {
        format!(
            "Page number {} not found in document ({} pages).",
            page,
            pages.len()
        )
    })?;

    let rect = normalize_rect(rect);
    let rect_obj = Object::Array(vec![
        rect[0].into(),
        rect[1].into(),
        rect[2].into(),
        rect[3].into(),
    ]);

    let subtype = kind.to_lowercase();
    let annot_dict = match subtype.as_str() {
        "highlight" => {
            let quad = Object::Array(vec![
                rect[0].into(),
                rect[3].into(),
                rect[2].into(),
                rect[3].into(),
                rect[0].into(),
                rect[1].into(),
                rect[2].into(),
                rect[1].into(),
            ]);
            let mut d = dictionary! {
                "Type" => "Annot",
                "Subtype" => "Highlight",
                "Rect" => rect_obj.clone(),
                "QuadPoints" => quad,
                "C" => color_array(color),
                "F" => 4_i64, // Print flag
            };
            if let Some(text) = contents {
                d.set("Contents", Object::string_literal(text));
            }
            d
        }
        "underline" => {
            let quad = Object::Array(vec![
                rect[0].into(),
                rect[3].into(),
                rect[2].into(),
                rect[3].into(),
                rect[0].into(),
                rect[1].into(),
                rect[2].into(),
                rect[1].into(),
            ]);
            let mut d = dictionary! {
                "Type" => "Annot",
                "Subtype" => "Underline",
                "Rect" => rect_obj.clone(),
                "QuadPoints" => quad,
                "C" => color_array(color),
                "F" => 4_i64,
            };
            if let Some(text) = contents {
                d.set("Contents", Object::string_literal(text));
            }
            d
        }
        "strikeout" => {
            let quad = Object::Array(vec![
                rect[0].into(),
                rect[3].into(),
                rect[2].into(),
                rect[3].into(),
                rect[0].into(),
                rect[1].into(),
                rect[2].into(),
                rect[1].into(),
            ]);
            let mut d = dictionary! {
                "Type" => "Annot",
                "Subtype" => "StrikeOut",
                "Rect" => rect_obj.clone(),
                "QuadPoints" => quad,
                "C" => color_array(color),
                "F" => 4_i64,
            };
            if let Some(text) = contents {
                d.set("Contents", Object::string_literal(text));
            }
            d
        }
        "note" | "text" | "freetext" => {
            let mut d = dictionary! {
                "Type" => "Annot",
                "Subtype" => "FreeText",
                "Rect" => rect_obj.clone(),
                "C" => color_array(color),
                "F" => 4_i64,
                "DA" => Object::string_literal("0 0 0 rg /Helv 12 Tf"),
            };
            if let Some(text) = contents.clone() {
                d.set("Contents", Object::string_literal(text.clone()));

                // Generate Appearance Stream
                let w = rect[2] - rect[0];
                let h = rect[3] - rect[1];

                let mut ap_content = String::new();
                ap_content.push_str("q\n");
                ap_content.push_str("0 0 0 rg\n"); // Black text
                ap_content.push_str("BT\n/Helv 10 Tf\n"); // Font Helvetica 10pt

                // Basic text wrapping / positioning
                let mut current_y = h - 12.0;
                for line in text.lines() {
                    if current_y < 0.0 { break; }
                    ap_content.push_str(&format!("2 {} Td\n", current_y));
                    ap_content.push_str(&format!("({}) Tj\n", line.replace("(", "\\(").replace(")", "\\)")));
                    ap_content.push_str(&format!("-2 -{} Td\n", current_y)); // Reset X
                    current_y -= 12.0;
                }
                ap_content.push_str("ET\nQ\n");

                let ap_dict = dictionary! {
                    "Type" => "XObject",
                    "Subtype" => "Form",
                    "BBox" => Object::Array(vec![0.into(), 0.into(), w.into(), h.into()]),
                    "Resources" => dictionary! {
                        "Font" => dictionary! {
                            "Helv" => dictionary! {
                                "Type" => "Font",
                                "Subtype" => "Type1",
                                "BaseFont" => "Helvetica",
                                "Encoding" => "WinAnsiEncoding",
                            }
                        }
                    },
                };
                let ap_ref = doc.add_object(lopdf::Stream::new(ap_dict, ap_content.into_bytes()));
                d.set("AP", dictionary! { "N" => Object::Reference(ap_ref) });
            }
            d
        }
        "redact" | "blackout" => {
            dictionary! {
                "Type" => "Annot",
                "Subtype" => "Square",
                "Rect" => rect_obj.clone(),
                "C" => Object::Array(vec![0f32.into(), 0f32.into(), 0f32.into()]),
                "IC" => Object::Array(vec![0f32.into(), 0f32.into(), 0f32.into()]),
                "BS" => dictionary! { "W" => 0_i64 },
                "F" => 4_i64,
            }
        }
        "square" | "rectangle" => {
            dictionary! {
                "Type" => "Annot",
                "Subtype" => "Square",
                "Rect" => rect_obj.clone(),
                "C" => color_array(color),
                "F" => 4_i64,
                "BS" => dictionary! { "W" => 2_i64 }, // Border style width 2
            }
        }
        "circle" | "oval" => {
            dictionary! {
                "Type" => "Annot",
                "Subtype" => "Circle",
                "Rect" => rect_obj.clone(),
                "C" => color_array(color),
                "F" => 4_i64,
                "BS" => dictionary! { "W" => 2_i64 },
            }
        }
        _ => {
            return Err(format!(
                "Unsupported annotation type '{}'. Allowed: highlight, underline, strikeout, note, redact, square, circle",
                kind
            ))
        }
    };

    let annot_id = doc.add_object(Object::Dictionary(annot_dict));

    {
        let page_obj = doc
            .get_object_mut(page_id)
            .map_err(|e| format!("Failed to fetch page object {:?}: {}", page_id, e))?;
        let page_dict = page_obj
            .as_dict_mut()
            .map_err(|_| "Page object is not a dictionary".to_string())?;

        match page_dict.get_mut(b"Annots") {
            Ok(annots_obj) => {
                if let Ok(arr) = annots_obj.as_array_mut() {
                    arr.push(Object::Reference(annot_id));
                } else {
                    return Err("Existing Annots entry is not an array".to_string());
                }
            }
            Err(_) => {
                page_dict.set("Annots", Object::Array(vec![Object::Reference(annot_id)]));
            }
        }
    }

    doc.save(output_path)
        .map_err(|e| format!("Failed to save annotated PDF to '{}': {}", output_path, e))?;

    Ok(())
}

#[tauri::command]
pub fn add_ink_annotation(
    path: &str,
    page: u32,
    gestures: Vec<Vec<[f32; 2]>>,
    color: Option<[f32; 3]>,
    width: Option<f32>,
    output_path: &str,
) -> Result<(), String> {
    if page == 0 {
        return Err("Page number must be 1-based.".to_string());
    }
    let mut doc = Document::load(path).map_err(|e| e.to_string())?;
    let pages = doc.get_pages();
    let page_id = *pages.get(&page).ok_or("Page not found")?;

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    let mut ink_list = Vec::new();
    for gesture in gestures {
        let mut path_points = Vec::new();
        for [x, y] in gesture {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            path_points.push(x.into());
            path_points.push(y.into());
        }
        ink_list.push(Object::Array(path_points));
    }

    let rect = Object::Array(vec![min_x.into(), min_y.into(), max_x.into(), max_y.into()]);

    let annot_dict = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Ink",
        "Rect" => rect,
        "InkList" => Object::Array(ink_list),
        "C" => color_array(color),
        "F" => 4_i64,
        "BS" => dictionary! { "W" => width.unwrap_or(2.0) as i64 },
    };

    let annot_id = doc.add_object(Object::Dictionary(annot_dict));

    if let Ok(page_obj) = doc.get_object_mut(page_id) {
        if let Ok(page_dict) = page_obj.as_dict_mut() {
            if let Ok(annots) = page_dict.get_mut(b"Annots") {
                if let Ok(arr) = annots.as_array_mut() {
                    arr.push(Object::Reference(annot_id));
                }
            } else {
                page_dict.set("Annots", Object::Array(vec![Object::Reference(annot_id)]));
            }
        }
    }

    doc.save(output_path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_annotation(
    path: &str,
    annot_id: (u32, u16),
    output_path: &str,
) -> Result<(), String> {
    let mut doc = Document::load(path).map_err(|e| e.to_string())?;

    // 1. Remove from all page Annots arrays
    let page_ids: Vec<_> = doc.get_pages().values().cloned().collect();
    for page_id in page_ids {
        if let Ok(Object::Dictionary(mut page)) = doc.get_object_mut(page_id).cloned() {
            if let Ok(Object::Array(ref mut annots)) = page.get_mut(b"Annots") {
                annots.retain(|a| {
                    if let Ok(id) = a.as_reference() {
                        id != annot_id
                    } else {
                        true
                    }
                });
                doc.objects.insert(page_id, Object::Dictionary(page));
            }
        }
    }

    // 2. Remove the object itself
    doc.objects.remove(&annot_id);

    doc.save(output_path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_annotation_contents(
    path: &str,
    annot_id: (u32, u16),
    new_contents: String,
    output_path: &str,
) -> Result<(), String> {
    let mut doc = Document::load(path).map_err(|e| e.to_string())?;

    if let Ok(Object::Dictionary(mut annot)) = doc.get_object_mut(annot_id).cloned() {
        annot.set("Contents", Object::string_literal(new_contents));
        doc.objects.insert(annot_id, Object::Dictionary(annot));
    } else {
        return Err("Annotation not found or not a dictionary".to_string());
    }

    doc.save(output_path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_utils::create_minimal_pdf;
    use std::path::PathBuf;

    #[test]
    fn test_add_highlight_annotation() {
        let base = PathBuf::from("target/test_data_annotations");
        let out = PathBuf::from("target/test_output_annotations");
        fs::create_dir_all(&base).ok();
        fs::create_dir_all(&out).ok();

        let input = base.join("annot.pdf");
        create_minimal_pdf(input.to_str().unwrap(), 2, "Annot").expect("create");
        let output = out.join("annot_out.pdf");

        let result = add_annotation(
            input.to_str().unwrap(),
            1,
            [50.0, 650.0, 250.0, 670.0],
            "highlight".to_string(),
            Some("Test highlight".to_string()),
            Some([1.0, 0.9, 0.3]),
            output.to_str().unwrap(),
        );
        assert!(result.is_ok(), "add_annotation failed: {:?}", result.err());

        let doc = Document::load(output.to_str().unwrap()).expect("load");
        let pages = doc.get_pages();
        let page_id = pages.get(&1).unwrap();
        let page = doc.get_object(*page_id).unwrap().as_dict().unwrap();
        let annots = page.get(b"Annots").unwrap().as_array().unwrap();
        assert_eq!(annots.len(), 1);

        let annot_ref = annots[0].as_reference().unwrap();
        let annot_dict = doc.get_object(annot_ref).unwrap().as_dict().unwrap();
        assert_eq!(
            annot_dict.get(b"Subtype").unwrap().as_name_str().unwrap(),
            "Highlight"
        );
    }

    #[test]
    fn test_add_square_annotation() {
        let (test_dir, output_dir) = crate::pdf::test_utils::setup_unique_paths("square");
        let input = test_dir.join("input.pdf");
        let output = output_dir.join("output.pdf");
        create_minimal_pdf(input.to_str().unwrap(), 1, "Square").unwrap();

        let result = add_annotation(
            input.to_str().unwrap(),
            1,
            [100.0, 100.0, 200.0, 200.0],
            "square".to_string(),
            None,
            None,
            output.to_str().unwrap(),
        );
        assert!(result.is_ok());
        crate::pdf::test_utils::teardown_unique_paths(&test_dir, &output_dir);
    }

    #[test]
    fn test_add_ink_annotation_success() {
        let (test_dir, output_dir) = crate::pdf::test_utils::setup_unique_paths("ink");
        let input = test_dir.join("input.pdf");
        let output = output_dir.join("output.pdf");
        create_minimal_pdf(input.to_str().unwrap(), 1, "Ink").unwrap();

        let gestures = vec![
            vec![[10.0, 10.0], [20.0, 20.0], [30.0, 10.0]],
            vec![[50.0, 50.0], [60.0, 60.0]],
        ];

        let result = add_ink_annotation(
            input.to_str().unwrap(),
            1,
            gestures,
            Some([1.0, 0.0, 0.0]),
            Some(3.0),
            output.to_str().unwrap(),
        );
        assert!(result.is_ok());
        crate::pdf::test_utils::teardown_unique_paths(&test_dir, &output_dir);
    }

    #[test]
    fn test_add_underline_and_strikeout() {
        let base = PathBuf::from("target/test_data_annotations_multi");
        let out = PathBuf::from("target/test_output_annotations_multi");
        fs::create_dir_all(&base).ok();
        fs::create_dir_all(&out).ok();

        let input = base.join("annot.pdf");
        create_minimal_pdf(input.to_str().unwrap(), 1, "Annot").expect("create");
        let output = out.join("annot_out.pdf");

        let res1 = add_annotation(
            input.to_str().unwrap(),
            1,
            [40.0, 640.0, 200.0, 660.0],
            "underline".to_string(),
            None,
            None,
            output.to_str().unwrap(),
        );
        assert!(res1.is_ok(), "underline failed: {:?}", res1.err());

        // Apply strikeout on the output file
        let res2 = add_annotation(
            output.to_str().unwrap(),
            1,
            [60.0, 620.0, 220.0, 640.0],
            "strikeout".to_string(),
            Some("strike".to_string()),
            Some([1.0, 0.2, 0.2]),
            output.to_str().unwrap(),
        );
        assert!(res2.is_ok(), "strikeout failed: {:?}", res2.err());
    }

    #[test]
    fn test_add_note_annotation() {
        let base = PathBuf::from("target/test_data_annotations_note");
        let out = PathBuf::from("target/test_output_annotations_note");
        fs::create_dir_all(&base).ok();
        fs::create_dir_all(&out).ok();

        let input = base.join("annot.pdf");
        create_minimal_pdf(input.to_str().unwrap(), 1, "Annot").expect("create");
        let output = out.join("annot_out.pdf");

        let res = add_annotation(
            input.to_str().unwrap(),
            1,
            [30.0, 600.0, 70.0, 640.0],
            "note".to_string(),
            Some("Note body".to_string()),
            Some([0.1, 0.6, 1.0]),
            output.to_str().unwrap(),
        );
        assert!(res.is_ok(), "note failed: {:?}", res.err());
    }

    #[test]
    fn test_add_text_annotation_invalid_page() {
        let base = PathBuf::from("target/test_data_annotations_invalid");
        let out = PathBuf::from("target/test_output_annotations_invalid");
        fs::create_dir_all(&base).ok();
        fs::create_dir_all(&out).ok();

        let input = base.join("annot.pdf");
        create_minimal_pdf(input.to_str().unwrap(), 1, "Annot").expect("create");
        let output = out.join("annot_out.pdf");

        let result = add_annotation(
            input.to_str().unwrap(),
            5,
            [10.0, 10.0, 50.0, 50.0],
            "text".to_string(),
            None,
            None,
            output.to_str().unwrap(),
        );
        assert!(result.is_err());
    }
}
