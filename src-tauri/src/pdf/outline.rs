use lopdf::{Document, Object};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct OutlineItem {
    pub title: String,
    pub page: Option<u32>,
    pub children: Vec<OutlineItem>,
}

#[tauri::command]
pub fn get_pdf_outline(path: &str) -> Result<Vec<OutlineItem>, String> {
    let doc = Document::load(path).map_err(|e| e.to_string())?;

    if let Ok(Object::Reference(root_id)) = doc.trailer.get(b"Root") {
        if let Ok(Object::Dictionary(catalog)) = doc.get_object(*root_id) {
            if let Ok(Object::Reference(outlines_id)) = catalog.get(b"Outlines") {
                if let Ok(Object::Dictionary(outlines)) = doc.get_object(*outlines_id) {
                    if let Ok(Object::Reference(first_id)) = outlines.get(b"First") {
                        return Ok(parse_outline_node(&doc, *first_id));
                    }
                }
            }
        }
    }

    Ok(vec![])
}

fn parse_outline_node(doc: &Document, node_id: (u32, u16)) -> Vec<OutlineItem> {
    let mut items = vec![];
    let mut current_id = Some(node_id);

    while let Some(id) = current_id {
        if let Ok(Object::Dictionary(node)) = doc.get_object(id) {
            let title = if let Ok(Object::String(bytes, _)) = node.get(b"Title") {
                String::from_utf8_lossy(bytes).to_string()
            } else {
                "Untitled".to_string()
            };

            // Simplified page extraction
            let page = None;

            let mut children = vec![];
            if let Ok(Object::Reference(first_child)) = node.get(b"First") {
                children = parse_outline_node(doc, *first_child);
            }

            items.push(OutlineItem {
                title,
                page,
                children,
            });

            current_id = if let Ok(Object::Reference(next_id)) = node.get(b"Next") {
                Some(*next_id)
            } else {
                None
            };
        } else {
            break;
        }
    }
    items
}

#[tauri::command]
pub fn set_pdf_outline(
    path: &str,
    items: Vec<OutlineItem>,
    output_path: &str,
) -> Result<(), String> {
    let mut doc = Document::load(path).map_err(|e| e.to_string())?;

    // Create new Outlines hierarchy
    let outlines_id = doc.new_object_id();
    let mut nodes = vec![];
    for item in &items {
        let node_id = doc.new_object_id();
        nodes.push((node_id, item));
    }

    for i in 0..nodes.len() {
        let (id, item) = &nodes[i];
        let mut node_dict = lopdf::Dictionary::new();
        node_dict.set("Title", lopdf::Object::string_literal(item.title.as_str()));

        if i > 0 {
            node_dict.set("Prev", lopdf::Object::Reference(nodes[i - 1].0));
        }
        if i < nodes.len() - 1 {
            node_dict.set("Next", lopdf::Object::Reference(nodes[i + 1].0));
        }

        node_dict.set("Parent", lopdf::Object::Reference(outlines_id));

        if let Some(p) = item.page {
            if let Some(page_id) = doc.get_pages().get(&p) {
                node_dict.set(
                    "Dest",
                    lopdf::Object::Array(vec![
                        lopdf::Object::Reference(*page_id),
                        lopdf::Object::Name(b"Fit".to_vec()),
                    ]),
                );
            }
        }

        doc.objects
            .insert(*id, lopdf::Object::Dictionary(node_dict));
    }

    let mut outlines_dict = lopdf::Dictionary::new();
    outlines_dict.set("Type", lopdf::Object::Name(b"Outlines".to_vec()));
    if !nodes.is_empty() {
        outlines_dict.set("First", lopdf::Object::Reference(nodes[0].0));
        outlines_dict.set("Last", lopdf::Object::Reference(nodes[nodes.len() - 1].0));
    }
    outlines_dict.set("Count", lopdf::Object::Integer(nodes.len() as i64));

    doc.objects
        .insert(outlines_id, lopdf::Object::Dictionary(outlines_dict));

    if let Ok(Object::Reference(root_id)) = doc.trailer.get(b"Root") {
        if let Ok(Object::Dictionary(mut catalog)) = doc.get_object(*root_id).cloned() {
            catalog.set("Outlines", lopdf::Object::Reference(outlines_id));
            doc.objects.insert(*root_id, Object::Dictionary(catalog));
        }
    }

    doc.save(output_path)
        .map_err(|e| format!("Failed to save: {}", e))?;
    Ok(())
}
