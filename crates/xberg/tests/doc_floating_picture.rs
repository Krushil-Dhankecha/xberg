//! Regression test for #2273: a floating picture in a `.doc` reaches the extracted images.
//!
//! Word 97-2003 anchors a picture positioned over the text with `\x08`; `PlcSpaMom` names
//! its shape, and the shape's `pib` selects a BSE whose blip lies in the WordDocument stream
//! at `foDelay`. The extractor only followed inline pictures, so floating ones were skipped
//! and their text never reached embedded OCR.
//!
//! The fixture is synthetic: LibreOffice saved a document with placeholder text and a
//! generated PNG as Word 97-2003. It is the file attached to the issue. Being a writer's own
//! layout rather than a test builder's, it checks the reading of the format the unit tests
//! in `extraction::doc` encode.

#![cfg(feature = "office")]

mod helpers;
use helpers::extract_bytes_document_blocking;

use xberg::core::config::{ExtractionConfig, ImageExtractionConfig, OutputFormat};

#[test]
fn floating_picture_is_extracted_between_its_surrounding_paragraphs() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/office/doc_floating_picture.doc");
    let bytes = std::fs::read(&path).expect("floating picture fixture must be present");
    let config = ExtractionConfig {
        images: Some(ImageExtractionConfig {
            extract_images: true,
            ..Default::default()
        }),
        output_format: OutputFormat::Markdown,
        use_cache: false,
        ..Default::default()
    };

    let document =
        extract_bytes_document_blocking(&bytes, "application/msword", &config).expect("extraction must succeed");

    assert!(
        document.processing_warnings.is_empty(),
        "{:?}",
        document.processing_warnings
    );
    let images = document.images.as_deref().unwrap_or_default();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].format, "png");
    assert!(images[0].data.starts_with(b"\x89PNG\r\n\x1a\n"));
    image::load_from_memory(&images[0].data).expect("the picture must decode");

    let content = &document.content;
    let before = content.find("BEFORE THE IMAGE").expect("native text before");
    let placeholder = content.find("![").expect("the picture has a placeholder");
    let after = content.find("AFTER THE IMAGE").expect("native text after");
    assert!(before < placeholder && placeholder < after, "{content:?}");
}
