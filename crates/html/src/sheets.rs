use glory_style::sheet::{Error, ErrorKind, Relation, State, Term, identifier};
use glory_style::{Css, Profile, Sheet};

/// Compile checked semantic selectors into the shared HTML mapping. Rule order,
/// duplicate declarations and class/state specificity are preserved. Layout and
/// cascade remain the consumer engine's responsibility.
pub fn sheet(sheet: &Sheet, profile: Profile) -> Result<String, Error> {
    if profile.css != Css::Html {
        return Err(Error {
            line: 1,
            column: 1,
            kind: ErrorKind::Target(profile.name),
        });
    }
    sheet.check(profile)?;
    let mut css = String::new();
    for rule in sheet.rules() {
        for (index, selector) in rule.selectors().iter().enumerate() {
            if index > 0 {
                css.push(',');
            }
            for compound in selector.compounds() {
                match compound.relation() {
                    Some(Relation::Child) => css.push('>'),
                    Some(Relation::Descendant) => css.push(' '),
                    None => {}
                }
                // Internal parts and user-owned document elements must not
                // become semantic selector targets. :where adds no specificity.
                css.push_str(":where([data-gly-kind])");
                for term in compound.terms() {
                    match term {
                        Term::Class(name) => {
                            css.push('.');
                            css.push_str(&identifier(name));
                        }
                        Term::State(state) => css.push_str(match state {
                            State::Hover => ":hover",
                            // Check's label owns the semantic class; only its
                            // internal input receives physical keyboard focus.
                            State::Focus => {
                                ":is(:focus,:where([data-gly-kind=check]):focus-within)"
                            }
                            State::Enabled => "[data-gly-enabled]",
                            State::Disabled => "[data-gly-disabled]",
                            State::Checked => "[data-gly-checked]",
                        }),
                    }
                }
            }
        }
        css.push('{');
        for declaration in rule.declarations() {
            css.push_str(&crate::style(declaration.style()));
        }
        css.push('}');
    }
    Ok(css)
}
