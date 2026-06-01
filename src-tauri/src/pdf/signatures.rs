use lopdf::{dictionary, Document, Object};
use openssl::cms::{CMSOptions, CmsContentInfo};
use openssl::pkcs12::Pkcs12;
use openssl::pkey::PKey;
use openssl::stack::Stack;
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::X509;
use std::fs;
use std::path::Path;

fn normalize_rect(mut rect: [f32; 4]) -> [f32; 4] {
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
        None => Object::Array(vec![0.1f32.into(), 0.4f32.into(), 1.0f32.into()]),
    }
}

#[tauri::command]
pub fn add_signature_visual(
    path: &str,
    page: u32,
    rect: [f32; 4],
    strokes: Vec<Vec<[f32; 2]>>,
    color: Option<[f32; 3]>,
    width: Option<f32>,
    output_path: &str,
) -> Result<(), String> {
    if page == 0 {
        return Err("Page number must be 1-based.".to_string());
    }
    if strokes.is_empty() || strokes.iter().all(|s| s.is_empty()) {
        return Err("Signature requires at least one point.".to_string());
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

    // Build InkList (array of strokes)
    let mut ink_list = Vec::with_capacity(strokes.len());
    for points in &strokes {
        let mut ink_array = Vec::with_capacity(points.len() * 2);
        for pt in points {
            ink_array.push(pt[0].into());
            ink_array.push(pt[1].into());
        }
        ink_list.push(Object::Array(ink_array));
    }

    let mut annot = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Ink",
        "Rect" => rect_obj,
        "InkList" => Object::Array(ink_list),
        "C" => color_array(color),
        "Border" => Object::Array(vec![0.into(), 0.into(), width.unwrap_or(2.0).max(0.1).into()]),
        "F" => 4_i64, // Print flag
    };
    annot.set("Contents", Object::string_literal("Visual signature (ink)"));

    // Appearance stream so the ink is visible across viewers
    let x0 = rect[0];
    let y0 = rect[1];
    let bbox_width = (rect[2] - rect[0]).max(1.0);
    let bbox_height = (rect[3] - rect[1]).max(1.0);
    let mut ap_stream = String::new();
    // set color and stroke width
    let stroke_width = width.unwrap_or(2.0).max(0.1);
    let col = color.unwrap_or([0.1, 0.4, 1.0]);
    ap_stream.push_str(&format!("{:.3} {:.3} {:.3} RG\n", col[0], col[1], col[2]));
    ap_stream.push_str(&format!("{:.2} w\n", stroke_width));

    // Process each stroke for the appearance stream
    for points in &strokes {
        if let Some(first) = points.first() {
            ap_stream.push_str(&format!("{:.2} {:.2} m\n", first[0] - x0, first[1] - y0));
            for p in points.iter().skip(1) {
                ap_stream.push_str(&format!("{:.2} {:.2} l\n", p[0] - x0, p[1] - y0));
            }
            ap_stream.push_str("S\n");
        }
    }
    let ap_dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "BBox" => Object::Array(vec![0.into(), 0.into(), bbox_width.into(), bbox_height.into()]),
        "Resources" => dictionary! {},
    };
    let ap_ref = doc.add_object(lopdf::Stream::new(ap_dict, ap_stream.into_bytes()));

    let mut annot_dict = annot;
    annot_dict.set("AP", dictionary! { "N" => Object::Reference(ap_ref) });
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
        .map_err(|e| format!("Failed to save signed PDF to '{}': {}", output_path, e))?;

    Ok(())
}

/// Validate inputs for cryptographic signing without yet performing CMS signing.
#[tauri::command]
pub fn sign_pdf_pfx(
    path: &str,
    page: u32,
    rect: [f32; 4],
    pfx_path: &str,
    pfx_password: String,
    reason: Option<String>,
    location: Option<String>,
    contact: Option<String>,
    output_path: &str,
) -> Result<(), String> {
    if page == 0 {
        return Err("Page number must be 1-based.".to_string());
    }
    let input_path = Path::new(path);
    if !input_path.exists() || !input_path.is_file() {
        return Err(format!("Input file not found or not a file: {}", path));
    }
    let pfx = Path::new(pfx_path);
    if !pfx.exists() || !pfx.is_file() {
        return Err(format!("PFX file not found or not a file: {}", pfx_path));
    }
    if pfx_password.is_empty() {
        return Err("PFX password cannot be empty.".to_string());
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
    let rect = normalize_rect(rect);
    if rect[0] == rect[2] || rect[1] == rect[3] {
        return Err("Signature rectangle must have width and height.".to_string());
    }

    // Load document and check page exists
    let mut doc =
        Document::load(path).map_err(|e| format!("Failed to load PDF '{}': {}", path, e))?;
    let pages = doc.get_pages();
    if !pages.contains_key(&page) {
        return Err(format!(
            "Page number {} not found in document ({} pages).",
            page,
            pages.len()
        ));
    }

    // Load PFX
    let pfx_bytes =
        fs::read(pfx_path).map_err(|e| format!("Failed to read PFX '{}': {}", pfx_path, e))?;
    let parsed = Pkcs12::from_der(&pfx_bytes)
        .map_err(|e| format!("Invalid PFX: {}", e))?
        .parse2(&pfx_password)
        .map_err(|e| format!("Failed to parse PFX: {}", e))?;
    let pkey: PKey<openssl::pkey::Private> = parsed.pkey.ok_or("No private key in PFX")?;
    let cert: X509 = parsed.cert.ok_or("No certificate in PFX")?;
    let mut chain_stack = Stack::new().map_err(|e| e.to_string())?;
    if let Some(chain) = parsed.ca {
        for c in chain {
            chain_stack.push(c).map_err(|e| e.to_string())?;
        }
    }

    // Build signature dictionary with placeholders
    let sig_id = doc.new_object_id();
    let widget_id = doc.new_object_id();
    let rect_obj = Object::Array(vec![
        rect[0].into(),
        rect[1].into(),
        rect[2].into(),
        rect[3].into(),
    ]);

    let mut sig_dict = dictionary! {
        "Type" => "Sig",
        "Filter" => "Adobe.PPKLite",
        "SubFilter" => "adbe.pkcs7.detached",
        "ByteRange" => Object::Array(vec![0.into(), 0.into(), 0.into(), 0.into()]),
        "Contents" => Object::String(vec![0u8; 8192], lopdf::StringFormat::Hexadecimal),
        "M" => chrono::Utc::now().format("D:%Y%m%d%H%M%S+00'00'").to_string(),
    };
    if let Some(r) = reason.clone() {
        sig_dict.set("Reason", Object::string_literal(r));
    }
    if let Some(l) = location.clone() {
        sig_dict.set("Location", Object::string_literal(l));
    }
    if let Some(c) = contact.clone() {
        sig_dict.set("ContactInfo", Object::string_literal(c));
    }
    doc.objects.insert(sig_id, Object::Dictionary(sig_dict));

    let widget = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "FT" => "Sig",
        "Rect" => rect_obj,
        "P" => Object::Reference(*pages.get(&page).unwrap()),
        "V" => Object::Reference(sig_id),
        "T" => Object::string_literal("Signature1"),
        "F" => 4_i64,
    };
    doc.objects
        .insert(widget_id, Object::Dictionary(widget.clone()));

    // Attach widget to page Annots
    {
        let page_id = *pages.get(&page).unwrap();
        let page_obj = doc
            .get_object_mut(page_id)
            .map_err(|e| format!("Failed to fetch page object {:?}: {}", page_id, e))?;
        let page_dict = page_obj
            .as_dict_mut()
            .map_err(|_| "Page object is not a dictionary".to_string())?;

        match page_dict.get_mut(b"Annots") {
            Ok(annots_obj) => {
                if let Ok(arr) = annots_obj.as_array_mut() {
                    arr.push(Object::Reference(widget_id));
                } else {
                    return Err("Existing Annots entry is not an array".to_string());
                }
            }
            Err(_) => {
                page_dict.set("Annots", Object::Array(vec![Object::Reference(widget_id)]));
            }
        }
    }

    // Ensure AcroForm exists and includes the widget
    {
        let catalog_id = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = doc
            .get_object_mut(catalog_id)
            .map_err(|e| format!("Failed to get catalog: {}", e))?;
        let catalog_dict = catalog
            .as_dict_mut()
            .map_err(|_| "Catalog is not a dictionary".to_string())?;

        let fields_array = match catalog_dict.get_mut(b"AcroForm") {
            Ok(acro_obj) => {
                let acro_dict = acro_obj
                    .as_dict_mut()
                    .map_err(|_| "AcroForm is not a dictionary".to_string())?;
                acro_dict.get_mut(b"Fields")
            }
            Err(_) => {
                let mut acro = dictionary! { "Fields" => Object::Array(vec![]) };
                acro.set("SigFlags", Object::Integer(3));
                catalog_dict.set("AcroForm", acro);
                catalog_dict
                    .get_mut(b"AcroForm")
                    .unwrap()
                    .as_dict_mut()
                    .unwrap()
                    .get_mut(b"Fields")
            }
        }
        .map_err(|_| "Failed to get Fields array".to_string())?;

        if let Ok(arr) = fields_array.as_array_mut() {
            arr.push(Object::Reference(widget_id));
        } else {
            return Err("Fields is not an array".to_string());
        }
    }

    // Save to buffer with placeholders
    let mut buffer = Vec::new();
    doc.save_to(&mut buffer)
        .map_err(|e| format!("Failed to serialize PDF: {}", e))?;

    // Locate placeholders
    let br_marker = b"/ByteRange [";
    let br_pos = buffer
        .windows(br_marker.len())
        .position(|w| w == br_marker)
        .ok_or_else(|| "ByteRange not found".to_string())?;
    let contents_marker = b"/Contents <";
    let contents_pos = buffer
        .windows(contents_marker.len())
        .position(|w| w == contents_marker)
        .ok_or_else(|| "Contents not found".to_string())?;

    // Parse positions
    let contents_start = contents_pos + contents_marker.len();
    let contents_end = buffer[contents_start..]
        .iter()
        .position(|&b| b == b'>')
        .ok_or_else(|| "Contents end not found".to_string())?
        + contents_start;
    let contents_len = contents_end - contents_start;

    let b_range_start = br_pos + br_marker.len();
    let b_range_end = buffer[b_range_start..]
        .iter()
        .position(|&b| b == b']')
        .ok_or_else(|| "ByteRange end not found".to_string())?
        + b_range_start;

    // Compute ByteRange: [0, contents_start- (br1), after_contents_start (br2_start), len2]
    let br1_start = 0;
    let br1_len = contents_start - br1_start - 1; // exclude '<'
    let br2_start = contents_end + 1;
    let br2_len = buffer.len() - br2_start;

    let new_br = format!("[{} {} {} {}]", br1_start, br1_len, br2_start, br2_len);
    // replace inside buffer
    let mut br_bytes = buffer.clone();
    let br_slice = &mut br_bytes[b_range_start..b_range_start + new_br.len()];
    br_slice.copy_from_slice(new_br.as_bytes());
    // fill remaining with spaces if placeholder longer
    for i in b_range_start + new_br.len()..=b_range_end {
        if i < br_bytes.len() {
            br_bytes[i] = b' ';
        }
    }

    // Build signed data
    let mut signed_data = Vec::with_capacity(br1_len + br2_len);
    signed_data.extend_from_slice(&br_bytes[0..br1_len + br1_start]);
    signed_data.extend_from_slice(&br_bytes[br2_start..br2_start + br2_len]);

    let cms = CmsContentInfo::sign(
        Some(&cert),
        Some(&pkey),
        Some(&chain_stack),
        Some(&signed_data),
        CMSOptions::DETACHED | CMSOptions::BINARY,
    )
    .map_err(|e| format!("CMS sign failed: {}", e))?;
    let signature_der = cms
        .to_der()
        .map_err(|e| format!("DER encode failed: {}", e))?;
    if signature_der.len() * 2 > contents_len {
        return Err(format!(
            "Signature too large for placeholder ({} > {}).",
            signature_der.len() * 2,
            contents_len
        ));
    }
    // encode to hex
    let mut hex_sig = hex::encode(signature_der);
    // pad to placeholder length
    if hex_sig.len() < contents_len {
        hex_sig.push_str(&"0".repeat(contents_len - hex_sig.len()));
    }
    let contents_slice = &mut br_bytes[contents_start..contents_start + contents_len];
    contents_slice.copy_from_slice(hex_sig.as_bytes());

    fs::write(output_path, &br_bytes)
        .map_err(|e| format!("Failed to write signed PDF '{}': {}", output_path, e))?;

    Ok(())
}

/// Stub for signature verification; returns a controlled error until implemented.
#[tauri::command]
pub fn verify_signatures(path: &str) -> Result<Vec<String>, String> {
    let input_path = Path::new(path);
    if !input_path.exists() || !input_path.is_file() {
        return Err(format!("Input file not found or not a file: {}", path));
    }
    let pdf_bytes = fs::read(path).map_err(|e| format!("Failed to read PDF: {}", e))?;

    let br_marker = b"/ByteRange [";
    let br_pos = pdf_bytes
        .windows(br_marker.len())
        .position(|w| w == br_marker)
        .ok_or_else(|| "ByteRange not found".to_string())?;
    let contents_marker = b"/Contents <";
    let contents_pos = pdf_bytes
        .windows(contents_marker.len())
        .position(|w| w == contents_marker)
        .ok_or_else(|| "Contents not found".to_string())?;
    let contents_start = contents_pos + contents_marker.len();
    let contents_end = pdf_bytes[contents_start..]
        .iter()
        .position(|&b| b == b'>')
        .ok_or_else(|| "Contents end not found".to_string())?
        + contents_start;

    // Parse ByteRange numbers
    let br_start = br_pos + br_marker.len();
    let br_end = pdf_bytes[br_start..]
        .iter()
        .position(|&b| b == b']')
        .ok_or_else(|| "ByteRange end not found".to_string())?
        + br_start;
    let br_str = std::str::from_utf8(&pdf_bytes[br_start..br_end]).map_err(|e| e.to_string())?;
    let nums: Vec<u64> = br_str
        .split_whitespace()
        .filter_map(|s| s.parse::<u64>().ok())
        .collect();
    if nums.len() != 4 {
        return Err("Invalid ByteRange values".to_string());
    }
    let (br1_start, br1_len, br2_start, br2_len) = (
        nums[0] as usize,
        nums[1] as usize,
        nums[2] as usize,
        nums[3] as usize,
    );

    // Extract signed data
    let mut signed_data = Vec::with_capacity(br1_len + br2_len);
    signed_data.extend_from_slice(&pdf_bytes[br1_start..br1_start + br1_len]);
    signed_data.extend_from_slice(&pdf_bytes[br2_start..br2_start + br2_len]);

    // Extract signature hex
    let sig_hex =
        std::str::from_utf8(&pdf_bytes[contents_start..contents_end]).map_err(|e| e.to_string())?;
    let sig_bytes = hex::decode(sig_hex.trim_end_matches('0'))
        .map_err(|e| format!("Invalid hex signature: {}", e))?;
    let mut cms =
        CmsContentInfo::from_der(&sig_bytes).map_err(|e| format!("CMS parse failed: {}", e))?;

    let mut store_builder = X509StoreBuilder::new().map_err(|e| e.to_string())?;
    store_builder.set_default_paths().ok(); // best-effort system roots
    let store = store_builder.build();
    let mut data = signed_data.clone();
    cms.verify(
        None,
        Some(&store),
        None,
        Some(&mut data),
        CMSOptions::DETACHED | CMSOptions::BINARY,
    )
    .map_err(|e| format!("Signature verification failed: {}", e))?;

    Ok(vec![
        "Signature verified (trust depends on system roots)".to_string()
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_utils::create_minimal_pdf;
    use std::path::PathBuf;

    #[test]
    fn test_add_signature_visual_ok() {
        let base = PathBuf::from("target/test_data_signature_visual");
        let out = PathBuf::from("target/test_output_signature_visual");
        fs::create_dir_all(&base).ok();
        fs::create_dir_all(&out).ok();
        let input = base.join("sig.pdf");
        create_minimal_pdf(input.to_str().unwrap(), 1, "Sig").expect("create");
        let output = out.join("sig_out.pdf");

        let result = add_signature_visual(
            input.to_str().unwrap(),
            1,
            [50.0, 650.0, 250.0, 700.0],
            vec![vec![[60.0, 660.0], [120.0, 690.0], [200.0, 660.0]]],
            Some([0.2, 0.8, 0.4]),
            Some(2.5),
            output.to_str().unwrap(),
        );
        assert!(
            result.is_ok(),
            "add_signature_visual failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_add_signature_visual_invalid_page() {
        let base = PathBuf::from("target/test_data_signature_invalid");
        let out = PathBuf::from("target/test_output_signature_invalid");
        fs::create_dir_all(&base).ok();
        fs::create_dir_all(&out).ok();
        let input = base.join("sig.pdf");
        create_minimal_pdf(input.to_str().unwrap(), 1, "Sig").expect("create");
        let output = out.join("sig_out.pdf");

        let result = add_signature_visual(
            input.to_str().unwrap(),
            5,
            [10.0, 10.0, 50.0, 50.0],
            vec![vec![[10.0, 10.0], [20.0, 20.0]]],
            None,
            None,
            output.to_str().unwrap(),
        );
        assert!(result.is_err());
    }
}
