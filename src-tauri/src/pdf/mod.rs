pub mod utils;

// Declare the submodules within the 'pdf' module
pub mod extractor;
pub mod merger;
pub mod parser;
pub mod splitter;

pub mod rotator;

pub mod annotation_reader;
pub mod briefing;
pub mod compare;
pub mod convert;
pub mod editor;
pub mod forensic_redact;
pub mod forms;
pub mod image_to_pdf;
pub mod metadata;
pub mod outline;
pub mod rasterizer;
pub mod remover;
pub mod reorder;
pub mod sanitize;
pub mod security_utils;
pub mod templates;
pub mod watcher;
pub mod watermark;

// Shared helpers only compiled for tests
#[cfg(test)]
pub mod test_utils;

pub mod annotations;
pub mod signatures;

// Optional but recommended: Re-export the functions you want to be easily accessible
// from the 'pdf' module itself, hiding the internal structure (parser, merger, etc.)
// This makes the import in main.rs cleaner.
pub use annotation_reader::get_annotations;
pub use annotations::{
    add_annotation, add_ink_annotation, delete_annotation, update_annotation_contents,
};
pub use briefing::generate_briefing;
pub use compare::compare_pdfs_text;
pub use convert::pdf_to_layout_json;
pub use convert::pdf_to_text;
pub use convert::pdf_to_text_string;
pub use convert::write_text_file;
pub use editor::{pdf_to_docx, replace_text_block};
pub use extractor::extract_pdf_page;
pub use forensic_redact::forensic_redact;
pub use forms::{create_form_fields, get_form_fields, set_form_fields};
pub use image_to_pdf::images_to_pdf;
pub use merger::merge_pdfs;
pub use metadata::{batch_update_metadata, update_metadata};
pub use outline::{get_pdf_outline, set_pdf_outline};
pub use parser::parse_pdf;
pub use rasterizer::pdf_to_images;
pub use remover::delete_pages;
pub use reorder::reorder_pages;
pub use rotator::rotate_pdf;
pub use sanitize::sanitize_pdf;
pub use security_utils::{compress_pdf, decrypt_pdf, encrypt_pdf, flatten_annotations};
pub use signatures::add_signature_visual;
pub use signatures::sign_pdf_pfx;
pub use signatures::verify_signatures;
pub use splitter::split_pdf;
pub use templates::{markdown_to_pdf, office_to_pdf};
pub use watcher::start_folder_watcher;
pub use watermark::add_watermark;
