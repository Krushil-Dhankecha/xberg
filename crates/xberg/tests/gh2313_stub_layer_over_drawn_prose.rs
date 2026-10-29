//! GH#2313 -- a page whose prose is drawn (outlined glyphs, tiled pictures) carries only a
//! title and a footer as text. In auto mode the page must reach OCR: the scan score must
//! clear the default threshold and the page must be in `scanned_pages`, which is the list the
//! OCR router reads. The PDF is built here, so the test needs no fixture file.
//!
//! The controls break one condition of the rule each and must stay at a score of zero.

#![cfg(feature = "pdf")]

mod helpers;
use helpers::extract_bytes_document_blocking;
use xberg::core::config::DEFAULT_SCANNED_MIN_CONFIDENCE;
use xberg::{ExtractionConfig, FormatMetadata, PdfConfig};

const PAGE_W: f32 = 612.0;
const PAGE_H: f32 = 792.0;

/// One-page PDF: a raster covering a quarter of the page, `rectangles` filled rectangles (the
/// drawn prose), and the text layer `text_lines` set in Helvetica at `font` points.
fn build_pdf(rectangles: usize, text_lines: &[&str]) -> Vec<u8> {
    build_pdf_sized(rectangles, text_lines, STUB_FONT)
}

fn build_pdf_sized(rectangles: usize, text_lines: &[&str], font: usize) -> Vec<u8> {
    let mut content = String::from("q 306 0 0 396 0 0 cm /Im1 Do Q\n0 g\n");
    for i in 0..rectangles {
        let x = 20 + (i % 40) * 14;
        let y = 120 + (i / 40) * 9;
        content.push_str(&format!("{x} {y} 10 5 re f\n"));
    }
    for (n, line) in text_lines.iter().enumerate() {
        let y = 740 - n * 28;
        content.push_str(&format!("BT /F1 {font} Tf 30 {y} Td ({line}) Tj ET\n"));
    }

    let image = vec![0x80u8; 64];
    let objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] \
             /Resources << /Font << /F1 5 0 R >> /XObject << /Im1 6 0 R >> >> /Contents 4 0 R >>"
        )
        .into_bytes(),
        [
            format!("<< /Length {} >>\nstream\n", content.len()).into_bytes(),
            content.into_bytes(),
            b"\nendstream".to_vec(),
        ]
        .concat(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
        [
            format!(
                "<< /Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceGray \
                 /BitsPerComponent 8 /Length {} >>\nstream\n",
                image.len()
            )
            .into_bytes(),
            image,
            b"\nendstream".to_vec(),
        ]
        .concat(),
    ];

    let mut pdf = b"%PDF-1.5\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        pdf.extend_from_slice(body);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for off in &offsets {
        pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

fn scan_result(pdf: &[u8]) -> (f64, bool) {
    let config = ExtractionConfig {
        disable_ocr: true,
        use_cache: false,
        enable_quality_processing: false,
        pdf_options: Some(PdfConfig {
            extract_tables: false,
            ..Default::default()
        }),
        ..Default::default()
    };
    let result = extract_bytes_document_blocking(pdf, "application/pdf", &config).expect("extract synthetic PDF");
    let Some(FormatMetadata::Pdf(pdf_metadata)) = result.metadata.format else {
        panic!("the synthetic PDF produced no PDF metadata");
    };
    let confidence = pdf_metadata.scanned_confidence.expect("scanned_confidence is set");
    let selected = pdf_metadata.scanned_pages.is_some_and(|pages| pages.contains(&1));
    (f64::from(confidence), selected)
}

/// At 20 pt the two stub lines cover more than the header-sized cap (0.02 of the page) and at
/// most the rule's stub bound (0.03), so only the new rule selects the page. At 24 pt and
/// above they cover more than the stub bound and the rule must not fire.
const STUB_FONT: usize = 20;
const OVERSIZED_FONT: usize = 28;

const STUB: [&str; 2] = ["Annual Report of the Council", "Page 1 of the bound volume"];

#[test]
fn drawn_prose_under_a_stub_text_layer_is_sent_to_ocr() {
    let (confidence, selected) = scan_result(&build_pdf(600, &STUB));
    assert!(
        confidence >= DEFAULT_SCANNED_MIN_CONFIDENCE,
        "a drawn page with a stub text layer must clear the default threshold: got {confidence}"
    );
    assert!(selected, "the page must be in scanned_pages so auto mode runs OCR on it");
}

#[test]
fn a_page_with_few_drawn_paths_is_not_sent_to_ocr() {
    let (confidence, selected) = scan_result(&build_pdf(10, &STUB));
    assert_eq!(confidence, 0.0, "a page that is not drawn keeps a score of zero");
    assert!(!selected);
}

#[test]
fn a_page_with_a_real_text_layer_is_not_sent_to_ocr() {
    let line = "The council met on the first day of the month and passed the budget in full";
    let lines = [line; 20];
    let (confidence, selected) = scan_result(&build_pdf(600, &lines));
    assert_eq!(confidence, 0.0, "a real text layer keeps a score of zero");
    assert!(!selected);
}

#[test]
fn a_text_layer_wider_than_the_stub_bound_is_not_sent_to_ocr() {
    let (confidence, selected) = scan_result(&build_pdf_sized(600, &STUB, OVERSIZED_FONT));
    assert_eq!(confidence, 0.0, "a text layer past the stub bound keeps a score of zero");
    assert!(!selected);
}
