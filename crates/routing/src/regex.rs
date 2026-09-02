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

/// Browser-side stand-in for the `regex` crate, backed by `RegExp`.
///
/// The source string is kept alongside the compiled `RegExp`: `RegExp` has no
/// accessor that returns it (`as_string()` on the object is always `None`), so
/// [`as_str`](Self::as_str) used to return `""` for every pattern — which made
/// `PartialEq` report *all* regex path wisps as equal on wasm.
#[derive(Debug, Clone)]
pub struct Regex {
    source: String,
    inner: js_sys::RegExp,
}

impl PartialEq for Regex {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}
impl Eq for Regex {}

impl Regex {
    pub fn new(re: &str) -> Result<Regex, RegexError> {
        // `RegExp::new` throws a `SyntaxError` for an invalid pattern, which
        // would trap the whole wasm module. Compile it defensively and report
        // the failure the way the native `regex` crate does.
        let compiled = compile(re).ok_or_else(|| RegexError::Syntax(re.to_owned()))?;
        Ok(Regex {
            source: re.to_owned(),
            inner: compiled,
        })
    }

    pub fn as_str(&self) -> String {
        self.source.clone()
    }
    pub fn captures(&self, haystack: &str) -> Option<Captures> {
        self.inner.exec(haystack).map(Captures)
    }
}

pub struct Captures(js_sys::Array<js_sys::JsString>);

impl Captures {
    pub fn get(&self, index: usize) -> Option<String> {
        self.0.get(index as u32).as_string()
    }
    pub fn len(&self) -> usize {
        self.0.length() as usize
    }
}

/// Compiles `re` through `new RegExp(re)`, returning `None` when the engine
/// rejects it. Uses a JS-side `try`/`catch` because a `SyntaxError` thrown
/// through `js_sys::RegExp::new` cannot be caught on the Rust side.
fn compile(re: &str) -> Option<js_sys::RegExp> {
    use wasm_bindgen::JsCast;

    let factory = js_sys::Function::new_with_args("source", "try { return new RegExp(source); } catch (_) { return null; }");
    let compiled = factory.call1(&wasm_bindgen::JsValue::NULL, &wasm_bindgen::JsValue::from_str(re)).ok()?;
    if compiled.is_null() || compiled.is_undefined() {
        return None;
    }
    compiled.dyn_into::<js_sys::RegExp>().ok()
}
