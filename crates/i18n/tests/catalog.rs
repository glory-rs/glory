use glory_core::{AccessError, Runtime};
use glory_i18n::{Args, Catalog, Error, I18n, Locale, TranslationIssue};
use std::{cell::Cell, rc::Rc};

fn locale(value: &str) -> Locale {
    value.parse().unwrap()
}
fn plain(value: String) -> String {
    value.replace(['\u{2068}', '\u{2069}'], "")
}
fn catalog() -> Rc<Catalog> {
    Rc::new(Catalog::new("en", [
        ("en", "hello = Hello, { $name }!\nitems = { $count ->\n    [one] One item\n   *[other] { $count } items\n}\nonly = English fallback\nbutton = Button\n    .title = Open\nbroken = { missing }\ncycle = { cycle }\n"),
        ("zh-CN", "hello = 你好，{ $name }！\nitems = 共 { $count } 项\nbutton = 按钮\nbroken = { $absent }\n"),
    ]).unwrap())
}

#[test]
fn construction_rejects_invalid_and_duplicate_definitions() {
    assert!(matches!(
        Catalog::new("!", [("en", "a = A")]),
        Err(Error::InvalidLocale(_))
    ));
    assert!(matches!(
        Catalog::new("en", [("en", "a = {")]),
        Err(Error::Parse { .. })
    ));
    assert!(matches!(
        Catalog::new("en", [("en", "a = A\na = B")]),
        Err(Error::DuplicateEntry { .. })
    ));
    assert!(matches!(
        Catalog::new("en", [("en", "a = A"), ("en", "b = B")]),
        Err(Error::DuplicateLocale(_))
    ));
    assert!(matches!(
        Catalog::new("en", [("zh-CN", "a = A")]),
        Err(Error::MissingFallback(_))
    ));
}

#[test]
fn fallback_is_explicit_and_does_not_hide_resolver_errors() {
    let catalog = catalog();
    assert_eq!(
        catalog.format(&locale("zh-CN"), "only", None).unwrap(),
        "English fallback"
    );
    assert_eq!(
        catalog.format(&locale("fr"), "button.title", None).unwrap(),
        "Open"
    );
    assert!(matches!(
        catalog.format_exact(&locale("fr"), "only", None),
        Err(Error::UnknownLocale(_))
    ));
    assert!(matches!(
        catalog.format_exact(&locale("zh-CN"), "only", None),
        Err(Error::MissingMessage { .. })
    ));
    assert!(matches!(
        catalog.format_exact(&locale("en"), "button.absent", None),
        Err(Error::MissingMessage { .. })
    ));
    assert!(matches!(
        catalog.format(&locale("zh-CN"), "broken", None),
        Err(Error::Format { .. })
    ));
    assert!(matches!(
        catalog.format(&locale("en"), "cycle", None),
        Err(Error::Format { .. })
    ));
    assert!(matches!(
        catalog.format(&locale("en"), "hello", None),
        Err(Error::Format { .. })
    ));
}

#[test]
fn fluent_resolves_plural_rules_terms_attributes_and_isolates_arguments() {
    let catalog = catalog();
    for (count, expected) in [(1, "One item"), (2, "2 items")] {
        let mut args = Args::new();
        args.set("count", count);
        assert_eq!(
            plain(
                catalog
                    .format_exact(&locale("en"), "items", Some(&args))
                    .unwrap()
            ),
            expected
        );
        assert_eq!(
            plain(
                catalog
                    .format_exact(&locale("zh-CN"), "items", Some(&args))
                    .unwrap()
            ),
            format!("共 {count} 项")
        );
    }
    let mut args = Args::new();
    args.set("name", "世界");
    let text = catalog
        .format_exact(&locale("en"), "hello", Some(&args))
        .unwrap();
    assert!(text.contains("\u{2068}世界\u{2069}"));
    let terms = Catalog::new(
        "en",
        [(
            "en",
            "-brand = Glory\nintro = Welcome to { -brand }\n    .label = Enter\n",
        )],
    )
    .unwrap();
    assert_eq!(
        plain(terms.format_exact(&locale("en"), "intro", None).unwrap()),
        "Welcome to Glory"
    );
    assert_eq!(
        terms
            .format_exact(&locale("en"), "intro.label", None)
            .unwrap(),
        "Enter"
    );
}

#[test]
fn completeness_compares_values_terms_attributes_and_variable_names() {
    let good = Catalog::new("en", [
        ("en", "-brand = Glory\nitems = { $count ->\n    [one] Item\n   *[other] Items\n}\n    .label = { $count }\n"),
        ("zh-CN", "-brand = Glory\nitems = { $count } 项\n    .label = { $count }\n"),
    ]).unwrap();
    assert!(good.validate_complete().is_empty());
    let bad = Catalog::new(
        "en",
        [
            (
                "en",
                "-brand = Glory\nhello = { $name }\n    .label = Greeting\n",
            ),
            ("zh-CN", "hello = { $person }\nextra = 多余\n"),
        ],
    )
    .unwrap();
    let issues = bad.validate_complete();
    assert!(
        issues
            .iter()
            .any(|v| matches!(v, TranslationIssue::Missing { key, .. } if key == "-brand"))
    );
    assert!(
        issues
            .iter()
            .any(|v| matches!(v, TranslationIssue::Missing { key, .. } if key == "hello.label"))
    );
    assert!(
        issues
            .iter()
            .any(|v| matches!(v, TranslationIssue::Variables { key, .. } if key == "hello"))
    );
    assert!(
        issues
            .iter()
            .any(|v| matches!(v, TranslationIssue::Unexpected { key, .. } if key == "extra"))
    );
}

#[test]
fn messages_track_locale_and_arguments_and_remain_app_local() {
    let a = Runtime::new();
    let b = Runtime::new();
    let a_owner = a.owner();
    let b_owner = b.owner();
    let language = a_owner.cage(locale("en"));
    let name = a_owner.cage(String::from("Ada"));
    let shared = catalog();
    let first = I18n::new(shared.clone(), language);
    let second = I18n::new(shared, locale("en"));
    let message = first
        .message(&a_owner, "hello", move || {
            let mut args = Args::new();
            args.set("name", name.get());
            args
        })
        .unwrap();
    let independent = second.message(&b_owner, "only", Args::new).unwrap();
    assert_eq!(plain(message.get().unwrap()), "Hello, Ada!");
    name.revise(|name| *name = "Lin".into());
    language.revise(|id| *id = locale("zh-CN"));
    a.flush().unwrap();
    assert_eq!(plain(message.get().unwrap()), "你好，Lin！");
    assert_eq!(independent.get().unwrap(), "English fallback");
    // Derived locale sources work with the same explicit ownership rules.
    let derived = a_owner.bond(move || language.get());
    let context = I18n::new(first.catalog().clone(), derived);
    assert_eq!(
        context
            .message(&a_owner, "only", Args::new)
            .unwrap()
            .get()
            .unwrap(),
        "English fallback"
    );
    a_owner.dispose();
    assert_eq!(message.try_get(), Err(AccessError::Disposed));
    assert!(independent.is_alive());
}

#[test]
fn bare_formatting_creates_no_observer_and_owner_cleanup_releases_captures() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let language = owner.cage(locale("en"));
    let context = I18n::new(catalog(), language);
    let bare = context.format("only", None).unwrap();
    language.revise(|id| *id = locale("zh-CN"));
    runtime.flush().unwrap();
    assert_eq!(bare, "English fallback");
    let capture = Rc::new(Cell::new(0));
    let weak = Rc::downgrade(&capture);
    let message = context
        .message(&owner, "only", move || {
            capture.set(capture.get() + 1);
            Args::new()
        })
        .unwrap();
    assert!(weak.upgrade().is_some());
    assert_eq!(message.get().unwrap(), bare);
    owner.dispose();
    assert!(weak.upgrade().is_none());
}

#[test]
fn invalid_owners_and_sources_fail_before_arguments_execute() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source_owner = owner.child();
    let language = source_owner.cage(locale("en"));
    let context = I18n::new(catalog(), language);
    let other = Runtime::new();
    let foreign = other.owner();
    assert!(matches!(
        context.message(&foreign, "only", || panic!("must not execute")),
        Err(Error::Access(AccessError::WrongRuntime))
    ));
    source_owner.dispose();
    assert!(matches!(
        context.message(&owner, "only", || panic!("must not execute")),
        Err(Error::Access(AccessError::Disposed))
    ));
    owner.dispose();
    let context = I18n::new(catalog(), locale("en"));
    assert!(matches!(
        context.message(&owner, "only", || panic!("must not execute")),
        Err(Error::Access(AccessError::Disposed))
    ));
}
