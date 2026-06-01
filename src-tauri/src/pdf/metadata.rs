use lopdf::{Document, Object, StringFormat};

#[tauri::command]
pub fn update_metadata(
    path: &str,
    title: Option<String>,
    author: Option<String>,
    subject: Option<String>,
    keywords: Option<String>,
    output_path: &str,
) -> Result<(), String> {
    let mut doc = Document::load(path).map_err(|e| format!("Failed to load: {}", e))?;

    let info_id = if let Ok(id) = doc.trailer.get(b"Info") {
        match id {
            Object::Reference(id) => *id,
            _ => doc.add_object(lopdf::Dictionary::new()),
        }
    } else {
        let id = doc.add_object(lopdf::Dictionary::new());
        doc.trailer.set("Info", Object::Reference(id));
        id
    };

    if let Ok(Object::Dictionary(ref mut info)) = doc.get_object_mut(info_id) {
        if let Some(t) = title {
            info.set(
                "Title",
                Object::String(t.into_bytes(), StringFormat::Literal),
            );
        }
        if let Some(a) = author {
            info.set(
                "Author",
                Object::String(a.into_bytes(), StringFormat::Literal),
            );
        }
        if let Some(s) = subject {
            info.set(
                "Subject",
                Object::String(s.into_bytes(), StringFormat::Literal),
            );
        }
        if let Some(k) = keywords {
            info.set(
                "Keywords",
                Object::String(k.into_bytes(), StringFormat::Literal),
            );
        }
        info.set(
            "Producer",
            Object::String(
                "Pinnacle World-Class Assistant".into(),
                StringFormat::Literal,
            ),
        );
    }

    doc.save(output_path)
        .map_err(|e| format!("Failed to save: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn batch_update_metadata(
    paths: Vec<String>,
    title: Option<String>,
    author: Option<String>,
    subject: Option<String>,
    keywords: Option<String>,
) -> Result<(), String> {
    for path in paths {
        let output_path = path.clone();
        update_metadata(
            &path,
            title.clone(),
            author.clone(),
            subject.clone(),
            keywords.clone(),
            &output_path,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_utils::{create_minimal_pdf, setup_unique_paths, teardown_unique_paths};

    #[test]
    fn test_update_metadata_success() {
        let (test_dir, output_dir) = setup_unique_paths("metadata_success");
        let input_path = test_dir.join("input.pdf");
        create_minimal_pdf(input_path.to_str().unwrap(), 1, "Content").unwrap();

        let output_path = output_dir.join("output.pdf");
        let result = update_metadata(
            input_path.to_str().unwrap(),
            Some("New Title".to_string()),
            Some("Author Name".to_string()),
            None,
            None,
            output_path.to_str().unwrap(),
        );

        assert!(result.is_ok());
        let output_doc = Document::load(output_path).unwrap();
        let info_id = output_doc
            .trailer
            .get(b"Info")
            .unwrap()
            .as_reference()
            .unwrap();
        let info = output_doc.get_object(info_id).unwrap().as_dict().unwrap();

        let title = info.get(b"Title").unwrap().as_str().unwrap();
        assert_eq!(String::from_utf8_lossy(title), "New Title");

        teardown_unique_paths(&test_dir, &output_dir);
    }
}
