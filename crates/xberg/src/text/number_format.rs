//! Printing of a double the way a spreadsheet or presentation program shows it.

use std::borrow::Cow;

/// The most significant digits that Excel and PowerPoint keep for a number.
const GENERAL_FORMAT_DIGITS: usize = 15;

/// Format a float with at most the 15 significant digits Excel keeps.
///
/// A formula result is the double the calculation produced, with its binary noise past
/// those digits: `=0.1+0.2` is `0.30000000000000004` and `=100*1.19` is
/// `118.99999999999999`, which Excel shows as `0.3` and `119`. Rounding to 15 significant
/// digits before printing the shortest form drops that noise and keeps every digit a typed
/// value can have. Rust's `{}` already prints a whole number without a trailing decimal point.
/// Near `f64::MAX` the rounded form overflows to infinity, so the value is kept as it is.
pub(crate) fn format_excel_float(value: f64) -> String {
    if !value.is_finite() {
        return format!("{value}");
    }
    let rounded = format!("{value:.14e}")
        .parse::<f64>()
        .ok()
        .filter(|rounded| rounded.is_finite())
        .unwrap_or(value);
    format!("{rounded}")
}

/// Show the text of a stored number as the General format of a spreadsheet program shows it.
///
/// A file stores a double with 17 significant digits, so `1.1` is written `1.1000000000000001`.
/// Text with more than 15 significant digits is rewritten through [`format_excel_float`].
/// Any other text is returned as written: `42.0` and `1E-07` stay as they are, and so does text
/// that is no finite number (`#N/A`). ~keep
pub(crate) fn general_number_text(text: &str) -> Cow<'_, str> {
    if significant_digits(text) <= GENERAL_FORMAT_DIGITS {
        return Cow::Borrowed(text);
    }
    match text.parse::<f64>() {
        Ok(value) if value.is_finite() => Cow::Owned(format_excel_float(value)),
        _ => Cow::Borrowed(text),
    }
}

/// The count of digits in the mantissa of `text`, without leading and trailing zeros.
fn significant_digits(text: &str) -> usize {
    let mantissa = text.split(['e', 'E']).next().unwrap_or(text);
    let digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    let start = digits.iter().position(|d| *d != b'0');
    let end = digits.iter().rposition(|d| *d != b'0');
    match (start, end) {
        (Some(start), Some(end)) => end - start + 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_excel_float_drops_binary_noise() {
        assert_eq!(format_excel_float(0.1 + 0.2), "0.3");
        assert_eq!(format_excel_float(118.99999999999999), "119");
        assert_eq!(format_excel_float(42.0), "42");
    }

    #[test]
    fn long_digits_print_as_general_format() {
        assert_eq!(general_number_text("1.1000000000000001"), "1.1");
        assert_eq!(general_number_text("0.30000000000000004"), "0.3");
        assert_eq!(general_number_text("118.99999999999999"), "119");
        assert_eq!(general_number_text("-2.2000000000000002"), "-2.2");
        assert_eq!(general_number_text("3.3000000000000003E-2"), "0.033");
    }

    #[test]
    fn short_text_stays_as_written() {
        for text in [
            "42",
            "42.0",
            "1E-07",
            "-0.5",
            "0",
            "1.10",
            "123456789012345",
            "#N/A",
            "n/a",
            "",
        ] {
            assert_eq!(general_number_text(text), text);
            assert!(
                matches!(general_number_text(text), Cow::Borrowed(_)),
                "{text} is rewritten"
            );
        }
    }

    #[test]
    fn digit_count_ignores_sign_point_zeros_and_exponent() {
        assert_eq!(significant_digits("0.00012300"), 3);
        assert_eq!(significant_digits("-1.1000000000000001E+5"), 17);
        assert_eq!(significant_digits("1E-07"), 1);
        assert_eq!(significant_digits("0.0"), 0);
    }

    #[test]
    fn long_text_that_is_no_finite_number_stays() {
        assert_eq!(
            general_number_text("not a number at all, long"),
            "not a number at all, long"
        );
        assert_eq!(general_number_text("1e999999999999999999"), "1e999999999999999999");
    }
}
