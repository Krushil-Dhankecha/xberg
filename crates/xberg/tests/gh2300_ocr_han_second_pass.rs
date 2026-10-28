//! Regression coverage for GH#2300: an image whose first OCR pass is noisy and whose script is
//! Han is read again with the Chinese language added to the configured ones.
//!
//! The fixture is a generated picture of four neutral Chinese sentences. With the default
//! language `eng` the first pass returns Latin noise. Tesseract's script detection reports Han,
//! and the second pass reads the sentences.

#![cfg(feature = "ocr")]

mod helpers;

use helpers::extract_bytes_document_blocking;
use xberg::core::config::{ExtractionConfig, OcrConfig};

const CHINESE_LINES: &[u8] = include_bytes!("fixtures/ocr/gh2300_chinese_lines.png");
const ENGLISH_TABLE: &[u8] = include_bytes!("fixtures/ocr/gh1785_shaded_rows.png");
/// The sentences are about 50 characters long. Half of them is far above the Latin noise.
const MIN_HAN_CHARACTERS: usize = 25;

fn han_characters(text: &str) -> usize {
    text.chars().filter(|c| ('\u{4E00}'..='\u{9FFF}').contains(c)).count()
}

fn config(language: &[&str]) -> ExtractionConfig {
    ExtractionConfig {
        ocr: Some(OcrConfig {
            language: language.iter().map(|l| l.to_string()).collect(),
            ..Default::default()
        }),
        use_cache: false,
        ..Default::default()
    }
}

fn read(bytes: &[u8], config: &ExtractionConfig) -> String {
    extract_bytes_document_blocking(bytes, "image/png", config)
        .expect("OCR of the fixture must not error")
        .content
}

#[test]
fn a_chinese_image_read_with_the_default_language_is_read_as_chinese() {
    let text = read(CHINESE_LINES, &config(&["eng"]));
    assert!(
        han_characters(&text) >= MIN_HAN_CHARACTERS,
        "the default language must still read Chinese through the second pass, got {text:?}"
    );
}

#[test]
fn an_english_image_keeps_its_first_pass() {
    let text = read(ENGLISH_TABLE, &config(&["eng"]));
    assert_eq!(
        han_characters(&text),
        0,
        "an English picture gains no Chinese, got {text:?}"
    );
    assert!(
        text.to_lowercase().contains("total"),
        "the English text is still read, got {text:?}"
    );
}
