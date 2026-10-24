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
    Table,
    TableCaption,
    TableHead,
    TableBody,
    TableFoot,
    TableRow,
    ColumnHeader,
    RowHeader,
    TableCell,
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
            Some(DocumentPart::Table),
            Some(
                DocumentPart::TableCaption
                | DocumentPart::TableHead
                | DocumentPart::TableBody
                | DocumentPart::TableFoot,
            ),
        ) => true,
        (Some(DocumentPart::Table), _) => false,
        (
            Some(DocumentPart::TableHead | DocumentPart::TableBody | DocumentPart::TableFoot),
            Some(DocumentPart::TableRow),
        ) => true,
        (Some(DocumentPart::TableHead | DocumentPart::TableBody | DocumentPart::TableFoot), _) => {
            false
        }
        (
            Some(DocumentPart::TableRow),
            Some(DocumentPart::ColumnHeader | DocumentPart::RowHeader | DocumentPart::TableCell),
        ) => true,
        (Some(DocumentPart::TableRow), _) => false,
        (Some(DocumentPart::TableCaption), _) => matches!(child.props, Props::Text(_)),
        (
            _,
            Some(
                DocumentPart::TableCaption
                | DocumentPart::TableHead
                | DocumentPart::TableBody
                | DocumentPart::TableFoot
                | DocumentPart::TableRow
                | DocumentPart::ColumnHeader
                | DocumentPart::RowHeader
                | DocumentPart::TableCell,
            ),
        ) => detached,

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
