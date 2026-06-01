use image::GenericImageView;
use lopdf::{dictionary, Document, Object, Stream};
use std::path::Path;

#[tauri::command]
pub fn images_to_pdf(image_paths: Vec<String>, output_path: &str) -> Result<(), String> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut kids = vec![];

    for path_str in image_paths {
        let path = Path::new(&path_str);
        if !path.exists() {
            continue;
        }

        let img =
            image::open(path).map_err(|e| format!("Failed to open image {}: {}", path_str, e))?;
        let (width, height) = img.dimensions();

        // Convert image to JPEG for small PDF size
        let mut jpeg_data = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut jpeg_data),
            image::ImageFormat::Jpeg,
        )
        .map_err(|e| format!("Failed to process image: {}", e))?;

        let stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => width as i64,
                "Height" => height as i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
                "Filter" => "DCTDecode",
            },
            jpeg_data,
        );

        let image_id = doc.add_object(stream);

        // Create page that fits the image
        let content = format!("q {} 0 0 {} 0 0 cm /Im1 Do Q", width, height);
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => dictionary! {
                "XObject" => dictionary! { "Im1" => image_id }
            },
            "MediaBox" => vec![0.into(), 0.into(), (width as f32).into(), (height as f32).into()],
        });

        kids.push(Object::Reference(page_id));
    }

    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => Object::Integer(count),
        }),
    );

    let root_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });

    doc.trailer.set("Root", root_id);
    doc.save(output_path)
        .map_err(|e| format!("Failed to save PDF: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::test_utils::{setup_unique_paths, teardown_unique_paths};

    #[test]
    fn test_images_to_pdf_empty() {
        let (test_dir, output_dir) = setup_unique_paths("img2pdf");
        let output_path = output_dir.join("output.pdf");

        let result = images_to_pdf(vec![], output_path.to_str().unwrap());
        assert!(result.is_ok());
        // Even with no images, it creates a PDF with no pages (or at least doesn't crash)

        teardown_unique_paths(&test_dir, &output_dir);
    }
}
