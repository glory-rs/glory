//! Fluent text localization with explicit, application-local language sources.
//!
//! Catalogs contain immutable resources, not application language state. Share
//! one `Rc<Catalog>` between apps; each [`I18n`] independently reads a static
//! language, Cage, or Bond. Ordinary formatting does not install a subscription.
//! [`I18n::message`] explicitly creates an Owner-owned derived value.
//!
//! There is no filesystem/browser discovery, global mutable language, renderer,
//! HTML interpretation, or implicit observer cache. Fluent output is plain text.

#![doc = include_str!("../README.md")]

use fluent_bundle::{FluentBundle, FluentResource};
use fluent_syntax::ast;
use glory_core::{AccessError, Bond, Lotus, Owner};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    rc::Rc,
};

pub use fluent_bundle::{FluentArgs as Args, FluentValue as Value};
pub use unic_langid::LanguageIdentifier as Locale;

/// Localization failures remain values; the application chooses its display policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidLocale(String),
    DuplicateLocale(String),
    MissingFallback(String),
    Parse {
        locale: String,
        errors: Vec<String>,
    },
    DuplicateEntry {
        locale: String,
        errors: Vec<String>,
    },
    UnknownLocale(String),
    MissingMessage {
        locale: String,
        key: String,
    },
    Format {
        locale: String,
        key: String,
        errors: Vec<String>,
    },
    Access(AccessError),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<AccessError> for Error {
    fn from(error: AccessError) -> Self {
        Self::Access(error)
    }
}

/// A catalog completeness issue, suitable for build/release gates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranslationIssue {
    Missing {
        locale: String,
        key: String,
    },
    Unexpected {
        locale: String,
        key: String,
    },
    Variables {
        locale: String,
        key: String,
        expected: Vec<String>,
        actual: Vec<String>,
    },
}

type Signature = BTreeMap<String, BTreeSet<String>>;
struct Language {
    id: Locale,
    bundle: FluentBundle<FluentResource>,
    signature: Signature,
}

/// A finite collection of parsed Fluent resources. Construction is transactional:
/// invalid resources or duplicate definitions never produce a usable catalog.
pub struct Catalog {
    fallback: Locale,
    languages: BTreeMap<String, Language>,
}

impl Catalog {
    /// Each locale has one source; callers can join several embedded `.ftl`
    /// files before construction. No I/O or parsing occurs during formatting.
    /// Fluent's bidirectional isolation is enabled, including for arguments.
    pub fn new<I, L, S>(fallback: &str, sources: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = (L, S)>,
        L: AsRef<str>,
        S: Into<String>,
    {
        let fallback = parse_locale(fallback)?;
        let mut languages = BTreeMap::new();
        for (locale, source) in sources {
            let id = parse_locale(locale.as_ref())?;
            let key = id.to_string();
            if languages.contains_key(&key) {
                return Err(Error::DuplicateLocale(key));
            }
            let resource =
                FluentResource::try_new(source.into()).map_err(|(_, errors)| Error::Parse {
                    locale: key.clone(),
                    errors: errors.iter().map(ToString::to_string).collect(),
                })?;
            let signature = signature(&resource);
            let mut bundle = FluentBundle::new(vec![id.clone()]);
            bundle
                .add_resource(resource)
                .map_err(|errors| Error::DuplicateEntry {
                    locale: key.clone(),
                    errors: errors.iter().map(ToString::to_string).collect(),
                })?;
            languages.insert(
                key,
                Language {
                    id,
                    bundle,
                    signature,
                },
            );
        }
        if !languages.contains_key(&fallback.to_string()) {
            return Err(Error::MissingFallback(fallback.to_string()));
        }
        Ok(Self {
            fallback,
            languages,
        })
    }

    pub fn fallback(&self) -> &Locale {
        &self.fallback
    }
    pub fn locales(&self) -> impl Iterator<Item = &Locale> {
        self.languages.values().map(|language| &language.id)
    }

    /// Format in the requested language, then the configured fallback only if
    /// the language/message/attribute is absent. Resolver errors are reported,
    /// never hidden by fallback. Keys may be `message` or `message.attribute`.
    pub fn format(
        &self,
        locale: &Locale,
        key: &str,
        args: Option<&Args<'_>>,
    ) -> Result<String, Error> {
        match self.format_exact(locale, key, args) {
            Err(Error::UnknownLocale(_) | Error::MissingMessage { .. })
                if locale != &self.fallback =>
            {
                self.format_exact(&self.fallback, key, args)
            }
            result => result,
        }
    }

    /// Strict formatting, useful when a locale must be complete (for example,
    /// prerendering a published translated document).
    pub fn format_exact(
        &self,
        locale: &Locale,
        key: &str,
        args: Option<&Args<'_>>,
    ) -> Result<String, Error> {
        let locale = locale.to_string();
        let language = self
            .languages
            .get(&locale)
            .ok_or_else(|| Error::UnknownLocale(locale.clone()))?;
        let (id, attribute) = key
            .split_once('.')
            .map_or((key, None), |(id, attr)| (id, Some(attr)));
        let missing = || Error::MissingMessage {
            locale: locale.clone(),
            key: key.to_owned(),
        };
        let message = language.bundle.get_message(id).ok_or_else(missing)?;
        let pattern = match attribute {
            Some(attribute) => message
                .get_attribute(attribute)
                .map(|attribute| attribute.value()),
            None => message.value(),
        }
        .ok_or_else(missing)?;
        let mut errors = Vec::new();
        let result = language.bundle.format_pattern(pattern, args, &mut errors);
        if errors.is_empty() {
            Ok(result.into_owned())
        } else {
            Err(Error::Format {
                locale,
                key: key.to_owned(),
                errors: errors.iter().map(ToString::to_string).collect(),
            })
        }
    }

    /// Compare message/term values, attributes and directly referenced variable
    /// names against the fallback. Plural variants and word order may differ.
    /// This is a schema check, not a proof that all possible arguments resolve;
    /// `format_exact` still reports missing references, cycles and bad arguments.
    pub fn validate_complete(&self) -> Vec<TranslationIssue> {
        let reference = &self.languages[&self.fallback.to_string()].signature;
        let mut issues = Vec::new();
        for (locale, language) in &self.languages {
            for (key, expected) in reference {
                match language.signature.get(key) {
                    None => issues.push(TranslationIssue::Missing {
                        locale: locale.clone(),
                        key: key.clone(),
                    }),
                    Some(actual) if actual != expected => {
                        issues.push(TranslationIssue::Variables {
                            locale: locale.clone(),
                            key: key.clone(),
                            expected: expected.iter().cloned().collect(),
                            actual: actual.iter().cloned().collect(),
                        })
                    }
                    Some(_) => {}
                }
            }
            for key in language
                .signature
                .keys()
                .filter(|key| !reference.contains_key(*key))
            {
                issues.push(TranslationIssue::Unexpected {
                    locale: locale.clone(),
                    key: key.clone(),
                });
            }
        }
        issues
    }
}

/// Share a catalog while independently selecting a static or reactive language.
/// Cloning this context does not create reactive nodes or own an Owner.
#[derive(Clone)]
pub struct I18n {
    catalog: Rc<Catalog>,
    locale: Lotus<Locale>,
}

impl I18n {
    pub fn new(catalog: Rc<Catalog>, locale: impl Into<Lotus<Locale>>) -> Self {
        Self {
            catalog,
            locale: locale.into(),
        }
    }
    pub fn locale(&self) -> Result<Locale, Error> {
        Ok(self.locale.try_get()?)
    }
    pub fn catalog(&self) -> &Rc<Catalog> {
        &self.catalog
    }

    /// Reads the language immediately. It tracks only when called inside an
    /// existing Bond/Binding/Effect; a bare component-build read is not reactive.
    /// The reactive value borrow ends before Fluent runs.
    pub fn format(&self, key: &str, args: Option<&Args<'_>>) -> Result<String, Error> {
        let locale = self.locale()?;
        self.catalog.format(&locale, key, args)
    }

    /// Explicit Owner-owned message. Argument construction runs only when the
    /// Bond evaluates, under ordinary pure-computation rules. Reads in that
    /// closure and in the locale source are tracked. Nothing is cached by key.
    /// Applications must handle the returned message's formatting errors.
    pub fn message(
        &self,
        owner: &Owner,
        key: impl Into<String>,
        arguments: impl Fn() -> Args<'static> + 'static,
    ) -> Result<Bond<Result<String, Error>>, Error> {
        if !owner.is_alive() {
            return Err(AccessError::Disposed.into());
        }
        let runtime = match &self.locale {
            Lotus::Value(_) => None,
            Lotus::Cage(value) => Some(value.runtime()),
            Lotus::Bond(value) => Some(value.runtime()),
        };
        if runtime.is_some_and(|runtime| runtime != owner.runtime()) {
            return Err(AccessError::WrongRuntime.into());
        }
        // Validate a reactive source before retaining the user's argument closure.
        match &self.locale {
            Lotus::Cage(value) if !value.is_alive() => return Err(AccessError::Disposed.into()),
            Lotus::Bond(value) if !value.is_alive() => return Err(AccessError::Disposed.into()),
            _ => {}
        }
        let context = self.clone();
        let key = key.into();
        Ok(owner.bond(move || context.format(&key, Some(&arguments()))))
    }
}

fn parse_locale(value: &str) -> Result<Locale, Error> {
    value
        .parse()
        .map_err(|_| Error::InvalidLocale(value.to_owned()))
}

fn signature(resource: &FluentResource) -> Signature {
    let mut result = Signature::new();
    for entry in resource.entries() {
        let (id, value, attributes) = match entry {
            ast::Entry::Message(message) => (
                message.id.name.to_owned(),
                message.value.as_ref(),
                &message.attributes,
            ),
            ast::Entry::Term(term) => (
                format!("-{}", term.id.name),
                Some(&term.value),
                &term.attributes,
            ),
            _ => continue,
        };
        if let Some(value) = value {
            result.insert(id.clone(), pattern_variables(value));
        }
        for attribute in attributes {
            result.insert(
                format!("{id}.{}", attribute.id.name),
                pattern_variables(&attribute.value),
            );
        }
    }
    result
}

fn pattern_variables(pattern: &ast::Pattern<&str>) -> BTreeSet<String> {
    let mut variables = BTreeSet::new();
    visit_pattern(pattern, &mut variables);
    variables
}
fn visit_pattern(pattern: &ast::Pattern<&str>, variables: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            visit_expression(expression, variables);
        }
    }
}
fn visit_expression(expression: &ast::Expression<&str>, variables: &mut BTreeSet<String>) {
    match expression {
        ast::Expression::Inline(inline) => visit_inline(inline, variables),
        ast::Expression::Select { selector, variants } => {
            visit_inline(selector, variables);
            for variant in variants {
                visit_pattern(&variant.value, variables);
            }
        }
    }
}
fn visit_arguments(arguments: &ast::CallArguments<&str>, variables: &mut BTreeSet<String>) {
    for argument in &arguments.positional {
        visit_inline(argument, variables);
    }
    for argument in &arguments.named {
        visit_inline(&argument.value, variables);
    }
}
fn visit_inline(expression: &ast::InlineExpression<&str>, variables: &mut BTreeSet<String>) {
    match expression {
        ast::InlineExpression::VariableReference { id } => {
            variables.insert(id.name.to_owned());
        }
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            visit_arguments(arguments, variables)
        }
        ast::InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => visit_arguments(arguments, variables),
        ast::InlineExpression::Placeable { expression } => visit_expression(expression, variables),
        _ => {}
    }
}
