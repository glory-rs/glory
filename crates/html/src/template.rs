//! Finite template slot kinds and shared HTML profile validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Attribute(&'static str),
    Click,
    Children,
    Range,
    Class(&'static str, bool),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub &'static str);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Error {}

pub(crate) fn attribute(name: &str) -> bool {
    matches!(
        name,
        "id" | "class" | "role" | "title" | "disabled" | "type"
    ) || name.starts_with("aria-")
}
pub(crate) fn class_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 128
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}
pub(crate) fn context(parent: Option<&str>, child: &str, in_button: bool) -> bool {
    matches!(
        child,
        "div" | "span" | "button" | "a" | "h1" | "table" | "tbody" | "tr" | "td"
    ) && !(in_button && matches!(child, "button" | "a"))
        && match parent {
            Some("table") => child == "tbody",
            Some("tbody") => child == "tr",
            Some("tr") => child == "td",
            Some("span" | "button" | "a" | "h1") => child == "span",
            _ => !matches!(child, "tbody" | "tr" | "td") || parent.is_none(),
        }
}
