//! Glyphs without a Unicode value in native PDF text.
//!
//! The native extractor keeps one U+FFFD for each Type0 glyph that has no Unicode value, so the
//! OCR routing gates can count the loss. A page that OCR does not replace must not show the
//! marker to the caller: this module removes it after the routing decision, keeps every byte
//! offset that points into the text correct, and builds the warning for a caller who has not
//! configured OCR. ~keep

use crate::core::config::ExtractionConfig;
use crate::types::internal::InternalDocument;
use crate::types::{PageBoundary, PageContent, ProcessingWarning, Table};

/// The marker the native extractor leaves for a glyph without a Unicode value.
pub(super) const LOST_GLYPH_MARKER: char = '\u{FFFD}';

/// Byte positions of the markers in a text, in order. Maps an offset in the text with the markers
/// to the same place in the text without them.
struct MarkerPositions(Vec<usize>);

impl MarkerPositions {
    fn of(text: &str) -> Option<Self> {
        let positions: Vec<usize> = text.match_indices(LOST_GLYPH_MARKER).map(|(at, _)| at).collect();
        (!positions.is_empty()).then_some(Self(positions))
    }

    fn map(&self, offset: usize) -> usize {
        offset - LOST_GLYPH_MARKER.len_utf8() * self.0.partition_point(|&at| at < offset)
    }
}

fn without_markers(text: &str) -> String {
    text.replace(LOST_GLYPH_MARKER, "")
}

/// Remove every marker from `text`. `boundaries` index `text` by byte and move with it.
/// Returns the number of markers removed.
pub(super) fn strip_text(text: &mut String, boundaries: Option<&mut Vec<PageBoundary>>) -> usize {
    let Some(markers) = MarkerPositions::of(text) else {
        return 0;
    };
    for boundary in boundaries.into_iter().flatten() {
        boundary.byte_start = markers.map(boundary.byte_start);
        boundary.byte_end = markers.map(boundary.byte_end);
    }
    *text = without_markers(text);
    markers.0.len()
}

/// Remove every marker from the text of the elements of `doc`, with their annotation offsets.
pub(super) fn strip_document(doc: &mut InternalDocument) -> usize {
    let mut removed = 0;
    for element in &mut doc.elements {
        let Some(markers) = MarkerPositions::of(&element.text) else {
            continue;
        };
        for annotation in &mut element.annotations {
            annotation.start = markers.map(annotation.start as usize) as u32;
            annotation.end = markers.map(annotation.end as usize) as u32;
        }
        element.text = without_markers(&element.text);
        removed += markers.0.len();
    }
    for table in &mut doc.tables {
        removed += strip_table(table);
    }
    removed
}

/// Remove every marker from a page: its text, its hierarchy blocks and its tables.
/// A page whose text becomes blank is marked blank.
pub(super) fn strip_page(page: &mut PageContent) -> usize {
    let mut removed = page.content.matches(LOST_GLYPH_MARKER).count();
    if removed > 0 {
        page.content = without_markers(&page.content);
        page.is_blank = Some(crate::extraction::blank_detection::is_page_text_blank(&page.content));
    }
    if let Some(hierarchy) = page.hierarchy.as_mut() {
        for block in &mut hierarchy.blocks {
            removed += block.text.matches(LOST_GLYPH_MARKER).count();
            block.text = without_markers(&block.text);
        }
    }
    for table in &mut page.tables {
        if table.markdown.contains(LOST_GLYPH_MARKER)
            || table.cells.iter().flatten().any(|c| c.contains(LOST_GLYPH_MARKER))
        {
            removed += strip_table(std::sync::Arc::make_mut(table));
        }
    }
    removed
}

/// Remove every marker from the cells and the Markdown of a table.
pub(super) fn strip_table(table: &mut Table) -> usize {
    let mut removed = table.markdown.matches(LOST_GLYPH_MARKER).count();
    table.markdown = without_markers(&table.markdown);
    for cell in table.cells.iter_mut().flatten() {
        removed += cell.matches(LOST_GLYPH_MARKER).count();
        *cell = without_markers(cell);
    }
    removed
}

/// The warning for a caller whose extraction lost glyphs and has no way to recover them: no `ocr`
/// block, and OCR not switched off. Other callers either asked for OCR or chose to go without.
pub(super) fn lost_glyph_warning(lost: usize, config: &ExtractionConfig) -> Option<ProcessingWarning> {
    if lost == 0 || config.ocr.is_some() || config.effective_disable_ocr() {
        return None;
    }
    Some(ProcessingWarning {
        source: std::borrow::Cow::Borrowed("pdf"),
        message: std::borrow::Cow::Borrowed(
            "Some glyphs in this PDF have no Unicode value, so their characters are missing from \
             the text. Set `ocr` to read the affected pages with OCR.",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::OcrConfig;
    use crate::extractors::pdf::PdfExtractor;
    use crate::plugins::InternalDocumentExtractor;
    use crate::types::document_structure::{AnnotationKind, TextAnnotation};
    use crate::types::internal::{ElementKind, InternalElement};

    const MARK: &str = "\u{FFFD}";

    #[test]
    fn strip_text_removes_markers_and_moves_page_boundaries() {
        // "ab" + marker (bytes 0..5), then "cd" + marker + "ef" (bytes 5..12).
        let mut text = format!("ab{MARK}cd{MARK}ef");
        let mut boundaries = vec![
            PageBoundary {
                byte_start: 0,
                byte_end: 5,
                page_number: 1,
            },
            PageBoundary {
                byte_start: 5,
                byte_end: 12,
                page_number: 2,
            },
        ];

        let removed = strip_text(&mut text, Some(&mut boundaries));

        assert_eq!(removed, 2);
        assert_eq!(text, "abcdef");
        assert_eq!(
            (boundaries[0].byte_start, boundaries[0].byte_end),
            (0, 2),
            "page 1 ends after its own two letters"
        );
        assert_eq!((boundaries[1].byte_start, boundaries[1].byte_end), (2, 6));
    }

    #[test]
    fn strip_text_leaves_text_without_markers_untouched() {
        let mut text = "plain \u{e9} text".to_string();
        let mut boundaries = vec![PageBoundary {
            byte_start: 0,
            byte_end: text.len(),
            page_number: 1,
        }];
        let before: Vec<(usize, usize, u32)> = boundaries
            .iter()
            .map(|b| (b.byte_start, b.byte_end, b.page_number))
            .collect();

        assert_eq!(strip_text(&mut text, Some(&mut boundaries)), 0);
        assert_eq!(text, "plain \u{e9} text");
        let after: Vec<(usize, usize, u32)> = boundaries
            .iter()
            .map(|b| (b.byte_start, b.byte_end, b.page_number))
            .collect();
        assert_eq!(after, before);
    }

    #[test]
    fn strip_document_moves_annotation_offsets_with_the_text() {
        let mut doc = InternalDocument::new("application/pdf");
        let mut element = InternalElement::text(ElementKind::Paragraph, format!("x{MARK}yz"), 0);
        // "yz" sits at bytes 4..6 while the marker is present.
        element.annotations = vec![TextAnnotation {
            start: 4,
            end: 6,
            kind: AnnotationKind::Bold,
        }];
        doc.elements.push(element);

        let removed = strip_document(&mut doc);

        assert_eq!(removed, 1);
        assert_eq!(doc.elements[0].text, "xyz");
        assert_eq!(
            (doc.elements[0].annotations[0].start, doc.elements[0].annotations[0].end),
            (1, 3)
        );
    }

    #[test]
    fn strip_page_marks_a_page_blank_when_only_markers_were_on_it() {
        let mut page = PageContent {
            page_number: 1,
            content: format!("{MARK}{MARK}\n{MARK}"),
            tables: Vec::new(),
            image_indices: Vec::new(),
            image_preprocessing: None,
            hierarchy: None,
            is_blank: Some(false),
            layout_regions: None,
            speaker_notes: None,
            section_name: None,
            sheet_name: None,
            ocr_confidence: None,
            native_content: None,
        };

        assert_eq!(strip_page(&mut page), 3);
        assert_eq!(page.content, "\n");
        assert_eq!(page.is_blank, Some(true));
    }

    #[test]
    fn strip_table_cleans_cells_and_markdown() {
        let mut table = Table {
            cells: vec![vec![format!("a{MARK}"), "b".to_string()]],
            markdown: format!("| a{MARK} | b |"),
            page_number: 1,
            bounding_box: None,
            ..Table::default()
        };

        assert_eq!(strip_table(&mut table), 2);
        assert_eq!(table.cells[0][0], "a");
        assert_eq!(table.markdown, "| a | b |");
    }

    #[test]
    fn warning_names_the_missing_ocr_setting_only_for_a_caller_without_ocr() {
        let bare = ExtractionConfig::default();
        let with_block = ExtractionConfig {
            ocr: Some(OcrConfig::default()),
            ..ExtractionConfig::default()
        };
        let switched_off = ExtractionConfig {
            disable_ocr: true,
            ..ExtractionConfig::default()
        };

        let warning = lost_glyph_warning(3, &bare).expect("a caller without OCR gets the warning");
        assert_eq!(warning.source, "pdf");
        assert!(
            warning.message.contains("no Unicode value"),
            "message: {}",
            warning.message
        );
        assert!(
            warning.message.contains("`ocr`"),
            "message names the setting: {}",
            warning.message
        );
        assert!(lost_glyph_warning(0, &bare).is_none(), "no lost glyph, no warning");
        assert!(lost_glyph_warning(3, &with_block).is_none(), "the caller asked for OCR");
        assert!(
            lost_glyph_warning(3, &switched_off).is_none(),
            "the caller switched OCR off"
        );
    }

    /// One page: a Helvetica line of real words, then a Type0 line whose ToUnicode maps code
    /// 0x0041 to "A" and code 0x0044 to NUL (a glyph without a Unicode value).
    fn build_type0_lost_glyph_pdf() -> Vec<u8> {
        let content = "BT /F1 12 Tf 72 700 Td (Real words stay.) Tj /F2 12 Tf 0 -20 Td <00410044> Tj ET\n";
        let to_unicode = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
            /CMapName /Test-UCS def /CMapType 2 def \
            1 begincodespacerange <0000> <FFFF> endcodespacerange \
            2 beginbfchar <0041> <0041> <0044> <0000> endbfchar endcmap end end";
        let objects: Vec<String> = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> >>"
                .to_string(),
            format!("<< /Length {} >>\nstream\n{content}endstream", content.len()),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_string(),
            "<< /Type /Font /Subtype /Type0 /BaseFont /AAAAAA+Test /Encoding /Identity-H \
             /DescendantFonts [7 0 R] /ToUnicode 9 0 R >>"
                .to_string(),
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /AAAAAA+Test \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /DW 600 /FontDescriptor 8 0 R /CIDToGIDMap /Identity >>"
                .to_string(),
            "<< /Type /FontDescriptor /FontName /AAAAAA+Test /Flags 4 /FontBBox [0 0 1000 1000] \
             /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 >>"
                .to_string(),
            format!(
                "<< /Length {} >>\nstream\n{to_unicode}\nendstream",
                to_unicode.len() + 1
            ),
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (index, body) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", index + 1).as_bytes());
        }
        let xref_pos = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f\r\n", objects.len() + 1).as_bytes());
        for offset in &offsets {
            pdf.extend_from_slice(format!("{offset:010} 00000 n\r\n").as_bytes());
        }
        pdf.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        pdf
    }

    async fn extract(config: &ExtractionConfig) -> crate::types::ExtractedDocument {
        let document = PdfExtractor::new()
            .extract_content(&build_type0_lost_glyph_pdf(), "application/pdf", config)
            .await
            .expect("the synthetic PDF extracts");
        crate::extraction::derive::derive_extraction_result(document, false, crate::core::config::OutputFormat::Plain)
    }

    /// The product path: a PDF with no `ocr` block and a Type0 glyph without Unicode.
    /// The caller sees the real words, no marker, and a warning that says how to recover the page.
    #[tokio::test]
    async fn extraction_without_ocr_hides_the_marker_and_warns() {
        let config = ExtractionConfig {
            ocr_near_empty_fallback: Some(false),
            ..ExtractionConfig::default()
        };

        let result = extract(&config).await;

        assert!(
            result.content.contains("Real words stay."),
            "content: {:?}",
            result.content
        );
        assert!(
            result.content.contains('A'),
            "the mapped Type0 glyph is read: {:?}",
            result.content
        );
        assert!(
            !result.content.contains(LOST_GLYPH_MARKER),
            "marker in content: {:?}",
            result.content
        );
        assert!(
            result
                .processing_warnings
                .iter()
                .any(|w| w.message.contains("no Unicode value")),
            "warnings: {:?}",
            result.processing_warnings
        );
    }

    /// Control: with OCR switched off the output keeps its base value, so no marker and no new warning.
    #[tokio::test]
    async fn extraction_with_ocr_off_hides_the_marker_without_a_warning() {
        let config = ExtractionConfig {
            disable_ocr: true,
            ..ExtractionConfig::default()
        };

        let result = extract(&config).await;

        assert!(
            result.content.contains("Real words stay."),
            "content: {:?}",
            result.content
        );
        assert!(
            !result.content.contains(LOST_GLYPH_MARKER),
            "marker in content: {:?}",
            result.content
        );
        assert!(
            !result
                .processing_warnings
                .iter()
                .any(|w| w.message.contains("no Unicode value")),
            "warnings: {:?}",
            result.processing_warnings
        );
    }
}
