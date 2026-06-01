use lopdf::{Document, Object};

#[derive(serde::Serialize)]
pub struct PdfAnnotation {
    pub id: (u32, u16),
    pub page: u32,
    pub kind: String,
    pub contents: Option<String>,
    pub rect: [f32; 4],
}

#[tauri::command]
pub fn get_annotations(path: &str) -> Result<Vec<PdfAnnotation>, String> {
    let doc = Document::load(path).map_err(|e| e.to_string())?;
    let mut results = vec![];

    for (page_num, page_id) in doc.get_pages() {
        if let Ok(Object::Dictionary(page)) = doc.get_object(page_id) {
            if let Ok(Object::Array(annots)) = page.get(b"Annots") {
                for annot_ref in annots {
                    if let Ok(id) = annot_ref.as_reference() {
                        if let Ok(Object::Dictionary(annot)) = doc.get_object(id) {
                            let kind = if let Ok(Object::Name(k)) = annot.get(b"Subtype") {
                                String::from_utf8_lossy(k).to_string()
                            } else {
                                "Unknown".to_string()
                            };

                            let contents =
                                if let Ok(Object::String(bytes, _)) = annot.get(b"Contents") {
                                    Some(String::from_utf8_lossy(bytes).to_string())
                                } else {
                                    None
                                };

                            let rect = if let Ok(Object::Array(r)) = annot.get(b"Rect") {
                                if r.len() == 4 {
                                    [
                                        r[0].as_f32().unwrap_or(0.0),
                                        r[1].as_f32().unwrap_or(0.0),
                                        r[2].as_f32().unwrap_or(0.0),
                                        r[3].as_f32().unwrap_or(0.0),
                                    ]
                                } else {
                                    [0.0; 4]
                                }
                            } else {
                                [0.0; 4]
                            };

                            results.push(PdfAnnotation {
                                id,
                                page: page_num,
                                kind,
                                contents,
                                rect,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(results)
}
