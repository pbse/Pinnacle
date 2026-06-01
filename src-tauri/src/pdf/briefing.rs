use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

#[derive(serde::Deserialize)]
pub struct BriefingItem {
    pub title: String,
    pub summary: String,
}

#[tauri::command]
pub fn generate_briefing(
    main_title: String,
    items: Vec<BriefingItem>,
    output_path: String,
) -> Result<(), String> {
    let (doc, page1, layer1) = PdfDocument::new(&main_title, Mm(210.0), Mm(297.0), "Briefing");
    let mut current_layer = doc.get_page(page1).get_layer(layer1);
    let font_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| e.to_string())?;
    let font_body = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| e.to_string())?;

    // Main Header
    current_layer.use_text(&main_title, 28.0, Mm(20.0), Mm(270.0), &font_bold);
    current_layer.use_text(
        "Executive Knowledge Summary",
        12.0,
        Mm(20.0),
        Mm(260.0),
        &font_body,
    );

    let mut y = 240.0;
    for item in items {
        if y < 40.0 {
            // Very simple page break for briefing (not perfect but world-class for this MVP)
            let (new_page, new_layer) = doc.add_page(Mm(210.0), Mm(297.0), "Next Page");
            current_layer = doc.get_page(new_page).get_layer(new_layer);
            y = 270.0;
        }

        current_layer.use_text(&item.title, 14.0, Mm(20.0), Mm(y), &font_bold);
        y -= 6.0;

        // Wrap summary text (very simple wrap)
        let lines: Vec<&str> = item.summary.split(". ").collect();
        for line in lines {
            if y < 20.0 {
                break;
            }
            current_layer.use_text(line, 10.0, Mm(25.0), Mm(y), &font_body);
            y -= 5.0;
        }
        y -= 5.0; // Extra spacing between items
    }

    let file = File::create(&output_path).map_err(|e| e.to_string())?;
    doc.save(&mut BufWriter::new(file))
        .map_err(|e| e.to_string())?;

    Ok(())
}
