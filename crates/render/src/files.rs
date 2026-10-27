//! Finite file-picker declarations and opaque document-owned read references.
//! A token is never a filename, URL, filesystem path, browser object or lease.
use crate::{Error, NodeId};
#[path = "file_preview.rs"]
mod preview;
pub use preview::*;
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAX_FILE_COUNT: usize = 32;
pub const MAX_FILE_FILTERS: usize = 32;
pub const MAX_FILE_FILTER_BYTES: usize = 2048;
pub const MAX_FILE_NAME_BYTES: usize = 1024;
pub const MAX_FILE_MIME_BYTES: usize = 127;
pub const MAX_FILE_METADATA_BYTES: usize = 32768;
pub const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_FILE_CHUNK_BYTES: u32 = 65536;
pub const MAX_FILE_READS: usize = 4;

/// Distinguishes independent consumers even when they share one RuntimeId.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileDocumentId(u64);
impl FileDocumentId {
    pub fn new() -> Result<Self, Error> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self::allocate(&NEXT)
    }
    fn allocate(counter: &AtomicU64) -> Result<Self, Error> {
        counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map(Self)
            .map_err(|_| Error::InvalidProps("file document identity exhausted"))
    }
    pub fn value(self) -> u64 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileToken {
    pub document: FileDocumentId,
    pub node: NodeId,
    pub revision: u64,
    pub serial: u64,
}
impl FileToken {
    pub fn validate(self) -> Result<(), Error> {
        if self.serial == 0 {
            Err(Error::InvalidProps(
                "file token requires a positive document-local serial",
            ))
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInfo {
    pub token: FileToken,
    /// Display metadata only. Never interpret or reopen this string as a path.
    pub name: String,
    pub media_type: String,
    pub size: u64,
}
fn mime(value: &str, wildcard: bool) -> bool {
    if value.len() > MAX_FILE_MIME_BYTES {
        return false;
    }
    let Some((major, minor)) = value.split_once('/') else {
        return false;
    };
    let token = |value: &str| {
        !value.is_empty()
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"!#$&-^_.+".contains(&b))
    };
    token(major) && (token(minor) || wildcard && minor == "*")
}
impl FileInfo {
    pub fn validate(&self) -> Result<(), Error> {
        self.token.validate()?;
        if self.name.is_empty()
            || self.name.len() > MAX_FILE_NAME_BYTES
            || self.name.contains(['\r', '\0'])
            || (!self.media_type.is_empty() && !mime(&self.media_type, false))
            || self.size > MAX_FILE_BYTES
        {
            Err(Error::InvalidProps(
                "file metadata exceeds its canonical name, MIME or byte domain",
            ))
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileFilter {
    Mime(String),
    Extension(String),
}
impl FileFilter {
    pub fn validate(&self) -> Result<(), Error> {
        let valid = match self {
            Self::Mime(value) => mime(value, true),
            Self::Extension(value) => {
                value.len() >= 2
                    && value.len() <= 32
                    && value.starts_with('.')
                    && value.bytes().skip(1).all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b".-_".contains(&b)
                    })
            }
        };
        if valid {
            Ok(())
        } else {
            Err(Error::InvalidProps(
                "file filter requires a lowercase MIME, type/*, or extension",
            ))
        }
    }
    pub fn text(&self) -> &str {
        match self {
            Self::Mime(value) | Self::Extension(value) => value,
        }
    }
    pub fn matches(&self, file: &FileInfo) -> bool {
        match self {
            Self::Extension(extension) => file.name.to_ascii_lowercase().ends_with(extension),
            Self::Mime(value) => {
                if let Some(prefix) = value.strip_suffix('*') {
                    file.media_type.starts_with(prefix)
                } else {
                    file.media_type == *value
                }
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileConfig {
    pub multiple: bool,
    pub drop: bool,
    pub preview: Option<FilePreviewLimits>,
    pub filters: Vec<FileFilter>,
    pub max_files: usize,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
}
impl Default for FileConfig {
    fn default() -> Self {
        Self {
            multiple: false,
            drop: false,
            preview: None,
            filters: Vec::new(),
            max_files: 1,
            max_file_bytes: 16 * 1024 * 1024,
            max_total_bytes: 32 * 1024 * 1024,
        }
    }
}
impl FileConfig {
    pub fn validate(&self) -> Result<(), Error> {
        if !(1..=MAX_FILE_COUNT).contains(&self.max_files)
            || !self.multiple && self.max_files != 1
            || self.max_file_bytes > MAX_FILE_BYTES
            || self.max_total_bytes > MAX_FILE_BYTES
            || self.filters.len() > MAX_FILE_FILTERS
        {
            return Err(Error::InvalidProps(
                "file picker configuration exceeds count, size or filter domain",
            ));
        }
        if let Some(preview) = self.preview {
            preview
                .validate()
                .map_err(|_| Error::InvalidProps("invalid finite file preview budget"))?;
        }
        let mut bytes = 0usize;
        for filter in &self.filters {
            filter.validate()?;
            bytes = bytes
                .checked_add(filter.text().len())
                .ok_or(Error::InvalidProps("file filter size overflow"))?;
        }
        if bytes > MAX_FILE_FILTER_BYTES {
            return Err(Error::InvalidProps("file filter strings exceed 2048 bytes"));
        }
        Ok(())
    }
    pub fn validate_files(&self, files: &[FileInfo]) -> Result<(), Error> {
        self.validate()?;
        if files.len() > self.max_files {
            return Err(Error::InvalidProps(
                "selected file count exceeds configuration",
            ));
        }
        let mut total = 0u64;
        let mut metadata = 0usize;
        for (index, file) in files.iter().enumerate() {
            file.validate()?;
            if file.size > self.max_file_bytes {
                return Err(Error::InvalidProps(
                    "selected file exceeds per-file byte budget",
                ));
            }
            total = total
                .checked_add(file.size)
                .ok_or(Error::InvalidProps("selected file sizes overflow"))?;
            metadata = metadata
                .checked_add(file.name.len() + file.media_type.len())
                .ok_or(Error::InvalidProps("selected file metadata size overflow"))?;
            if total > self.max_total_bytes || metadata > MAX_FILE_METADATA_BYTES {
                return Err(Error::InvalidProps(
                    "selected file total exceeds byte or metadata budget",
                ));
            }
            if !self.filters.is_empty() && !self.filters.iter().any(|filter| filter.matches(file)) {
                return Err(Error::InvalidProps(
                    "selected file does not match a declared filter",
                ));
            }
            if files[..index].iter().any(|other| other.token == file.token) {
                return Err(Error::InvalidProps("selected file tokens must be unique"));
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileProps {
    pub label: String,
    pub enabled: bool,
    pub configuration: FileConfig,
    pub revision: u64,
}
impl Default for FileProps {
    fn default() -> Self {
        Self {
            label: "Choose file".into(),
            enabled: true,
            configuration: FileConfig::default(),
            revision: 0,
        }
    }
}
impl FileProps {
    pub fn validate(&self) -> Result<(), Error> {
        self.configuration.validate()?;
        self.validate_label()
    }
    pub(crate) fn validate_label(&self) -> Result<(), Error> {
        if self.label.len() > 4096
            || self.label.contains(['\r', '\0'])
            || self.label.trim().is_empty()
        {
            return Err(Error::InvalidProps(
                "file button requires a nonempty canonical label of at most 4096 bytes",
            ));
        }
        Ok(())
    }
    pub fn validate_update(&self, previous: &Self) -> Result<(), Error> {
        self.validate()?;
        if self.revision < previous.revision
            || self.revision == previous.revision && self.configuration != previous.configuration
        {
            return Err(Error::InvalidProps(
                "file configuration changes must advance their revision",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileRejection {
    Count,
    Type,
    Bytes,
    Metadata,
    Busy,
    Unavailable,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileOutcome {
    Selected(Vec<FileInfo>),
    Cancelled,
    Rejected(FileRejection),
}
impl FileOutcome {
    pub fn validate(&self) -> Result<(), Error> {
        if let Self::Selected(files) = self {
            if files.is_empty() || files.len() > MAX_FILE_COUNT {
                return Err(Error::InvalidEvent("selected file event exceeds 32 files"));
            }
            let mut metadata = 0usize;
            let mut total = 0u64;
            for (index, file) in files.iter().enumerate() {
                file.validate()?;
                if let Some(first) = files.first()
                    && (file.token.document != first.token.document
                        || file.token.node != first.token.node
                        || file.token.revision != first.token.revision)
                {
                    return Err(Error::InvalidEvent(
                        "selected references require the same document, node and revision",
                    ));
                }
                metadata = metadata
                    .checked_add(file.name.len() + file.media_type.len())
                    .ok_or(Error::InvalidEvent("file metadata overflow"))?;
                total = total
                    .checked_add(file.size)
                    .ok_or(Error::InvalidEvent("file sizes overflow"))?;
                if metadata > MAX_FILE_METADATA_BYTES
                    || total > MAX_FILE_BYTES
                    || files[..index].iter().any(|other| other.token == file.token)
                {
                    return Err(Error::InvalidEvent(
                        "selected file event exceeds its unique metadata/byte domain",
                    ));
                }
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileRead {
    pub file: FileToken,
    pub offset: u64,
    pub length: u32,
}
impl FileRead {
    pub fn validate(self) -> Result<(), Error> {
        self.file.validate()?;
        if self.length > MAX_FILE_CHUNK_BYTES
            || self
                .offset
                .checked_add(u64::from(self.length))
                .is_none_or(|end| end > MAX_FILE_BYTES)
        {
            Err(Error::InvalidProps(
                "file reads require a finite range of at most 65536 bytes",
            ))
        } else {
            Ok(())
        }
    }
    pub fn validate_file(self, file: &FileInfo) -> Result<(), Error> {
        self.validate()?;
        file.validate()?;
        if self.file != file.token || self.offset + u64::from(self.length) > file.size {
            Err(Error::InvalidProps(
                "file read exceeds its live selected reference or size",
            ))
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileCapabilities {
    pub document: FileDocumentId,
    pub picker: bool,
    pub drop: bool,
    pub read: bool,
    pub preview: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileOrigin {
    Picker,
    Drop,
    /// Trusted adoption of a pre-WASM selection into a fresh consumer lifetime.
    Adopted,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEvent {
    pub document: FileDocumentId,
    pub revision: u64,
    pub request: Option<u64>,
    pub batch: u64,
    pub origin: FileOrigin,
    pub outcome: FileOutcome,
}
impl FileEvent {
    pub fn validate(&self) -> Result<(), Error> {
        if self.batch == 0
            || match self.origin {
                FileOrigin::Picker => self.request.is_none_or(|request| request == 0),
                FileOrigin::Drop | FileOrigin::Adopted => self.request.is_some(),
            }
        {
            return Err(Error::InvalidEvent(
                "file observations require a positive batch and origin-specific request",
            ));
        }
        self.outcome.validate()?;
        if let FileOutcome::Selected(files) = &self.outcome
            && files.iter().any(|file| {
                file.token.document != self.document || file.token.revision != self.revision
            })
        {
            return Err(Error::InvalidEvent(
                "file observation references must match document and revision",
            ));
        }
        Ok(())
    }
    pub fn validate_node(&self, node: NodeId, configuration: &FileProps) -> Result<(), Error> {
        self.validate()?;
        if self.revision > configuration.revision {
            return Err(Error::InvalidEvent(
                "file observation exceeds current configuration revision",
            ));
        }
        if let FileOutcome::Selected(files) = &self.outcome
            && files.iter().any(|file| file.token.node != node)
        {
            return Err(Error::InvalidEvent(
                "file observation references a different picker node",
            ));
        }
        // A retired declaration may have had different filters/drop policy.
        // Validate its finite payload and identity, then let admission discard it.
        if self.revision < configuration.revision {
            return Ok(());
        }
        if self.origin == FileOrigin::Drop && !configuration.configuration.drop {
            return Err(Error::InvalidEvent("file drop was not enabled"));
        }
        if let FileOutcome::Selected(files) = &self.outcome {
            configuration.configuration.validate_files(files)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePreview {
    pub image: FileImageInfo,
    pub file: FileToken,
    pub source: String,
}
impl FilePreview {
    /// Exact document-local resource address used by native file services.
    /// The address grants no ambient path or filesystem access.
    pub fn native_source(file: FileToken) -> String {
        format!(
            "file-preview://document-{}/{}/{}/{}/{}/{}",
            file.document.value(),
            file.node.runtime().value(),
            file.node.slot(),
            file.node.generation(),
            file.revision,
            file.serial
        )
    }
    pub fn validate(&self) -> Result<(), Error> {
        self.file.validate()?;
        self.image
            .validate(FilePreviewLimits::default())
            .map_err(|_| Error::InvalidProps("file preview image exceeds pixel domain"))?;
        if self.source.len() > 2048
            || !(self.source.starts_with("blob:") || self.source == Self::native_source(self.file))
            || self.source.contains(['\r', '\n', '\0'])
        {
            Err(Error::InvalidProps(
                "file preview requires a bounded document-owned resource URI",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodeIds, RuntimeId};
    fn reference() -> FileInfo {
        let mut ids = NodeIds::new(RuntimeId::new());
        FileInfo {
            token: FileToken {
                document: FileDocumentId::new().unwrap(),
                node: ids.allocate(),
                revision: 3,
                serial: 1,
            },
            name: "bounded.txt".into(),
            media_type: "text/plain".into(),
            size: 4,
        }
    }
    #[test]
    fn document_identity_does_not_alias_same_runtime_consumers_and_never_wraps() {
        let a = FileDocumentId::new().unwrap();
        let b = FileDocumentId::new().unwrap();
        assert_ne!(a, b);
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(
            FileDocumentId::allocate(&counter).unwrap().value(),
            u64::MAX - 1
        );
        assert!(FileDocumentId::allocate(&counter).is_err());
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }
    #[test]
    fn independent_filter_count_string_and_file_size_budgets() {
        let mut c = FileConfig::default();
        c.validate().unwrap();
        c.max_files = 0;
        assert!(c.validate().is_err());
        c.max_files = 2;
        assert!(c.validate().is_err());
        c.multiple = true;
        c.max_files = 32;
        c.validate().unwrap();
        c.max_files = 33;
        assert!(c.validate().is_err());
        c.max_files = 32;
        c.filters = vec![FileFilter::Mime("text/plain".into()); 33];
        assert!(c.validate().is_err());
        c.filters = vec![FileFilter::Mime(format!("application/{}", "x".repeat(110))); 32];
        assert!(c.validate().is_err());
        c.filters = vec![FileFilter::Mime("text/*".into())];
        c.validate_files(&[reference()]).unwrap();
        c.max_file_bytes = 3;
        assert!(c.validate_files(&[reference()]).is_err());
        c.max_file_bytes = 4;
        c.max_total_bytes = 3;
        assert!(c.validate_files(&[reference()]).is_err());
        for value in [
            "",
            "TEXT/plain",
            "text/**",
            "text/plain; charset=utf8",
            "text/",
            "*/plain",
        ] {
            assert!(FileFilter::Mime(value.into()).validate().is_err())
        }
        for value in ["txt", ".TXT", ".a/b"] {
            assert!(FileFilter::Extension(value.into()).validate().is_err())
        }
    }
    #[test]
    fn names_mime_metadata_total_unique_and_origin_domains() {
        let original = reference();
        let mut item = original.clone();
        item.name = "x".repeat(1025);
        assert!(item.validate().is_err());
        item = original.clone();
        item.name = "nul\0".into();
        assert!(item.validate().is_err());
        item = original.clone();
        item.media_type = "X/upper".into();
        assert!(item.validate().is_err());
        item = original.clone();
        item.size = MAX_FILE_BYTES + 1;
        assert!(item.validate().is_err());
        assert!(FileOutcome::Selected(vec![]).validate().is_err());
        assert!(
            FileOutcome::Selected(vec![original.clone(), original.clone()])
                .validate()
                .is_err()
        );
        let mut other = original.clone();
        other.token.serial = 2;
        other.token.document = FileDocumentId::new().unwrap();
        assert!(
            FileOutcome::Selected(vec![original.clone(), other])
                .validate()
                .is_err()
        );
        let files = (1..=32)
            .map(|serial| {
                let mut value = original.clone();
                value.token.serial = serial;
                value.name = "x".repeat(1024);
                value
            })
            .collect::<Vec<_>>();
        assert!(FileOutcome::Selected(files).validate().is_err());
    }
    #[test]
    fn chunk_ranges_are_checked_before_bytes_or_platform_work() {
        let file = reference();
        FileRead {
            file: file.token,
            offset: 4,
            length: 0,
        }
        .validate_file(&file)
        .unwrap();
        for (offset, length) in [(0, 65537), (u64::MAX, 1), (5, 0), (3, 2)] {
            assert!(
                FileRead {
                    file: file.token,
                    offset,
                    length
                }
                .validate_file(&file)
                .is_err()
            )
        }
        let mut foreign = file.clone();
        foreign.token.document = FileDocumentId::new().unwrap();
        assert!(
            FileRead {
                file: file.token,
                offset: 0,
                length: 4
            }
            .validate_file(&foreign)
            .is_err()
        );
    }
    #[test]
    fn request_origin_config_revision_and_changed_declaration_are_independent() {
        let file = reference();
        let mut props = FileProps {
            revision: 3,
            ..Default::default()
        };
        let event = FileEvent {
            document: file.token.document,
            revision: 3,
            request: Some(1),
            batch: 1,
            origin: FileOrigin::Picker,
            outcome: FileOutcome::Selected(vec![file.clone()]),
        };
        event.validate_node(file.token.node, &props).unwrap();
        let mut invalid = event.clone();
        invalid.origin = FileOrigin::Drop;
        invalid.request = None;
        assert!(invalid.validate_node(file.token.node, &props).is_err());
        props.configuration.drop = true;
        invalid.validate_node(file.token.node, &props).unwrap();
        invalid.revision = 4;
        assert!(invalid.validate_node(file.token.node, &props).is_err());
        let mut next = props.clone();
        next.configuration.max_file_bytes = 1;
        assert!(next.validate_update(&props).is_err());
        next.revision = 4;
        next.validate_update(&props).unwrap();
    }
}

#[path = "file_state.rs"]
pub(crate) mod state;

/// One exact bounded read response, never an ambient filesystem path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChunk {
    pub file: FileToken,
    pub offset: u64,
    pub bytes: Vec<u8>,
}
impl FileChunk {
    pub fn validate_read(&self, request: FileRead) -> Result<(), Error> {
        request.validate()?;
        if self.file != request.file
            || self.offset != request.offset
            || self.bytes.len() != request.length as usize
        {
            return Err(Error::InvalidResponse(
                "file response does not match its exact bounded read",
            ));
        }
        Ok(())
    }
}
