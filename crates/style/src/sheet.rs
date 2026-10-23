//! Bounded, fail-closed CSS input. Parsing creates engine-independent rules;
//! the selected adapter must additionally check its own inline support profile.
//! No cascade, geometry, I/O or reactive runtime lives here.
mod values;
use crate::{Profile, Style, Unsupported};
use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, ParseErrorKind, Parser, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, SourceLocation, StyleSheetParser,
    Token,
};

/// Bound selector matching depth as well as total sheet storage. Every compound
/// requires a class term; no accepted selector can exceed this many compounds.
pub const MAX_SELECTOR_TERMS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub bytes: usize,
    pub rules: usize,
    pub selectors: usize,
    pub terms: usize,
    pub declarations: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 1_048_576,
            rules: 4096,
            selectors: 8192,
            terms: 65_536,
            declarations: 32_768,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Syntax,
    Selector,
    AtRule(String),
    Property(String),
    Value(String),
    Limit(&'static str),
    Unsupported(Unsupported),
    Target(&'static str),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    /// One-based source line and UTF-8 byte column, as reported by cssparser.
    pub line: u32,
    pub column: u32,
    pub kind: ErrorKind,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSS {}:{}: {:?}", self.line, self.column, self.kind)
    }
}
impl std::error::Error for Error {}
fn error(location: SourceLocation, kind: ErrorKind) -> Error {
    Error {
        line: location.line + 1,
        column: location.column,
        kind,
    }
}
fn finish(err: ParseError<Error>, location: SourceLocation) -> Error {
    match err.kind {
        ParseErrorKind::Custom(err) => err,
        ParseErrorKind::Basic(_) => error(location, ErrorKind::Syntax),
    }
}
fn custom(input: &Parser<'_>, kind: ErrorKind) -> ParseError<Error> {
    ParseError::custom(error(input.current_source_location(), kind))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Hover,
    Focus,
    Enabled,
    Disabled,
    Checked,
}
impl State {
    pub const ALL: &'static [Self] = &[
        Self::Hover,
        Self::Focus,
        Self::Enabled,
        Self::Disabled,
        Self::Checked,
    ];
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Term {
    Class(String),
    State(State),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Child,
    Descendant,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compound {
    relation: Option<Relation>,
    terms: Vec<Term>,
}
impl Compound {
    pub fn relation(&self) -> Option<Relation> {
        self.relation
    }
    pub fn terms(&self) -> &[Term] {
        &self.terms
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selector(Vec<Compound>);
impl Selector {
    pub fn compounds(&self) -> &[Compound] {
        &self.0
    }
    /// Only classes and states contribute: no id/type selectors are accepted.
    pub fn specificity(&self) -> usize {
        self.0.iter().map(|c| c.terms.len()).sum()
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    style: Style,
    location: SourceLocation,
}
impl Declaration {
    pub fn style(&self) -> &Style {
        &self.style
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    selectors: Vec<Selector>,
    declarations: Vec<Declaration>,
}
impl Rule {
    pub fn selectors(&self) -> &[Selector] {
        &self.selectors
    }
    /// Original order, including duplicates. Shorthands may not be reordered.
    pub fn declarations(&self) -> &[Declaration] {
        &self.declarations
    }
}
/// Validated finite CSS rules. `css` is opt-in; parsing does not promise that a
/// particular engine supports every declaration. Adapters check their Profile
/// before compiling/installing a sheet. Rules and declarations retain source
/// order; matching, cascade and inheritance belong to the selected engine.
///
/// ```
/// use glory_style::{Sheet, Profile};
/// let sheet = Sheet::parse(".panel > .title { color:#123456; margin:0 }")?;
/// sheet.check(Profile::HTML)?;
/// assert_eq!(sheet.rules()[0].selectors()[0].specificity(), 2);
/// # Ok::<(), glory_style::sheet::Error>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sheet {
    rules: Vec<Rule>,
}
impl Sheet {
    pub const VERSION: u32 = 1;
    /// Longhand groups represented by the typed value contract. `overflow`
    /// additionally accepts the CSS shorthand for the two overflow axes.
    pub const PROPERTIES: &'static [crate::Property] = crate::Property::ALL;
    pub fn parse(css: &str) -> Result<Self, Error> {
        Self::parse_with(css, Limits::default())
    }
    pub fn parse_with(css: &str, limits: Limits) -> Result<Self, Error> {
        if css.len() > limits.bytes {
            return Err(error(
                SourceLocation { line: 0, column: 1 },
                ErrorKind::Limit("bytes"),
            ));
        }
        let mut parser = Parser::new(css);
        // StyleSheetParser intentionally ignores an initial @charset. Our
        // already-decoded finite input does not accept encoding directives.
        let start = parser.state();
        loop {
            match parser.next().cloned() {
                Ok(Token::CDO | Token::CDC) => continue,
                Ok(Token::AtKeyword(name)) if name.eq_ignore_ascii_case("charset") => {
                    return Err(error(
                        parser.current_source_location(),
                        ErrorKind::AtRule(name.to_string()),
                    ));
                }
                _ => break,
            }
        }
        parser.reset(&start);
        let mut reader = Reader {
            limits,
            rules: 0,
            selectors: 0,
            terms: 0,
            declarations: 0,
        };
        let mut rules = Vec::new();
        for rule in StyleSheetParser::new(&mut parser, &mut reader) {
            rules.push(rule.map_err(|(err, _, location)| finish(err, location))?);
        }
        Ok(Self { rules })
    }
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
    pub fn check(&self, profile: Profile) -> Result<(), Error> {
        for rule in &self.rules {
            for declaration in &rule.declarations {
                profile
                    .inline
                    .check(&declaration.style)
                    .map_err(|unsupported| {
                        error(declaration.location, ErrorKind::Unsupported(unsupported))
                    })?;
            }
        }
        Ok(())
    }
}

/// Escape a decoded CSS class identifier. Raw '<' is hex-escaped as well so
/// generated CSS is safe in an HTML style raw-text body without entity escaping.
pub fn identifier(name: &str) -> String {
    let mut value = String::new();
    cssparser::serialize_identifier(name, &mut value).expect("string write");
    value.replace('<', "\\3c ")
}
struct Reader {
    limits: Limits,
    rules: usize,
    selectors: usize,
    terms: usize,
    declarations: usize,
}
fn count<'i>(
    value: &mut usize,
    max: usize,
    name: &'static str,
    input: &Parser<'i>,
) -> Result<(), ParseError<Error>> {
    if *value >= max {
        return Err(custom(input, ErrorKind::Limit(name)));
    }
    *value += 1;
    Ok(())
}
impl<'i> AtRuleParser<'i> for Reader {
    type Prelude = ();
    type AtRule = Rule;
    type Error = Error;
    fn parse_prelude(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i>,
    ) -> Result<(), ParseError<Error>> {
        Err(custom(input, ErrorKind::AtRule(name.to_string())))
    }
}
impl<'i> QualifiedRuleParser<'i> for Reader {
    type Prelude = Vec<Selector>;
    type QualifiedRule = Rule;
    type Error = Error;
    fn parse_prelude(
        &mut self,
        input: &mut Parser<'i>,
    ) -> Result<Self::Prelude, ParseError<Error>> {
        count(&mut self.rules, self.limits.rules, "rules", input)?;
        input.parse_comma_separated(|input| self.selector(input))
    }
    fn parse_block(
        &mut self,
        selectors: Self::Prelude,
        _: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<Rule, ParseError<Error>> {
        let mut declarations = Vec::new();
        for declaration in RuleBodyParser::new(input, &mut Body(self)) {
            declarations.push(
                declaration
                    .map_err(|(err, _, location)| ParseError::custom(finish(err, location)))?,
            );
        }
        Ok(Rule {
            selectors,
            declarations,
        })
    }
}
impl Reader {
    fn selector<'i>(&mut self, input: &mut Parser<'i>) -> Result<Selector, ParseError<Error>> {
        count(
            &mut self.selectors,
            self.limits.selectors,
            "selectors",
            input,
        )?;
        let mut parts = Vec::new();
        let mut selector_terms = 0;
        let mut current = Compound {
            relation: None,
            terms: Vec::new(),
        };
        let mut gap = false;
        while let Ok(token) = input.next_including_whitespace().cloned() {
            match token {
                Token::WhiteSpace(_) => {
                    gap = !current.terms.is_empty();
                    continue;
                }
                Token::Delim('>') => {
                    complete(&current, input)?;
                    parts.push(current);
                    current = Compound {
                        relation: Some(Relation::Child),
                        terms: Vec::new(),
                    };
                    gap = false;
                    continue;
                }
                _ => {}
            }
            if gap {
                complete(&current, input)?;
                parts.push(current);
                current = Compound {
                    relation: Some(Relation::Descendant),
                    terms: Vec::new(),
                };
                gap = false;
            }
            let term = match token {
                Token::Delim('.') => {
                    let Token::Ident(name) = input.next_including_whitespace()?.clone() else {
                        return Err(custom(input, ErrorKind::Selector));
                    };
                    Term::Class(name.to_string())
                }
                Token::Colon => {
                    let Token::Ident(name) = input.next_including_whitespace()?.clone() else {
                        return Err(custom(input, ErrorKind::Selector));
                    };
                    Term::State(match name.to_ascii_lowercase().as_str() {
                        "hover" => State::Hover,
                        "focus" => State::Focus,
                        "enabled" => State::Enabled,
                        "disabled" => State::Disabled,
                        "checked" => State::Checked,
                        _ => return Err(custom(input, ErrorKind::Selector)),
                    })
                }
                _ => return Err(custom(input, ErrorKind::Selector)),
            };
            count(&mut self.terms, self.limits.terms, "terms", input)?;
            count(
                &mut selector_terms,
                MAX_SELECTOR_TERMS,
                "selector terms",
                input,
            )?;
            current.terms.push(term);
        }
        complete(&current, input)?;
        parts.push(current);
        Ok(Selector(parts))
    }
}
fn complete<'i>(compound: &Compound, input: &Parser<'i>) -> Result<(), ParseError<Error>> {
    if !compound.terms.iter().any(|t| matches!(t, Term::Class(_))) {
        return Err(custom(input, ErrorKind::Selector));
    }
    Ok(())
}
struct Body<'a>(&'a mut Reader);
impl<'i> DeclarationParser<'i> for Body<'_> {
    type Declaration = Declaration;
    type Error = Error;
    fn parse_value(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i>,
        _: &ParserState,
    ) -> Result<Declaration, ParseError<Error>> {
        count(
            &mut self.0.declarations,
            self.0.limits.declarations,
            "declarations",
            input,
        )?;
        let location = input.current_source_location();
        let style = values::parse(&name.to_ascii_lowercase(), input)?;
        style
            .validate()
            .map_err(|message| custom(input, ErrorKind::Value(message.into())))?;
        Ok(Declaration { style, location })
    }
}
impl<'i> AtRuleParser<'i> for Body<'_> {
    type Prelude = ();
    type AtRule = Declaration;
    type Error = Error;
}
impl<'i> QualifiedRuleParser<'i> for Body<'_> {
    type Prelude = ();
    type QualifiedRule = Declaration;
    type Error = Error;
}
impl<'i> RuleBodyItemParser<'i, Declaration, Error> for Body<'_> {
    fn parse_declarations(&self) -> bool {
        true
    }
    fn parse_qualified(&self) -> bool {
        false
    }
}
