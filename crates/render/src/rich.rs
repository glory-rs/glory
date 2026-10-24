//! Finite rich editor values and observations; no DOM or editing engine types.
use crate::{Error, RichDocument, RichSelection};
use std::rc::Rc;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichCommand {
    Undo,
    Redo,
    Bold,
    Italic,
    Code,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichEditorProps {
    pub document: Rc<RichDocument>,
    pub enabled: bool,
    pub revision: u64,
}
impl Default for RichEditorProps {
    fn default() -> Self {
        Self {
            document: Rc::new(RichDocument::default()),
            enabled: true,
            revision: 0,
        }
    }
}
impl RichEditorProps {
    pub fn validate(&self) -> Result<(), Error> {
        self.document.validate()
    }
    pub(crate) fn validate_update(&self, previous: &Self) -> Result<(), Error> {
        self.validate()?;
        if self.revision < previous.revision
            || self.revision == previous.revision && self.document != previous.document
        {
            return Err(Error::InputRevision {
                current: previous.revision,
                incoming: self.revision,
            });
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichSnapshot {
    pub document: Rc<RichDocument>,
    pub selection: RichSelection,
    pub revision: u64,
    pub composing: bool,
}
impl RichSnapshot {
    pub fn validate(&self) -> Result<(), Error> {
        self.document.validate()?;
        self.document.validate_selection(self.selection)
    }
    pub(crate) fn validate_input(&self, props: &RichEditorProps) -> Result<(), Error> {
        self.validate()?;
        if self.revision > props.revision {
            return Err(Error::InputRevision {
                current: props.revision,
                incoming: self.revision,
            });
        }
        Ok(())
    }
}
