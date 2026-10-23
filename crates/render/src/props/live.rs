//! Field-only transactions for canonical descriptors stored in Memory. Raw
//! Descriptor::patch/patches retain their full construction-time validation.
use super::*;

fn exchange<T>(change: &mut Change<T>, target: &mut T, default: T) {
    match change {
        Change::Keep => {}
        Change::Set(value) => std::mem::swap(value, target),
        Change::Clear => *change = Change::Set(std::mem::replace(target, default)),
    }
}

impl Descriptor {
    /// Swap just the touched field into the inverse patch. Does not validate:
    /// the same operation restores a previously valid descriptor on rollback.
    pub(crate) fn swap_patch(&mut self, patch: &mut Patch) -> Result<(), Error> {
        match (patch, &mut self.props) {
            (Patch::Text(change), Props::Text(p)) => {
                exchange(change, &mut p.content, String::new())
            }
            (Patch::TextKind(change), Props::Text(p)) => {
                exchange(change, &mut p.kind, TextKind::default())
            }
            (Patch::Selectable(change), Props::Text(p)) => {
                exchange(change, &mut p.selectable, true)
            }
            (Patch::Label(change), Props::Button(p)) => {
                exchange(change, &mut p.label, String::new())
            }
            (Patch::Label(change), Props::Check(p)) => {
                exchange(change, &mut p.label, String::new())
            }
            (Patch::Label(change), Props::Link(p)) => exchange(change, &mut p.label, String::new()),
            (Patch::Enabled(change), Props::Button(p)) => exchange(change, &mut p.enabled, true),
            (Patch::Enabled(change), Props::Check(p)) => exchange(change, &mut p.enabled, true),
            (Patch::Enabled(change), Props::Input(p)) => exchange(change, &mut p.enabled, true),
            (Patch::Value { value, revision }, Props::Input(p)) => {
                if !matches!(value, Change::Keep) {
                    exchange(value, &mut p.value, String::new());
                    std::mem::swap(revision, &mut p.revision);
                }
            }
            (Patch::Purpose(change), Props::Input(p)) => {
                exchange(change, &mut p.purpose, Purpose::default())
            }
            (Patch::Multiline(change), Props::Input(p)) => {
                exchange(change, &mut p.multiline, false)
            }
            (Patch::Checked(change), Props::Check(p)) => exchange(change, &mut p.checked, false),
            (Patch::Source(change), Props::Image(p)) => {
                exchange(change, &mut p.source, String::new())
            }
            (Patch::Alternative(change), Props::Image(p)) => {
                exchange(change, &mut p.alternative, None)
            }
            (Patch::Target(change), Props::Link(p)) => {
                exchange(change, &mut p.target, String::new())
            }
            (Patch::Classes(change), _) => exchange(change, &mut self.classes, Vec::new()),
            (Patch::Style(change), _) => match change {
                Change::Keep => {}
                Change::Set(style) => std::mem::swap(style.as_mut(), &mut self.style),
                Change::Clear => *change = Change::Set(Box::new(std::mem::take(&mut self.style))),
            },
            (Patch::Role(change), _) => exchange(change, &mut self.role, None),
            (Patch::Name(change), _) => exchange(change, &mut self.name, None),
            (Patch::Focus(change), _) => exchange(change, &mut self.focus, FocusMode::default()),
            (Patch::Item(change), _) => exchange(change, &mut self.item, None),
            _ => {
                return Err(Error::InvalidProps(
                    "property does not belong to this component",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn patch_live(&mut self, inverse: &mut Patch) -> Result<(), Error> {
        let value = matches!(inverse, Patch::Value { value, .. } if !matches!(value, Change::Keep));
        let style = matches!(inverse, Patch::Style(change) if !matches!(change, Change::Keep));
        let classes = matches!(inverse, Patch::Classes(change) if !matches!(change, Change::Keep));
        self.swap_patch(inverse)?;
        let result = (|| {
            let multiline_changed = matches!((&self.props, &inverse), (Props::Input(input), Patch::Multiline(Change::Set(old))) if input.multiline != *old);
            if value || multiline_changed {
                self.normalize_input()?;
            }
            if value
                && let (
                    Props::Input(input),
                    Patch::Value {
                        value: Change::Set(old),
                        revision,
                    },
                ) = (&self.props, &inverse)
                && (input.revision < *revision
                    || input.revision == *revision && input.value != *old)
            {
                return Err(Error::InputRevision {
                    current: *revision,
                    incoming: input.revision,
                });
            }
            self.validate_fields(false, style, classes)
        })();
        if result.is_err() {
            // The inverse owns only built-in field values, never application
            // callbacks/options. Restoring it cannot run validation or I/O.
            self.swap_patch(inverse)
                .expect("same field restores its valid descriptor");
        }
        result
    }
}
