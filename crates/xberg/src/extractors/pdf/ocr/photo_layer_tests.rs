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

/// The two existing text judgements (language plausibility and the recognition-noise ratio)
/// both abstain on a one-line layer, so neither can tell invented words from a real title.
/// The layer gate therefore cannot reuse them to reject a short invented layer. ~keep
#[test]
fn should_show_that_the_existing_judgements_abstain_on_a_one_line_layer() {
    use super::plausibility::{PlausibilityVerdict, evaluate_text_plausibility};
    use super::scoring::is_ocr_recognition_noise;

    let thresholds = OcrQualityThresholds::default();
    for line in ["Meeting Notes", "Qxzv Kjwq Zxcv Wqjx Vkzq Jqxz"] {
        assert_eq!(evaluate_text_plausibility(line, &thresholds), PlausibilityVerdict::NotEvaluated);
        assert!(!is_ocr_recognition_noise(line, &thresholds));
    }
}

/// A pipeline scores every stage on its OCR text. A text layer that a stage put before noisy OCR
/// text must not raise that stage above a better stage, so the winner is the one the base picks
/// without any layer. ~keep
#[cfg(all(feature = "pdf", feature = "ocr"))]
#[tokio::test]
#[serial_test::serial]
async fn should_pick_the_same_pipeline_stage_with_and_without_a_text_layer() {
    use super::pipeline::{PageOcrHints, run_ocr_pipeline_for_page};
    use crate::core::config::{OcrConfig, OcrPipelineConfig, OcrPipelineStage};
    use crate::plugins::{ConfidenceSemantics, OcrBackend, OcrBackendType, Plugin};
    use crate::types::ExtractedDocument;
    use std::sync::Arc;

    const NOISY: &str = "layer-test-noisy-stage";
    const PLAIN: &str = "layer-test-plain-stage";
    const NOISY_TEXT: &str = "A B C D E F G H I J K L M N O P Q R S T U V W X Y Z A B C D";
    const PLAIN_TEXT: &str = "Total due on receipt";
    const LAYER: &str = "The committee met on Tuesday to review the annual budget. Members agreed that the proposal \
        was sound and asked the treasurer to prepare the final report. The next meeting takes place in the spring.";

    struct StageBackend {
        name: &'static str,
        text: &'static str,
        noisy: bool,
    }

    #[async_trait::async_trait]
    impl OcrBackend for StageBackend {
        fn backend_type(&self) -> OcrBackendType {
            OcrBackendType::Custom
        }
        fn supports_language(&self, _: &str) -> bool {
            true
        }
        async fn process_image(&self, _: &[u8], _: &OcrConfig) -> crate::Result<ExtractedDocument> {
            let mut metadata = crate::types::Metadata::default();
            if self.noisy {
                metadata
                    .additional
                    .insert("mean_text_conf".into(), serde_json::json!(18.0));
            }
            Ok(ExtractedDocument {
                content: self.text.to_string(),
                metadata,
                ..Default::default()
            })
        }
        fn confidence_semantics(&self) -> ConfidenceSemantics {
            ConfidenceSemantics::Legibility { scale_max: 100.0 }
        }
    }

    impl Plugin for StageBackend {
        fn name(&self) -> &str {
            self.name
        }
        fn version(&self) -> String {
            "1.0.0".to_string()
        }
        fn initialize(&self) -> crate::Result<()> {
            Ok(())
        }
        fn shutdown(&self) -> crate::Result<()> {
            Ok(())
        }
    }

    crate::plugins::register_ocr_backend(Arc::new(StageBackend { name: NOISY, text: NOISY_TEXT, noisy: true })).unwrap();
    crate::plugins::register_ocr_backend(Arc::new(StageBackend { name: PLAIN, text: PLAIN_TEXT, noisy: false })).unwrap();

    let stage = |backend: &str, priority: u32| OcrPipelineStage {
        backend: backend.to_string(),
        priority,
        language: None,
        tesseract_config: None,
        paddle_ocr_config: None,
        paddle_ocr_settings: None,
        vlm_config: None,
        backend_options: None,
    };
    let pipeline = OcrPipelineConfig {
        stages: vec![stage(NOISY, 120), stage(PLAIN, 100)],
        quality_thresholds: crate::core::config::OcrQualityThresholds {
            pipeline_min_quality: 0.99,
            ..Default::default()
        },
    };
    let config = ExtractionConfig {
        use_cache: false,
        ocr: Some(OcrConfig {
            pipeline: Some(pipeline.clone()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let images = [image::DynamicImage::new_rgb8(16, 16)];
    let hints = |layers: Option<ahash::AHashMap<u32, String>>| PageOcrHints {
        source_dpi: None,
        known_full_page_scan: false,
        single_block_pages: None,
        native_page_layers: layers.map(Arc::new),
        layer_prefix_sink: None,
    };
    let run = |hints: PageOcrHints| {
        let (config, pipeline, images) = (&config, &pipeline, &images);
        async move {
            run_ocr_pipeline_for_page(
                None,
                Some(images),
                #[cfg(feature = "layout-detection")]
                None,
                config,
                pipeline,
                None,
                0,
                true,
                None,
                Some(hints),
                0,
                None,
            )
            .await
            .map(|output| output.0)
            .expect("the pipeline must return a stage result")
        }
    };

    let without_layer = run(hints(None)).await;
    let with_layer = run(hints(Some(ahash::AHashMap::from([(1, LAYER.to_string())])))).await;
    crate::plugins::unregister_ocr_backend(NOISY).unwrap();
    crate::plugins::unregister_ocr_backend(PLAIN).unwrap();

    assert_eq!(without_layer, PLAIN_TEXT, "without a layer the plain stage scores higher");
    assert_eq!(with_layer, PLAIN_TEXT, "a layer must not change which stage wins");
}

#[test]
fn should_remove_only_the_layer_prefixes_a_stage_added() {
    use super::pipeline::text_without_layer_prefixes;

    let text = "Budget 2026\n\nocr line\nBudget 2026";
    assert_eq!(text_without_layer_prefixes(text, &["Budget 2026".to_string()]), "ocr line\nBudget 2026");
    assert_eq!(text_without_layer_prefixes(text, &[]), text);
}
