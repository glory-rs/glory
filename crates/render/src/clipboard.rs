//! Explicit text clipboard declarations and current request identity.
use crate::Error;

pub const MAX_CLIPBOARD_BYTES: usize = 65_536;

/// Programmatic clipboard source, not a promise that the host permits writing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClipboardText {
    pub text: String,
    pub revision: u64,
}
impl ClipboardText {
    pub fn validate(&self) -> Result<(), Error> {
        if self.text.len() > MAX_CLIPBOARD_BYTES {
            return Err(Error::InvalidProps("clipboard text exceeds 65536 UTF-8 bytes"));
        }
        if self.text.contains(['\r', '\0']) {
            return Err(Error::InvalidProps("clipboard text must use canonical LF and U+FFFD"));
        }
        Ok(())
    }
    pub(crate) fn validate_update(&self, previous: &Self) -> Result<(), Error> {
        self.validate()?;
        if self.revision < previous.revision
            || self.revision == previous.revision && self.text != previous.text
        {
            return Err(Error::InvalidProps("clipboard revisions must advance for changed text"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardOutcome {
    Written,
    Denied,
    Unavailable,
    Failed,
}

/// At most one pending request and one monotonic numeric watermark per live
/// NodeId. Source cancellation preserves the watermark; node retirement drops
/// it. No source value, request history or user callback is stored here.
#[derive(Clone, Debug, Default)]
pub(crate) struct Requests {
    last: u64,
    pending: Option<(u64, u64)>,
}
impl Requests {
    pub fn next(&self) -> Result<u64, Error> {
        self.last.checked_add(1).ok_or(Error::InvalidProps("clipboard request identity exhausted"))
    }
    pub fn admit(&mut self, request: u64, revision: u64) -> Result<(), Error> {
        if request <= self.last {
            return Err(Error::InvalidProps("clipboard request must strictly advance its live node watermark"));
        }
        self.last = request;
        self.pending = Some((request, revision));
        Ok(())
    }
    pub fn cancel(&mut self) { self.pending = None; }
    pub fn matches(&self, request: u64, revision: u64) -> bool { self.pending == Some((request, revision)) }
    pub fn observe(&mut self, request: u64, revision: u64) -> Result<bool, Error> {
        if request == 0 || request > self.last {
            return Err(Error::InvalidEvent("clipboard completion names an unadmitted request"));
        }
        if !self.matches(request, revision) { return Ok(false); }
        self.pending = None;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_and_revision_validation_precede_host_work() {
        let source = ClipboardText {text:"hello\n😀".into(),revision:9};
        source.validate().unwrap();source.validate_update(&source).unwrap();
        assert!(ClipboardText {text:"different".into(),..source.clone()}.validate_update(&source).is_err());
        assert!(ClipboardText {revision:8,..source.clone()}.validate_update(&source).is_err());
        for text in ["\r".into(),"\0".into(),"a".repeat(MAX_CLIPBOARD_BYTES+1)] {
            assert!(ClipboardText {text,revision:10}.validate().is_err());
        }
    }
    #[test]
    fn requests_are_first_only_monotonic_and_cancellation_preserves_aba_watermark() {
        let mut state=Requests::default();assert_eq!(state.next(),Ok(1));
        state.admit(1,0).unwrap();assert!(state.admit(1,0).is_err());
        assert_eq!(state.observe(2,0),Err(Error::InvalidEvent("clipboard completion names an unadmitted request")));
        assert_eq!(state.observe(1,0),Ok(true));assert_eq!(state.observe(1,0),Ok(false));
        state.admit(2,0).unwrap();state.cancel();assert!(!state.matches(2,0));assert!(state.admit(2,1).is_err());
        state.admit(3,0).unwrap();assert_eq!(state.observe(2,0),Ok(false));assert_eq!(state.observe(3,0),Ok(true));
        state.admit(u64::MAX,3).unwrap();assert!(state.next().is_err());assert_eq!(state.observe(u64::MAX,3),Ok(true));assert!(state.next().is_err());
    }
}
