// Adapted from fixed Codeg aff1e6c (Apache-2.0).
// Source ranges and changes: integration/inventory/native-codex-content-port.json.
//! JavaScript's string semantics, for the places coaxi ports one of
//! codex-acp's helpers and has to read text exactly the way the adapter does
//! (`acp::service_error`, `parsers::codex_desktop_attachments`).

/// JavaScript's `\s`, which is also what `String.prototype.trim` strips: its
/// WhiteSpace and LineTerminator characters. Not `char::is_whitespace`, which
/// adds U+0085 and leaves out U+FEFF.
pub(crate) fn is_js_whitespace(c: char) -> bool {
    // U+2000..=U+200A are the typographic spaces (en quad through hair space).
    matches!(c, '\u{2000}'..='\u{200A}')
        || matches!(
            c,
            '\t' | '\n'
                | '\u{0B}'
                | '\u{0C}'
                | '\r'
                | ' '
                | '\u{A0}'
                | '\u{1680}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
        )
}

/// JavaScript's `s.trim().length === 0`.
pub(crate) fn is_js_blank(s: &str) -> bool {
    s.chars().all(is_js_whitespace)
}
