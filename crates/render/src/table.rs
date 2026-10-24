//! Finite read-only table coordinates and explicit header associations.
use crate::{DescriptorView, DocumentPart, Error, Props, Role, SemanticId};

pub const MAX_CELL_HEADERS: usize = 32;
pub const MAX_TABLE_SPAN: u16 = 1000;
/// Current ordering of an explicitly sortable column header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    None,
    Ascending,
    Descending,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableMetadata {
    Table {
        rows: Option<u32>,
        columns: Option<u16>,
    },
    Row {
        index: u32,
    },
    Cell {
        row: u32,
        column: u16,
        row_span: u16,
        column_span: u16,
        headers: Vec<SemanticId>,
    },
}
impl TableMetadata {
    pub(crate) fn validate(&self, descriptor: DescriptorView<'_>) -> Result<(), Error> {
        let part = descriptor.document_part();
        let role = descriptor.role;
        let structural = matches!(descriptor.props, Props::View);
        let valid = match self {
            Self::Table { rows, columns } => {
                structural
                    && (part == Some(DocumentPart::Table)
                        || matches!(role, Some(Role::Table | Role::Grid)))
                    && rows.is_none_or(|value| value < u32::MAX)
                    && columns.is_none_or(|value| value > 0)
            }
            Self::Row { index } => {
                structural
                    && (part == Some(DocumentPart::TableRow) || role == Some(Role::Row))
                    && *index < u32::MAX
            }
            Self::Cell {
                row,
                column,
                row_span,
                column_span,
                headers,
            } => {
                structural
                    && (matches!(
                        part,
                        Some(
                            DocumentPart::TableCell
                                | DocumentPart::ColumnHeader
                                | DocumentPart::RowHeader
                        )
                    ) || matches!(role, Some(Role::Cell | Role::ColumnHeader | Role::GridCell)))
                    && (1..=MAX_TABLE_SPAN).contains(row_span)
                    && (1..=MAX_TABLE_SPAN).contains(column_span)
                    && row.checked_add(u32::from(*row_span)).is_some()
                    && column.checked_add(*column_span).is_some()
                    && headers.len() <= MAX_CELL_HEADERS
                    && (part.is_some() || headers.is_empty())
                    && headers
                        .iter()
                        .enumerate()
                        .all(|(i, value)| !headers[..i].contains(value))
            }
        };
        if valid {
            Ok(())
        } else {
            Err(Error::InvalidProps(
                "table metadata conflicts with its document part or finite coordinates",
            ))
        }
    }
}
