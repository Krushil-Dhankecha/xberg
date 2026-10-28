//! Writing systems and the OCR languages that read them.
//!
//! One table maps a configured language code to the scripts it is written in. The candle
//! script-plausibility filter (GH#1676) and the Tesseract language choice (GH#2300) both read it,
//! so the two cannot disagree about which language covers which script.

/// A writing system, coarse-grained: enough to tell which OCR language reads a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Script {
    Latin,
    Cyrillic,
    Greek,
    Arabic,
    Hebrew,
    Devanagari,
    Thai,
    Han,
    Kana,
    Hangul,
    Khmer,
}

/// Map a configured OCR language code (ISO 639-1, ISO 639-3, or a Tesseract code) to the
/// script(s) it is written in.
///
/// Returns `None` for a code this table does not recognize. The candle filter reads that as
/// "cannot judge script plausibility for this language list" and disables its script rule
/// rather than risk false positives against a language it cannot map. The Tesseract language
/// choice reads it as "this language does not cover the script".
pub(crate) fn scripts_for_language(lang: &str) -> Option<&'static [Script]> {
    match lang.trim().to_ascii_lowercase().as_str() {
        "en" | "eng" | "de" | "deu" | "ger" | "fr" | "fra" | "fre" | "es" | "spa" | "it" | "ita" | "pt" | "por"
        | "nl" | "nld" | "dut" | "pl" | "pol" | "sv" | "swe" | "tr" | "tur" | "vi" | "vie" | "id" | "ind" => {
            Some(&[Script::Latin])
        }
        "ru" | "rus" | "uk" | "ukr" | "bg" | "bul" | "sr" | "srp" => Some(&[Script::Cyrillic]),
        "el" | "ell" | "gre" => Some(&[Script::Greek]),
        "ar" | "ara" | "fa" | "fas" | "per" | "ur" | "urd" => Some(&[Script::Arabic]),
        "he" | "heb" | "yi" | "yid" => Some(&[Script::Hebrew]),
        "hi" | "hin" | "mr" | "mar" | "ne" | "nep" => Some(&[Script::Devanagari]),
        "th" | "tha" => Some(&[Script::Thai]),
        "km" | "khm" => Some(&[Script::Khmer]),
        "zh" | "zho" | "chi" | "chi_sim" | "chi_tra" => Some(&[Script::Han]),
        "ja" | "jpn" => Some(&[Script::Han, Script::Kana]),
        "ko" | "kor" => Some(&[Script::Hangul, Script::Han]),
        _ => None,
    }
}

/// The Tesseract language that reads a script, for a script no configured language covers.
///
/// Latin has no entry: it is the default and needs no help. Han maps to Simplified Chinese,
/// the model the measurements in GH#2300 used.
#[cfg(feature = "ocr-pipeline")]
pub(crate) fn tesseract_language_for_script(script: Script) -> Option<&'static str> {
    match script {
        Script::Latin => None,
        Script::Cyrillic => Some("rus"),
        Script::Greek => Some("ell"),
        Script::Arabic => Some("ara"),
        Script::Hebrew => Some("heb"),
        Script::Devanagari => Some("hin"),
        Script::Thai => Some("tha"),
        Script::Khmer => Some("khm"),
        Script::Han => Some("chi_sim"),
        Script::Kana => Some("jpn"),
        Script::Hangul => Some("kor"),
    }
}

/// `true` when one of `languages` reads `script`.
#[cfg(feature = "ocr-pipeline")]
pub(crate) fn languages_cover_script<S: AsRef<str>>(languages: &[S], script: Script) -> bool {
    languages
        .iter()
        .any(|lang| scripts_for_language(lang.as_ref()).is_some_and(|scripts| scripts.contains(&script)))
}

/// `true` when the language list is the `all` wildcard, which already reads every script.
#[cfg(feature = "ocr-pipeline")]
pub(crate) fn languages_are_wildcard<S: AsRef<str>>(languages: &[S]) -> bool {
    languages
        .iter()
        .any(|lang| matches!(lang.as_ref().trim().to_ascii_lowercase().as_str(), "all" | "*"))
}

#[cfg(all(test, feature = "ocr-pipeline"))]
mod tests {
    use super::*;

    #[test]
    fn a_language_covers_its_own_script_only() {
        assert!(languages_cover_script(&["khm"], Script::Khmer));
        assert!(languages_cover_script(&["eng", "chi_sim"], Script::Han));
        assert!(!languages_cover_script(&["eng"], Script::Khmer));
        assert!(!languages_cover_script(&["deu"], Script::Han));
    }

    #[test]
    fn an_unrecognized_language_covers_nothing() {
        assert!(!languages_cover_script(&["xyz"], Script::Khmer));
        assert!(scripts_for_language("xyz").is_none());
    }

    #[test]
    fn every_script_but_latin_has_an_ocr_language_that_covers_it() {
        for script in [
            Script::Cyrillic,
            Script::Greek,
            Script::Arabic,
            Script::Hebrew,
            Script::Devanagari,
            Script::Thai,
            Script::Khmer,
            Script::Han,
            Script::Kana,
            Script::Hangul,
        ] {
            let lang = tesseract_language_for_script(script).expect("a language for the script");
            assert!(languages_cover_script(&[lang], script), "{lang} must cover {script:?}");
        }
        assert_eq!(tesseract_language_for_script(Script::Latin), None);
    }

    #[test]
    fn the_wildcard_is_recognised_in_either_spelling() {
        assert!(languages_are_wildcard(&["eng", "ALL"]));
        assert!(languages_are_wildcard(&["*"]));
        assert!(!languages_are_wildcard(&["eng"]));
    }
}
