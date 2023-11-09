use std::fmt::Display;
use std::fmt::Formatter;

#[non_exhaustive]
pub enum RegexError {
    Syntax(String),
    CompiledTooBig(usize),
}
impl Display for RegexError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RegexError::Syntax(s) => write!(f, "Syntax error: {}", s),
            RegexError::CompiledTooBig(n) => write!(f, "Compiled regex too big: {}", n),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regex(js_sys::RegExp);

impl Regex {
    pub fn new(re: &str) -> Result<Regex, RegexError> {
        Ok(Regex(js_sys::RegExp::new(re, "")))
    }

    pub fn as_str(&self) -> String {
        self.0.as_string().unwrap_or_default()
    }
    pub fn captures(&self, haystack: &str) -> Option<Captures> {
        self.0.exec(haystack).map(|arr| Captures(arr))
    }
}

pub struct Captures(js_sys::Array);

impl Captures {
    pub fn get(&self, index: usize) -> Option<String> {
        self.0.get(index as u32).as_string()
    }
    pub fn len(&self) -> usize {
        self.0.length() as usize
    }
}
