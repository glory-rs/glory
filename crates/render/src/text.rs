use std::borrow::Cow;

/// Canonical editable text: CRLF/CR become LF in multiline controls, line
/// breaks are removed from single-line host input, and NUL becomes U+FFFD.
/// Programmatic single-line values must reject line breaks before calling this.
/// Already canonical text is borrowed without an allocation.
pub fn normalize_text(value: &str, multiline: bool) -> Cow<'_, str> {
    if !value.contains(['\r', '\0']) && (multiline || !value.contains('\n')) {
        return Cow::Borrowed(value);
    }
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' if multiline => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                output.push('\n');
            }
            '\r' | '\n' if !multiline => {}
            '\0' => output.push('\u{fffd}'),
            ch => output.push(ch),
        }
    }
    Cow::Owned(output)
}
