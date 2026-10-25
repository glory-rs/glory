# glory-i18n

Opt-in Fluent localization for Glory. Enable the facade's `i18n` feature and
use `glory::i18n`, or depend on `glory-i18n` directly. Core, semantic rendering,
HTML and SSR do not acquire this dependency by default.

```rust
use glory_core::Runtime;
use glory_i18n::{Args, Catalog, I18n, Locale};
use std::rc::Rc;

let runtime = Runtime::new();
let owner = runtime.owner();
let catalog = Rc::new(Catalog::new("en", [
    ("en", "greeting = Hello, { $name }!"),
    ("zh-CN", "greeting = 你好，{ $name }！"),
]).unwrap());
assert!(catalog.validate_complete().is_empty());
let language = owner.cage("en".parse::<Locale>().unwrap());
let i18n = I18n::new(catalog, language);
let message = i18n.message(&owner, "greeting", || {
    let mut args = Args::new();
    args.set("name", "Glory");
    args
}).unwrap();
assert!(message.get().unwrap().contains("Hello"));
language.revise(|value| *value = "zh-CN".parse().unwrap());
runtime.flush().unwrap();
assert!(message.get().unwrap().contains("你好"));
```

Catalogs parse embedded resources once and may be shared with `Rc`. Each context
independently accepts a static `Locale`, `Cage<Locale>` or `Bond<Locale>`. There is
no process-wide current language. `format` is an ordinary immediate read; it
tracks only inside an existing Bond, binding or effect. `message` explicitly
allocates an Owner-owned Bond and tracks both the language and argument reads.
Dropping the Owner invalidates its handles and releases its captures. Contexts
retain no Owner or implicit cache of message keys or visited components.

`Catalog::format` falls back only for a missing language, message or attribute.
`format_exact` never falls back. Syntax, duplicate definitions, missing arguments,
unresolved references and cycles return structured errors; the caller chooses
the display/error policy. Fluent's directional isolation is enabled. Output is
plain text and must never be interpreted as trusted HTML.

`validate_complete` compares message and term values, attributes, and directly
referenced variable names with the fallback. It permits different plural forms
and word order. It is a translation schema gate, not a proof of every Fluent
resolution path; exercise parameterized messages with representative arguments.

Routing, browser preferences, persistence, document metadata, HTTP negotiation,
resource loading and translation extraction belong to the application. For SSR,
choose the language from request/application data and give hydration the same
initial language before constructing its tree.
