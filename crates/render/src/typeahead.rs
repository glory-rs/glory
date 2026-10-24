//! A host reports one printable Unicode scalar and an explicit monotonic clock.
//! The application owns the bounded query, timeout and matching policy.
use crate::Error;

pub const MAX_TYPEAHEAD_MILLIS: u64 = 9_007_199_254_740_991;

pub fn validate_typeahead(text: &str, at_millis: u64) -> Result<(), Error> {
    let mut chars = text.chars();
    if at_millis > MAX_TYPEAHEAD_MILLIS
        || !chars
            .next()
            .is_some_and(|character| !character.is_control() && !character.is_whitespace())
        || chars.next().is_some()
    {
        return Err(Error::InvalidEvent(
            "typeahead requires one printable nonspace scalar and a finite clock",
        ));
    }
    Ok(())
}
