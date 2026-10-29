//! A page whose OCR text is judged recognition noise keeps its readable text layer first.

use super::document::{clean_native_page_layers, layer_lines_absent_from_ocr};
use super::scoring::native_layer_is_clean;
use crate::core::config::OcrQualityThresholds;
use crate::types::PageBoundary;

#[test]
fn should_call_a_short_readable_title_a_clean_layer() {
    assert!(native_layer_is_clean("Meeting Notes", &OcrQualityThresholds::default()));
}

#[test]
fn should_not_call_empty_or_symbol_only_text_a_clean_layer() {
    let thresholds = OcrQualityThresholds::default();
    assert!(!native_layer_is_clean("   \n ", &thresholds));
    assert!(!native_layer_is_clean("-- | ~ ..", &thresholds));
}

#[test]
fn should_skip_layer_lines_the_ocr_text_already_holds() {
    let ocr = "intro\nMEETING  notes!\nbody";
    assert_eq!(layer_lines_absent_from_ocr("Meeting Notes", ocr), None);
    assert_eq!(
        layer_lines_absent_from_ocr("Meeting Notes\nBudget 2026", ocr).as_deref(),
        Some("Budget 2026")
    );
}

#[test]
fn should_collect_layers_only_for_requested_pages_with_one_boundary() {
    let text = "Meeting Notes--|~..Agenda Items";
    let boundaries = [
        PageBoundary { byte_start: 0, byte_end: 13, page_number: 1 },
        PageBoundary { byte_start: 13, byte_end: 19, page_number: 2 },
        PageBoundary { byte_start: 19, byte_end: 31, page_number: 3 },
        PageBoundary { byte_start: 19, byte_end: 31, page_number: 3 },
    ];
    let thresholds = OcrQualityThresholds::default();
    let all = clean_native_page_layers(text, &boundaries, None, &thresholds);
    assert_eq!(all.get(&1).map(String::as_str), Some("Meeting Notes"));
    assert!(!all.contains_key(&2), "a symbol-only layer is not clean");
    assert!(!all.contains_key(&3), "a page with two boundaries is skipped");
    let only_two = clean_native_page_layers(text, &boundaries, Some(&std::collections::HashSet::from([2])), &thresholds);
    assert!(only_two.is_empty(), "page 1 is not requested and page 2 is not clean");
}

#[cfg(feature = "pdf")]
#[test]
fn should_put_the_layer_before_the_structured_page_and_shift_relationships() {
    use super::document::{flat_ocr_page_document, prepend_layer_to_page_document};
    use crate::types::internal::{Relationship, RelationshipKind, RelationshipTarget};

    let mut page = flat_ocr_page_document("first block\n\nsecond block");
    page.relationships.push(Relationship {
        source: 0,
        target: RelationshipTarget::Index(1),
        kind: RelationshipKind::InternalLink,
    });
    prepend_layer_to_page_document(&mut page, "Meeting Notes");
    let texts: Vec<&str> = page.elements.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(texts, ["Meeting Notes", "first block", "second block"]);
    assert_eq!(page.relationships[0].source, 1);
    assert!(matches!(page.relationships[0].target, RelationshipTarget::Index(2)));
}
