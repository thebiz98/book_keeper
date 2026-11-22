/// Name hygiene shared by manual item entry and every import path.
///
/// - trims both ends
/// - turns NBSP / other Unicode spaces into a plain space and collapses runs to one
/// - drops zero-width characters and the BOM
/// - keeps the original letter case
pub fn clean_name(raw: &str) -> String {
    let filtered: String = raw
        .chars()
        .filter(|c| !matches!(c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'))
        .collect();
    filtered.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `clean_name` for optional free text: blank after cleaning becomes `None`.
pub fn clean_optional(raw: Option<&str>) -> Option<String> {
    raw.map(clean_name).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_collapses_and_strips_invisibles() {
        assert_eq!(clean_name(" HIGHLITER"), "HIGHLITER");
        assert_eq!(clean_name("TRASING PAPER "), "TRASING PAPER");
        assert_eq!(clean_name("  Pen \t  Blue\u{00A0}\u{00A0}Ink  "), "Pen Blue Ink");
        assert_eq!(clean_name("\u{FEFF}Note\u{200B}book"), "Notebook");
        assert_eq!(clean_name("Mixed Case"), "Mixed Case");
        assert_eq!(clean_name("   "), "");
    }

    #[test]
    fn optional_blank_is_none() {
        assert_eq!(clean_optional(Some("  ")), None);
        assert_eq!(clean_optional(Some(" a  b ")), Some("a b".to_string()));
        assert_eq!(clean_optional(None), None);
    }
}
