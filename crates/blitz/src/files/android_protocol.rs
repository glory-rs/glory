//! Bounded protocol-v1 packets. No JNI/platform objects enter shared UI state.
use super::*;
pub(super) const MAX_REPLY: usize = glory_render::MAX_FILE_METADATA_BYTES + 32 * 20 + 4;
pub(super) struct Metadata {
    pub id: i64,
    pub size: u64,
    pub name: String,
    pub media_type: String,
}
pub(super) fn status(code: i64) -> Result<(), FileRejection> {
    match code {
        0..=2 => Ok(()),
        -1 => Err(FileRejection::Count),
        -2 => Err(FileRejection::Type),
        -3 => Err(FileRejection::Bytes),
        -4 => Err(FileRejection::Metadata),
        -5 => Err(FileRejection::Busy),
        -6 => Err(FileRejection::Unavailable),
        _ => Err(FileRejection::Failed),
    }
}
pub(super) fn encode(config: &FileConfig) -> Result<Vec<u8>, FileRejection> {
    config.validate().map_err(|_| FileRejection::Failed)?;
    let mut bytes = Vec::with_capacity(32 + glory_render::MAX_FILE_FILTER_BYTES + 64);
    bytes.extend_from_slice(&1u32.to_be_bytes());
    bytes.extend_from_slice(&u32::from(config.multiple).to_be_bytes());
    bytes.extend_from_slice(&(config.max_files as u32).to_be_bytes());
    bytes.extend_from_slice(&config.max_file_bytes.to_be_bytes());
    bytes.extend_from_slice(&config.max_total_bytes.to_be_bytes());
    bytes.extend_from_slice(&(config.filters.len() as u32).to_be_bytes());
    for filter in &config.filters {
        let value = filter.text().as_bytes();
        bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
        bytes.extend_from_slice(value);
    }
    Ok(bytes)
}
fn take<'a>(input: &mut &'a [u8], length: usize) -> Result<&'a [u8], FileRejection> {
    if length > input.len() {
        return Err(FileRejection::Metadata);
    }
    let (value, rest) = input.split_at(length);
    *input = rest;
    Ok(value)
}
pub(super) fn decode(bytes: &[u8]) -> Result<Vec<Metadata>, FileRejection> {
    if bytes.len() > MAX_REPLY {
        return Err(FileRejection::Metadata);
    }
    let mut input = bytes;
    let count = u32::from_be_bytes(take(&mut input, 4)?.try_into().expect("four bytes")) as usize;
    if !(1..=32).contains(&count) {
        return Err(FileRejection::Count);
    }
    let mut result: Vec<Metadata> = Vec::with_capacity(count);
    let mut metadata = 0usize;
    let mut total = 0u64;
    for _ in 0..count {
        let id = i64::from_be_bytes(take(&mut input, 8)?.try_into().expect("eight bytes"));
        let size = u64::from_be_bytes(take(&mut input, 8)?.try_into().expect("eight bytes"));
        let name = u16::from_be_bytes(take(&mut input, 2)?.try_into().expect("two bytes")) as usize;
        let mime = u16::from_be_bytes(take(&mut input, 2)?.try_into().expect("two bytes")) as usize;
        if id <= 0
            || result.iter().any(|item| item.id == id)
            || name == 0
            || name > 1024
            || mime > 127
        {
            return Err(FileRejection::Metadata);
        }
        metadata += name + mime;
        total = total.checked_add(size).ok_or(FileRejection::Bytes)?;
        if metadata > glory_render::MAX_FILE_METADATA_BYTES {
            return Err(FileRejection::Metadata);
        }
        if size > glory_render::MAX_FILE_BYTES || total > glory_render::MAX_FILE_BYTES {
            return Err(FileRejection::Bytes);
        }
        let name =
            std::str::from_utf8(take(&mut input, name)?).map_err(|_| FileRejection::Metadata)?;
        let media =
            std::str::from_utf8(take(&mut input, mime)?).map_err(|_| FileRejection::Metadata)?;
        if name.contains(['\r', '\0']) {
            return Err(FileRejection::Metadata);
        }
        if !media.is_empty() {
            let token = |text: &str| {
                !text.is_empty()
                    && text.bytes().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || b"!#$&-^_.+".contains(&c)
                    })
            };
            if !media
                .split_once('/')
                .is_some_and(|(major, minor)| token(major) && token(minor))
            {
                return Err(FileRejection::Metadata);
            }
        }
        result.push(Metadata {
            id,
            size,
            name: name.to_owned(),
            media_type: media.to_owned(),
        });
    }
    if !input.is_empty() {
        return Err(FileRejection::Metadata);
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn packet(files: &[(i64, u64, &str, &str)]) -> Vec<u8> {
        let mut bytes = (files.len() as u32).to_be_bytes().to_vec();
        for &(id, size, name, mime) in files {
            bytes.extend(id.to_be_bytes());
            bytes.extend(size.to_be_bytes());
            bytes.extend((name.len() as u16).to_be_bytes());
            bytes.extend((mime.len() as u16).to_be_bytes());
            bytes.extend(name.as_bytes());
            bytes.extend(mime.as_bytes());
        }
        bytes
    }
    #[test]
    fn finite_request_packet_and_error_codes_match_the_java_boundary() {
        let config = FileConfig {
            filters: vec![glory_render::FileFilter::Extension(".txt".into())],
            ..Default::default()
        };
        let bytes = encode(&config).unwrap();
        assert_eq!(bytes.len(), 38);
        assert_eq!(&bytes[..12], &[0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(&bytes[28..], &[0, 0, 0, 1, 0, 4, b'.', b't', b'x', b't']);
        for (code, expected) in [
            (-1, FileRejection::Count),
            (-2, FileRejection::Type),
            (-3, FileRejection::Bytes),
            (-4, FileRejection::Metadata),
            (-5, FileRejection::Busy),
            (-6, FileRejection::Unavailable),
            (-7, FileRejection::Failed),
            (3, FileRejection::Failed),
        ] {
            assert_eq!(status(code), Err(expected));
        }
    }
    #[test]
    fn selection_metadata_is_exact_utf8_bounded_and_owns_no_paths() {
        let bytes = packet(&[(7, 4, "中文😀.txt", "text/plain")]);
        let files = decode(&bytes).unwrap();
        assert_eq!(
            (
                files[0].id,
                files[0].size,
                files[0].name.as_str(),
                files[0].media_type.as_str()
            ),
            (7, 4, "中文😀.txt", "text/plain")
        );
        for length in 0..bytes.len() {
            assert!(decode(&bytes[..length]).is_err());
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(decode(&extra).is_err());
        let mut malformed = packet(&[(7, 4, "a.txt", "text/plain")]);
        malformed[24] = 0xff;
        assert!(decode(&malformed).is_err());
        for file in [
            (0, 4, "a.txt", "text/plain"),
            (7, glory_render::MAX_FILE_BYTES + 1, "a.txt", "text/plain"),
            (7, 4, "bad\rname", "text/plain"),
            (7, 4, "a.txt", "TEXT/plain"),
            (7, 4, "a.txt", "text/*"),
        ] {
            assert!(decode(&packet(&[file])).is_err());
        }
        assert!(
            decode(&packet(&[
                (7, 4, "a.txt", "text/plain"),
                (7, 4, "b.txt", "text/plain")
            ]))
            .is_err()
        );
        assert!(decode(&vec![0; MAX_REPLY + 1]).is_err());
        assert!(
            decode(&packet(&[
                (7, glory_render::MAX_FILE_BYTES, "a.txt", "text/plain"),
                (8, 1, "b.txt", "text/plain")
            ]))
            .is_err()
        );
        let huge = "😀".repeat(300);
        assert!(decode(&packet(&[(7, 4, &huge, "text/plain")])).is_err());
    }
}
