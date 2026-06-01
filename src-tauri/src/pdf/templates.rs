use printpdf::*;
use quick_xml::{events::Event, Reader};
use std::fs::File;
use std::io::{BufWriter, Read};
use zip::ZipArchive;

const PAGE_WIDTH_MM: f32 = 210.0;
const PAGE_HEIGHT_MM: f32 = 297.0;
const LEFT_MARGIN_MM: f32 = 20.0;
const TOP_TEXT_Y_MM: f32 = 270.0;
const BODY_START_Y_MM: f32 = 250.0;
const BOTTOM_MARGIN_MM: f32 = 20.0;
const BODY_FONT_SIZE: f32 = 11.0;
const LINE_HEIGHT_MM: f32 = 6.5;
const MAX_BODY_CHARS: usize = 92;

#[tauri::command]
pub fn markdown_to_pdf(title: String, content: String, output_path: String) -> Result<(), String> {
    text_to_pdf(&title, &content, &output_path)
}

#[tauri::command]
pub fn office_to_pdf(path: String, output_path: String) -> Result<(), String> {
    let lower = path.to_lowercase();
    let title = std::path::Path::new(&path)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Converted document")
        .to_string();

    let content = if lower.ends_with(".docx") {
        extract_docx_text(&path)?
    } else if lower.ends_with(".xlsx") {
        extract_xlsx_text(&path)?
    } else {
        return Err("Unsupported office document type.".to_string());
    };

    let fallback = if content.trim().is_empty() {
        "No readable text was found in this document.".to_string()
    } else {
        content
    };

    text_to_pdf(&title, &fallback, &output_path)
}

fn text_to_pdf(title: &str, content: &str, output_path: &str) -> Result<(), String> {
    let (doc, page1, layer1) =
        PdfDocument::new(title, Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), "Layer 1");
    let mut current_layer = doc.get_page(page1).get_layer(layer1);

    // Load a professional font
    let font = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| e.to_string())?;

    // Header
    current_layer.use_text(
        &sanitize_pdf_text(title),
        22.0,
        Mm(LEFT_MARGIN_MM),
        Mm(TOP_TEXT_Y_MM),
        &font,
    );

    let font_body = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| e.to_string())?;

    let mut y = BODY_START_Y_MM;
    let mut page_number = 1;

    for raw_line in content.lines() {
        let line = sanitize_pdf_text(raw_line);
        let wrapped = wrap_text(&line, MAX_BODY_CHARS);
        let lines = if wrapped.is_empty() {
            vec![String::new()]
        } else {
            wrapped
        };

        for line in lines {
            if y < BOTTOM_MARGIN_MM {
                page_number += 1;
                let (page, layer) = doc.add_page(
                    Mm(PAGE_WIDTH_MM),
                    Mm(PAGE_HEIGHT_MM),
                    format!("Layer {}", page_number),
                );
                current_layer = doc.get_page(page).get_layer(layer);
                y = TOP_TEXT_Y_MM;
            }

            current_layer.use_text(line, BODY_FONT_SIZE, Mm(LEFT_MARGIN_MM), Mm(y), &font_body);
            y -= LINE_HEIGHT_MM;
        }
    }

    let file = File::create(output_path).map_err(|e| e.to_string())?;
    doc.save(&mut BufWriter::new(file))
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn extract_docx_text(path: &str) -> Result<String, String> {
    let mut archive = open_zip(path)?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|e| format!("DOCX document.xml missing: {}", e))?
        .read_to_string(&mut xml)
        .map_err(|e| format!("Failed to read DOCX XML: {}", e))?;

    Ok(extract_xml_text(&xml, &[b"t"], &[b"p"]))
}

fn extract_xlsx_text(path: &str) -> Result<String, String> {
    let mut archive = open_zip(path)?;
    let shared_strings = read_shared_strings(&mut archive).unwrap_or_default();
    let mut output = String::new();
    let names: Vec<String> = archive.file_names().map(|name| name.to_string()).collect();

    for name in names {
        if !name.starts_with("xl/worksheets/") || !name.ends_with(".xml") {
            continue;
        }

        let mut xml = String::new();
        archive
            .by_name(&name)
            .map_err(|e| format!("Failed to open worksheet {}: {}", name, e))?
            .read_to_string(&mut xml)
            .map_err(|e| format!("Failed to read worksheet {}: {}", name, e))?;

        let sheet_text = extract_xlsx_sheet_text(&xml, &shared_strings);
        if !sheet_text.trim().is_empty() {
            if !output.is_empty() {
                output.push_str("\n\n");
            }
            output.push_str(&name.replace("xl/worksheets/", "").replace(".xml", ""));
            output.push('\n');
            output.push_str(&sheet_text);
        }
    }

    Ok(output)
}

fn open_zip(path: &str) -> Result<ZipArchive<File>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path, e))?;
    ZipArchive::new(file).map_err(|e| format!("Failed to read zipped office file: {}", e))
}

fn read_shared_strings(archive: &mut ZipArchive<File>) -> Result<Vec<String>, String> {
    let mut xml = String::new();
    archive
        .by_name("xl/sharedStrings.xml")
        .map_err(|e| format!("No shared strings found: {}", e))?
        .read_to_string(&mut xml)
        .map_err(|e| format!("Failed to read shared strings: {}", e))?;

    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut strings = Vec::new();
    let mut current = String::new();
    let mut inside_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"t" => inside_text = true,
            Ok(Event::End(e)) if e.name().as_ref() == b"t" => inside_text = false,
            Ok(Event::End(e)) if e.name().as_ref() == b"si" => {
                strings.push(current.trim().to_string());
                current.clear();
            }
            Ok(Event::Text(e)) if inside_text => {
                current.push_str(&e.unescape().map_err(|err| err.to_string())?);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Failed to parse shared strings: {}", e)),
            _ => {}
        }
    }

    Ok(strings)
}

fn extract_xml_text(xml: &str, text_tags: &[&[u8]], break_tags: &[&[u8]]) -> String {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut output = String::new();
    let mut inside_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if text_tags.iter().any(|tag| *tag == e.name().as_ref()) => {
                inside_text = true;
            }
            Ok(Event::End(e)) if text_tags.iter().any(|tag| *tag == e.name().as_ref()) => {
                inside_text = false;
                output.push(' ');
            }
            Ok(Event::End(e)) if break_tags.iter().any(|tag| *tag == e.name().as_ref()) => {
                output.push('\n');
            }
            Ok(Event::Text(e)) if inside_text => {
                if let Ok(text) = e.unescape() {
                    output.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    compact_lines(&output)
}

fn extract_xlsx_sheet_text(xml: &str, shared_strings: &[String]) -> String {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut output = String::new();
    let mut in_value = false;
    let mut current_is_shared = false;
    let mut row_values: Vec<String> = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"c" => {
                current_is_shared = e
                    .attributes()
                    .with_checks(false)
                    .filter_map(Result::ok)
                    .any(|attr| {
                        attr.key.as_ref() == b"t"
                            && attr
                                .decode_and_unescape_value(reader.decoder())
                                .map(|value| value == "s")
                                .unwrap_or(false)
                    });
            }
            Ok(Event::Start(e)) if e.name().as_ref() == b"v" => in_value = true,
            Ok(Event::End(e)) if e.name().as_ref() == b"v" => in_value = false,
            Ok(Event::End(e)) if e.name().as_ref() == b"c" => current_is_shared = false,
            Ok(Event::End(e)) if e.name().as_ref() == b"row" => {
                if !row_values.is_empty() {
                    output.push_str(&row_values.join(" | "));
                    output.push('\n');
                    row_values.clear();
                }
            }
            Ok(Event::Text(e)) if in_value => {
                let raw = e
                    .unescape()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                let value = if current_is_shared {
                    raw.parse::<usize>()
                        .ok()
                        .and_then(|index| shared_strings.get(index))
                        .cloned()
                        .unwrap_or(raw)
                } else {
                    raw
                };

                if !value.trim().is_empty() {
                    row_values.push(value);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    compact_lines(&output)
}

fn compact_lines(text: &str) -> String {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn sanitize_pdf_text(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_ascii() && !ch.is_control() {
                ch
            } else if ch == '\t' || ch == '\r' || ch == '\n' {
                ' '
            } else {
                ' '
            }
        })
        .collect::<String>()
}

fn wrap_text(line: &str, max_chars: usize) -> Vec<String> {
    if line.trim().is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();

    for word in line.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + word.len() + 1 <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}
