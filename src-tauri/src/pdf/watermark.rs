use lopdf::{content::Content, dictionary, Document, Object};

#[tauri::command]
pub fn add_watermark(
    path: &str,
    text: &str,
    opacity: f32,
    color: [f32; 3],
    output_path: &str,
) -> Result<(), String> {
    let mut doc = Document::load(path).map_err(|e| e.to_string())?;

    for (_, page_id) in doc.get_pages() {
        // ... (rest of the logic remains the same until content creation)
        let gs_name = "WatermarkGS";
        let font_name = "WatermarkFont";

        // Define Graphics State for Opacity
        let gs_dict = dictionary! {
            "Type" => "ExtGState",
            "ca" => opacity,
            "CA" => opacity,
        };

        let gs_id = doc.add_object(gs_dict);

        let mut existing_res_id = None;
        if let Ok(Object::Dictionary(page_dict)) = doc.get_object(page_id) {
            if let Ok(Object::Reference(id)) = page_dict.get(b"Resources") {
                existing_res_id = Some(*id);
            }
        }

        let res_id = if let Some(id) = existing_res_id {
            id
        } else {
            let id = doc.add_object(lopdf::Dictionary::new());
            if let Ok(Object::Dictionary(ref mut page_dict)) = doc.get_object_mut(page_id) {
                page_dict.set("Resources", Object::Reference(id));
            }
            id
        };

        if let Ok(Object::Dictionary(ref mut res_dict)) = doc.get_object_mut(res_id) {
            let mut extgstates = if let Ok(Object::Dictionary(egs)) = res_dict.get(b"ExtGState") {
                egs.clone()
            } else {
                lopdf::Dictionary::new()
            };
            extgstates.set(gs_name, Object::Reference(gs_id));
            res_dict.set("ExtGState", Object::Dictionary(extgstates));

            let mut fonts = if let Ok(Object::Dictionary(f)) = res_dict.get(b"Font") {
                f.clone()
            } else {
                lopdf::Dictionary::new()
            };
            fonts.set(
                font_name,
                dictionary! {
                    "Type" => "Font",
                    "Subtype" => "Type1",
                    "BaseFont" => "Helvetica-Bold",
                },
            );
            res_dict.set("Font", Object::Dictionary(fonts));
        }

        let content = format!(
            "\nq\n/{} gs\n{} {} {} rg\nBT\n/{} 60 Tf\n0.707 0.707 -0.707 0.707 100 100 Tm\n({}) Tj\nET\nQ\n",
            gs_name, color[0], color[1], color[2], font_name, text
        );

        let content_bytes = content.into_bytes();
        let content_ops = Content::decode(&content_bytes)
            .map_err(|e| format!("Failed to decode watermark: {}", e))?;
        doc.add_to_page_content(page_id, content_ops)
            .map_err(|e| format!("Failed to add watermark content: {}", e))?;
    }

    doc.save(output_path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_utils::{create_minimal_pdf, setup_unique_paths, teardown_unique_paths};

    #[test]
    fn test_add_watermark_success() {
        let (test_dir, output_dir) = setup_unique_paths("watermark_success");
        let input_path = test_dir.join("input.pdf");
        create_minimal_pdf(input_path.to_str().unwrap(), 2, "Watermark Test").unwrap();

        let output_path = output_dir.join("output.pdf");
        let result = add_watermark(
            input_path.to_str().unwrap(),
            "CONFIDENTIAL",
            0.5,
            [1.0, 0.0, 0.0],
            output_path.to_str().unwrap(),
        );

        assert!(result.is_ok());
        let output_doc = Document::load(output_path).unwrap();
        assert_eq!(output_doc.get_pages().len(), 2);

        teardown_unique_paths(&test_dir, &output_dir);
    }
}
