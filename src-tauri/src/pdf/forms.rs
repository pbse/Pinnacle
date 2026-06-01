use lopdf::{dictionary, Document, Object};

#[derive(serde::Serialize)]
pub struct FormField {
    pub name: String,
    pub field_type: String,
    pub value: String,
    pub page: u32,
    pub rect: Vec<f32>,
}

#[tauri::command]
pub fn get_form_fields(path: &str) -> Result<Vec<FormField>, String> {
    let doc = Document::load(path).map_err(|e| e.to_string())?;
    let mut fields = vec![];
    let pages = doc.get_pages();
    let mut page_id_to_num = std::collections::HashMap::new();
    for (num, id) in &pages {
        page_id_to_num.insert(*id, *num);
    }

    if let Ok(Object::Reference(root_id)) = doc.trailer.get(b"Root") {
        if let Ok(Object::Dictionary(catalog)) = doc.get_object(*root_id) {
            if let Ok(Object::Reference(acroform_id)) = catalog.get(b"AcroForm") {
                if let Ok(Object::Dictionary(acroform)) = doc.get_object(*acroform_id) {
                    if let Ok(Object::Array(field_refs)) = acroform.get(b"Fields") {
                        for field_ref in field_refs {
                            if let Ok(fid) = field_ref.as_reference() {
                                if let Ok(Object::Dictionary(field)) = doc.get_object(fid) {
                                    let name = if let Ok(Object::String(bytes, _)) = field.get(b"T")
                                    {
                                        String::from_utf8_lossy(bytes).to_string()
                                    } else {
                                        "Unnamed".to_string()
                                    };

                                    let field_type = if let Ok(Object::Name(ft)) = field.get(b"FT")
                                    {
                                        String::from_utf8_lossy(ft).to_string()
                                    } else {
                                        "Unknown".to_string()
                                    };

                                    let value =
                                        if let Ok(Object::String(bytes, _)) = field.get(b"V") {
                                            String::from_utf8_lossy(bytes).to_string()
                                        } else {
                                            "".to_string()
                                        };

                                    let rect = if let Ok(Object::Array(arr)) = field.get(b"Rect") {
                                        arr.iter().filter_map(|o| o.as_f32().ok()).collect()
                                    } else {
                                        vec![0.0, 0.0, 0.0, 0.0]
                                    };

                                    let page = if let Ok(Object::Reference(pid)) = field.get(b"P") {
                                        *page_id_to_num.get(&pid).unwrap_or(&1)
                                    } else {
                                        // If P is missing, we'd need to search all pages for this annot
                                        // For now, default to 1 or try a heuristic
                                        1
                                    };

                                    fields.push(FormField {
                                        name,
                                        field_type,
                                        value,
                                        page,
                                        rect,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(fields)
}

#[tauri::command]
pub fn set_form_fields(
    path: &str,
    updates: std::collections::HashMap<String, String>,
    output_path: &str,
) -> Result<(), String> {
    let mut doc = Document::load(path).map_err(|e| e.to_string())?;
    let mut field_updates = vec![];

    if let Ok(Object::Reference(root_id)) = doc.trailer.get(b"Root") {
        if let Ok(Object::Dictionary(catalog)) = doc.get_object(*root_id) {
            if let Ok(Object::Reference(acroform_id)) = catalog.get(b"AcroForm") {
                if let Ok(Object::Dictionary(acroform)) = doc.get_object(*acroform_id) {
                    if let Ok(Object::Array(field_refs)) = acroform.get(b"Fields") {
                        for field_ref in field_refs {
                            if let Ok(fid) = field_ref.as_reference() {
                                if let Ok(Object::Dictionary(field)) = doc.get_object(fid) {
                                    let name = if let Ok(Object::String(bytes, _)) = field.get(b"T")
                                    {
                                        String::from_utf8_lossy(bytes).to_string()
                                    } else {
                                        continue;
                                    };

                                    if let Some(new_value) = updates.get(&name) {
                                        let mut new_field = field.clone();
                                        new_field
                                            .set("V", Object::string_literal(new_value.as_str()));
                                        new_field.remove(b"AP");
                                        field_updates.push((fid, new_field));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for (fid, field) in field_updates {
        doc.objects.insert(fid, Object::Dictionary(field));
    }

    doc.save(output_path)
        .map_err(|e| format!("Failed to save: {}", e))?;
    Ok(())
}

#[derive(serde::Deserialize)]
pub struct NewFormField {
    pub name: String,
    pub field_type: String, // "Tx" (Text), "Btn" (Button/Checkbox), "Ch" (Choice)
    pub page: u32,
    pub rect: [f32; 4],
}

#[tauri::command]
pub fn create_form_fields(
    path: &str,
    new_fields: Vec<NewFormField>,
    output_path: &str,
) -> Result<(), String> {
    let mut doc = Document::load(path).map_err(|e| e.to_string())?;

    // 1. Ensure AcroForm exists
    let root_id = doc
        .trailer
        .get(b"Root")
        .and_then(|obj| obj.as_reference())
        .map_err(|_| "No Root found")?;
    let mut root = doc
        .get_object(root_id)
        .and_then(|obj| obj.as_dict())
        .map_err(|_| "Root is not a dict")?
        .clone();

    let acroform_id = if let Ok(id) = root.get(b"AcroForm").and_then(|obj| obj.as_reference()) {
        id
    } else {
        let id = doc.add_object(dictionary! {
            "Fields" => Vec::<Object>::new(),
            "NeedAppearances" => true,
        });
        root.set("AcroForm", Object::Reference(id));
        doc.objects.insert(root_id, Object::Dictionary(root));
        id
    };

    let mut acroform = doc
        .get_object(acroform_id)
        .and_then(|obj| obj.as_dict())
        .map_err(|_| "AcroForm is not a dict")?
        .clone();
    let mut global_fields = if let Ok(Object::Array(arr)) = acroform.get(b"Fields") {
        arr.clone()
    } else {
        Vec::new()
    };

    let pages = doc.get_pages();

    for nf in new_fields {
        let page_id = *pages
            .get(&nf.page)
            .ok_or_else(|| format!("Page {} not found", nf.page))?;

        let rect = Object::Array(vec![
            nf.rect[0].into(),
            nf.rect[1].into(),
            nf.rect[2].into(),
            nf.rect[3].into(),
        ]);

        let mut field_dict = dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "FT" => nf.field_type.as_str(),
            "T" => Object::string_literal(nf.name.as_str()),
            "Rect" => rect,
            "P" => page_id,
            "F" => 4_i64, // Print flag
        };

        if nf.field_type == "Tx" {
            field_dict.set("DA", Object::string_literal("/Helv 12 Tf 0 g"));
        } else if nf.field_type == "Btn" {
            // Basic Checkbox setup
            field_dict.set("V", "Off");
            field_dict.set("AS", "Off");
        }

        let field_id = doc.add_object(field_dict);
        global_fields.push(Object::Reference(field_id));

        // Add to page Annots
        if let Ok(Object::Dictionary(mut page)) = doc.get_object_mut(page_id).cloned() {
            match page.get_mut(b"Annots") {
                Ok(Object::Array(ref mut annots)) => {
                    annots.push(Object::Reference(field_id));
                }
                _ => {
                    page.set("Annots", Object::Array(vec![Object::Reference(field_id)]));
                }
            }
            doc.objects.insert(page_id, Object::Dictionary(page));
        }
    }

    acroform.set("Fields", global_fields);
    doc.objects
        .insert(acroform_id, Object::Dictionary(acroform));

    doc.save(output_path)
        .map_err(|e| format!("Failed to save: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_utils::{create_minimal_pdf, setup_unique_paths, teardown_unique_paths};

    #[test]
    fn test_get_form_fields_empty() {
        let (test_dir, output_dir) = setup_unique_paths("forms");
        let input_path = test_dir.join("input.pdf");

        create_minimal_pdf(input_path.to_str().unwrap(), 1, "FormsTest").unwrap();

        let result = get_form_fields(input_path.to_str().unwrap());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);

        teardown_unique_paths(&test_dir, &output_dir);
    }

    #[test]
    fn test_create_form_fields_success() {
        let (test_dir, output_dir) = setup_unique_paths("create_forms");
        let input_path = test_dir.join("input.pdf");
        let output_path = output_dir.join("output.pdf");

        create_minimal_pdf(input_path.to_str().unwrap(), 1, "CreateFormsTest").unwrap();

        let new_fields = vec![NewFormField {
            name: "TestField".to_string(),
            field_type: "Tx".to_string(),
            page: 1,
            rect: [100.0, 100.0, 200.0, 150.0],
        }];

        let result = create_form_fields(
            input_path.to_str().unwrap(),
            new_fields,
            output_path.to_str().unwrap(),
        );

        assert!(result.is_ok());
        assert!(output_path.exists());

        // Verify fields are there
        let fields = get_form_fields(output_path.to_str().unwrap()).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "TestField");

        teardown_unique_paths(&test_dir, &output_dir);
    }
}
