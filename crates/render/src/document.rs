//! Finite document structure, independent of a host tag or rendering engine.
use crate::{Error, Props};

/// Structural document parts are fixed for a live node's lifetime. Groups are
/// transparent; lists contain items and description lists contain terms/values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentPart {
    Blockquote,
    OrderedList,
    UnorderedList,
    ListItem,
    DescriptionList,
    Term,
    Definition,
}

/// The parent is the nearest boxed parent, crossing only boxless Groups. A
/// detached Plan may leave its root context open until actual live placement.
pub(crate) fn validate_child(
    child: crate::DescriptorView<'_>,
    parent: Option<crate::DescriptorView<'_>>,
    detached: bool,
) -> Result<(), Error> {
    if matches!(child.props, Props::Group) {
        return Ok(());
    }
    let part = child.document_part();
    let parent = parent.and_then(|d| d.document_part());
    let valid = match (parent, part) {
        (
            Some(DocumentPart::OrderedList | DocumentPart::UnorderedList),
            Some(DocumentPart::ListItem),
        ) => true,
        (Some(DocumentPart::OrderedList | DocumentPart::UnorderedList), _) => false,
        (
            Some(DocumentPart::DescriptionList),
            Some(DocumentPart::Term | DocumentPart::Definition),
        ) => true,
        (Some(DocumentPart::DescriptionList), _) => false,
        (_, Some(DocumentPart::ListItem)) => detached,
        (_, Some(DocumentPart::Term | DocumentPart::Definition)) => detached,
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidPlacement(
            "document child conflicts with its nearest boxed parent",
        ))
    }
}
