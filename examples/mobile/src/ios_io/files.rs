use super::*;
use glory::blitz::files::{FilePicker, FileReader, Reply, SelectedFile};
use glory::render::{
    FileConfig, FileFilter, FileRejection, MAX_FILE_CHUNK_BYTES, MAX_FILE_METADATA_BYTES,
};
#[derive(Clone)]
pub struct IosFiles(Arc<Client>);
fn error(code: i64) -> FileRejection {
    match code {
        -1 | -8 => FileRejection::Unavailable,
        -2 => FileRejection::Busy,
        -4 => FileRejection::Count,
        -5 => FileRejection::Type,
        -6 => FileRejection::Bytes,
        -7 => FileRejection::Metadata,
        _ => FileRejection::Failed,
    }
}
impl IosFiles {
    #[cfg(feature = "ios-io-probe")]
    pub(super) fn probe(&self, action: i64) -> Result<(), FileRejection> {
        accepted((self.0.host)(2, self.0.id, 100, 0, action, 0)).map_err(error)
    }
    pub(super) fn open(host: Host) -> Result<Self, FileRejection> {
        Client::open(host, 2).map(Self).map_err(error)
    }
    pub fn close(&self) {
        self.0.close();
    }
    /// Receipt count, picker admission, read admission, reserved handles,
    /// reserved metadata, reserved declared bytes, and live clients.
    pub fn stats(&self) -> Result<[u64; 7], FileRejection> {
        let limits = [5, 1, 4, 160, 160 * 1024, 2 << 30, 32];
        let mut stats = [0; 7];
        for (i, value) in stats.iter_mut().enumerate() {
            let code = self.0.call(11, 0, i as i64, 0);
            *value = u64::try_from(code).map_err(|_| error(code))?;
            if *value > limits[i] {
                return Err(FileRejection::Failed);
            }
        }
        Ok(stats)
    }
}
fn configuration(config: &FileConfig) -> Result<Vec<u8>, FileRejection> {
    config.validate().map_err(|_| FileRejection::Failed)?;
    let mut bytes = Vec::new();
    bytes.extend(1u32.to_be_bytes());
    bytes.extend(u32::from(config.multiple).to_be_bytes());
    bytes.extend((config.max_files as u32).to_be_bytes());
    bytes.extend(config.max_file_bytes.to_be_bytes());
    bytes.extend(config.max_total_bytes.to_be_bytes());
    bytes.extend((config.filters.len() as u32).to_be_bytes());
    for filter in &config.filters {
        let value = filter.text().as_bytes();
        bytes.extend((value.len() as u16).to_be_bytes());
        bytes.extend(value);
    }
    Ok(bytes)
}
struct Packet<'a>(&'a [u8]);
impl<'a> Packet<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], FileRejection> {
        if length > self.0.len() {
            return Err(FileRejection::Metadata);
        }
        let (head, rest) = self.0.split_at(length);
        self.0 = rest;
        Ok(head)
    }
    fn number(&mut self, length: usize) -> Result<u64, FileRejection> {
        let mut value = 0;
        for byte in self.take(length)? {
            value = (value << 8) | u64::from(*byte);
        }
        Ok(value)
    }
    fn text(&mut self, length: usize) -> Result<String, FileRejection> {
        std::str::from_utf8(self.take(length)?)
            .map(str::to_owned)
            .map_err(|_| FileRejection::Metadata)
    }
}
struct Metadata {
    id: i64,
    size: u64,
    name: String,
    mime: String,
}
fn metadata(bytes: &[u8], config: &FileConfig) -> Result<Vec<Metadata>, FileRejection> {
    let mut input = Packet(bytes);
    let count = input.number(4)? as usize;
    if count == 0 || count > config.max_files {
        return Err(FileRejection::Count);
    }
    let mut files: Vec<Metadata> = Vec::with_capacity(count);
    let mut total = 0u64;
    let mut metadata = 0usize;
    for _ in 0..count {
        let id = i64::try_from(input.number(8)?).map_err(|_| FileRejection::Metadata)?;
        let size = input.number(8)?;
        let name_len = input.number(2)? as usize;
        let mime_len = input.number(2)? as usize;
        if id <= 0
            || files.iter().any(|file| file.id == id)
            || name_len == 0
            || name_len > 1024
            || mime_len > 127
        {
            return Err(FileRejection::Metadata);
        }
        metadata += name_len + mime_len;
        if metadata > MAX_FILE_METADATA_BYTES {
            return Err(FileRejection::Metadata);
        }
        total = total.checked_add(size).ok_or(FileRejection::Bytes)?;
        if size > config.max_file_bytes || total > config.max_total_bytes {
            return Err(FileRejection::Bytes);
        }
        let name = input.text(name_len)?;
        let mime = input.text(mime_len)?;
        let token = |s: &str| {
            !s.is_empty()
                && s.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || b"!#$&-^_.+".contains(&b)
                })
        };
        if name.contains(['\r', '\0'])
            || (!mime.is_empty()
                && !mime
                    .split_once('/')
                    .is_some_and(|(a, b)| token(a) && token(b)))
        {
            return Err(FileRejection::Metadata);
        }
        if !config.filters.is_empty()
            && !config.filters.iter().any(|filter| match filter {
                FileFilter::Extension(extension) => name.to_ascii_lowercase().ends_with(extension),
                FileFilter::Mime(filter) => {
                    if let Some(prefix) = filter.strip_suffix('*') {
                        mime.starts_with(prefix)
                    } else {
                        mime == *filter
                    }
                }
            })
        {
            return Err(FileRejection::Type);
        }
        files.push(Metadata {
            id,
            size,
            name,
            mime,
        });
    }
    if !input.0.is_empty() {
        return Err(FileRejection::Metadata);
    }
    Ok(files)
}
impl FilePicker for IosFiles {
    fn pick(&self, config: FileConfig) -> Reply<Option<Vec<SelectedFile>>> {
        let packet = match configuration(&config) {
            Ok(packet) => packet,
            Err(error) => return Box::pin(std::future::ready(Err(error))),
        };
        let job = self.0.job(0, packet, MAX_FILE_METADATA_BYTES + 32 * 20 + 4);
        let client = self.0.clone();
        Box::pin(async move {
            let mut reply = job.await.map_err(error)?;
            if reply.code == 1 {
                return Ok(None);
            }
            let metadata = metadata(&reply.bytes, &config)?;
            reply.adopt().map_err(error)?;
            Ok(Some(
                metadata
                    .into_iter()
                    .map(|file| SelectedFile {
                        name: file.name,
                        media_type: file.mime,
                        size: file.size,
                        reader: Arc::new(Reader {
                            source: Arc::new(Source {
                                client: client.clone(),
                                file: file.id,
                                size: file.size,
                            }),
                        }),
                    })
                    .collect(),
            ))
        })
    }
}
struct Reader {
    source: Arc<Source>,
}
struct Source {
    client: Arc<Client>,
    file: i64,
    size: u64,
}
impl Drop for Source {
    fn drop(&mut self) {
        self.client.call(10, self.file, 0, 0);
    }
}
impl FileReader for Reader {
    fn read(&self, offset: u64, length: u32) -> Reply<Vec<u8>> {
        if length > MAX_FILE_CHUNK_BYTES
            || offset > self.source.size
            || u64::from(length) > self.source.size - offset
        {
            return Box::pin(std::future::ready(Err(FileRejection::Bytes)));
        }
        let source = self.source.clone();
        let mut packet = source.file.to_be_bytes().to_vec();
        packet.extend(offset.to_be_bytes());
        packet.extend(length.to_be_bytes());
        let job = source.client.job(1, packet, length as usize);
        Box::pin(async move {
            let result = job.await;
            // The lazy future retains its selected capability through native
            // admission/settlement, even when the caller drops the Reader.
            drop(source);
            let mut reply = result.map_err(error)?;
            if reply.code != 2 || reply.bytes.len() != length as usize {
                return Err(FileRejection::Bytes);
            }
            Ok(std::mem::take(&mut reply.bytes))
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn packet(id: u64, size: u64, name: &str, mime: &str) -> Vec<u8> {
        let mut bytes = 1u32.to_be_bytes().to_vec();
        bytes.extend(id.to_be_bytes());
        bytes.extend(size.to_be_bytes());
        bytes.extend((name.len() as u16).to_be_bytes());
        bytes.extend((mime.len() as u16).to_be_bytes());
        bytes.extend(name.as_bytes());
        bytes.extend(mime.as_bytes());
        bytes
    }
    #[test]
    fn file_metadata_defends_the_original_configuration_before_native_adoption() {
        let config = FileConfig {
            filters: vec![FileFilter::Extension(".txt".into())],
            ..Default::default()
        };
        let good = packet(17, 9, "中文😀.txt", "text/plain");
        assert_eq!(metadata(&good, &config).unwrap()[0].id, 17);
        for i in 0..good.len() {
            assert!(metadata(&good[..i], &config).is_err());
        }
        for bad in [
            packet(0, 9, "a.txt", "text/plain"),
            packet(17, 9, "a.png", "image/png"),
            packet(17, 9, "a.txt", "TEXT/plain"),
            packet(17, config.max_file_bytes + 1, "a.txt", "text/plain"),
        ] {
            assert!(metadata(&bad, &config).is_err());
        }
        let mut trailing = good;
        trailing.push(0);
        assert!(metadata(&trailing, &config).is_err());
        assert_eq!(configuration(&config).unwrap().len(), 38);
    }
}
