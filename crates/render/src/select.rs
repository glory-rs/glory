//! Finite intrinsic selection controls. Keys identify options; values are form data.
use std::{collections::HashSet, rc::Rc};

use crate::{Error, SemanticId};

pub const MAX_SELECT_OPTIONS: usize = 1024;
pub const MAX_SELECT_GROUPS: usize = 32;
pub const MAX_SELECT_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectOption {
    pub key: SemanticId,
    pub label: String,
    pub value: String,
    pub disabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectGroup {
    pub label: String,
    pub disabled: bool,
    pub options: Vec<SelectOption>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectEntry {
    Option(SelectOption),
    Group(SelectGroup),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectSelection {
    /// Unique keys in document order, independent of business option values.
    pub keys: Vec<SemanticId>,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectProps {
    /// Immutable declaration shared by live instances. Selection patches never
    /// clone labels, values or option declarations.
    pub entries: Rc<Vec<SelectEntry>>,
    pub selection: SelectSelection,
    pub multiple: bool,
    pub rows: Option<u16>,
    pub placeholder: String,
    pub enabled: bool,
}

impl Default for SelectProps {
    fn default() -> Self {
        Self {
            entries: Rc::default(),
            selection: SelectSelection::default(),
            multiple: false,
            rows: None,
            placeholder: String::new(),
            enabled: true,
        }
    }
}

impl SelectProps {
    pub fn options(&self) -> impl Iterator<Item = (&SelectOption, bool)> {
        self.entries.iter().flat_map(|entry| {
            let (options, disabled) = match entry {
                SelectEntry::Option(option) => (std::slice::from_ref(option), false),
                SelectEntry::Group(group) => (group.options.as_slice(), group.disabled),
            };
            options.iter().map(move |option| (option, disabled))
        })
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.entries.len() > MAX_SELECT_OPTIONS + MAX_SELECT_GROUPS
            || self.rows.is_some_and(|rows| !(1..=64).contains(&rows))
            || !text(&self.placeholder, 256)
        {
            return Err(Error::InvalidProps("invalid select configuration"));
        }
        let mut count = 0;
        let mut groups = 0;
        let mut bytes = self.placeholder.len();
        let mut keys = HashSet::new();
        for entry in self.entries.iter() {
            let options = match entry {
                SelectEntry::Option(option) => std::slice::from_ref(option),
                SelectEntry::Group(group) => {
                    groups += 1;
                    if groups > MAX_SELECT_GROUPS
                        || group.label.is_empty()
                        || !text(&group.label, 256)
                        || group.options.len() > MAX_SELECT_OPTIONS
                    {
                        return Err(Error::InvalidProps("invalid select group"));
                    }
                    bytes += group.label.len();
                    group.options.as_slice()
                }
            };
            for option in options {
                count += 1;
                if count > MAX_SELECT_OPTIONS
                    || option.key.as_str().len() > 64
                    || !option.key.as_str().is_ascii()
                    || !text(&option.label, 256)
                    || !text(&option.value, 4096)
                    || !keys.insert(&option.key)
                {
                    return Err(Error::InvalidProps("invalid select option"));
                }
                bytes += option.key.as_str().len() + option.label.len() + option.value.len();
                if bytes > MAX_SELECT_BYTES {
                    return Err(Error::InvalidProps(
                        "select declaration exceeds byte budget",
                    ));
                }
            }
        }
        self.validate_selection(&self.selection.keys)
    }

    pub fn validate_selection(&self, keys: &[SemanticId]) -> Result<(), Error> {
        if keys.len() > if self.multiple { MAX_SELECT_OPTIONS } else { 1 } {
            return Err(Error::InvalidProps("select selection exceeds cardinality"));
        }
        let mut next = keys.iter();
        let mut requested = next.next();
        // This walks only the bounded, mounted control declaration, never an
        // application data source or a history of previously visited options.
        for (option, _) in self.options() {
            if requested == Some(&option.key) {
                requested = next.next();
            }
        }
        if requested.is_some() {
            return Err(Error::InvalidProps(
                "select keys must be unique known keys in document order",
            ));
        }
        Ok(())
    }

    pub fn validate_user_selection(&self, keys: &[SemanticId]) -> Result<(), Error> {
        self.validate_selection(keys)?;
        for (option, group_disabled) in self.options() {
            if (option.disabled || group_disabled)
                && keys.contains(&option.key)
                && !self.selection.keys.contains(&option.key)
            {
                return Err(Error::InvalidEvent(
                    "disabled select option cannot be newly selected",
                ));
            }
        }
        Ok(())
    }
}

fn text(value: &str, limit: usize) -> bool {
    value.len() <= limit && !value.contains(['\0', '\r'])
}
