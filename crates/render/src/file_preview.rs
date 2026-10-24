//! Header-only admission. Pixel and encoded budgets do not claim total decoder RSS.
pub const MAX_FILE_HEADER_BYTES: usize = 65536;
pub const MAX_PREVIEW_ENCODED_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_PREVIEW_PIXELS: u64 = 16 * 1024 * 1024;
pub const MAX_PREVIEW_DIMENSION: u32 = 16384;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilePreviewLimits {
    pub encoded_bytes: u64,
    pub pixels: u64,
    pub dimension: u32,
}
impl Default for FilePreviewLimits {
    fn default() -> Self {
        Self {
            encoded_bytes: MAX_PREVIEW_ENCODED_BYTES,
            pixels: MAX_PREVIEW_PIXELS,
            dimension: MAX_PREVIEW_DIMENSION,
        }
    }
}
impl FilePreviewLimits {
    pub fn validate(self) -> Result<(), FilePreviewError> {
        if self.encoded_bytes == 0
            || self.encoded_bytes > MAX_PREVIEW_ENCODED_BYTES
            || self.pixels == 0
            || self.pixels > MAX_PREVIEW_PIXELS
            || self.dimension == 0
            || self.dimension > MAX_PREVIEW_DIMENSION
        {
            Err(FilePreviewError::Budget)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileImageFormat {
    Png,
    Jpeg,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileImageInfo {
    pub format: FileImageFormat,
    pub width: u32,
    pub height: u32,
}
impl FileImageInfo {
    pub fn validate(self, limits: FilePreviewLimits) -> Result<(), FilePreviewError> {
        limits.validate()?;
        if self.width == 0
            || self.height == 0
            || self.width > limits.dimension
            || self.height > limits.dimension
            || u64::from(self.width)
                .checked_mul(u64::from(self.height))
                .is_none_or(|pixels| pixels > limits.pixels)
        {
            Err(FilePreviewError::Dimensions)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilePreviewError {
    Budget,
    Encoded,
    Unsupported,
    Incomplete,
    Malformed,
    Dimensions,
}
fn u32be(value: &[u8]) -> u32 {
    u32::from_be_bytes(value.try_into().expect("checked four-byte slice"))
}
fn crc32(value: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in value {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb88320 } else { 0 }
        }
    }
    !crc
}
fn png(bytes: &[u8]) -> Result<FileImageInfo, FilePreviewError> {
    if bytes.len() < 33 {
        return Err(FilePreviewError::Incomplete);
    }
    if &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || u32be(&bytes[8..12]) != 13
        || &bytes[12..16] != b"IHDR"
        || crc32(&bytes[12..29]) != u32be(&bytes[29..33])
    {
        return Err(FilePreviewError::Malformed);
    }
    let color = bytes[25];
    if bytes[24] != 8
        || !matches!(color, 0 | 2 | 3 | 4 | 6)
        || bytes[26] != 0
        || bytes[27] != 0
        || bytes[28] != 0
    {
        return Err(FilePreviewError::Unsupported);
    }
    let image = FileImageInfo {
        format: FileImageFormat::Png,
        width: u32be(&bytes[16..20]),
        height: u32be(&bytes[20..24]),
    };
    let mut cursor = 33usize;
    let mut palette = false;
    loop {
        let header = cursor
            .checked_add(8)
            .filter(|end| *end <= bytes.len())
            .ok_or(FilePreviewError::Incomplete)?;
        let length = u32be(&bytes[cursor..cursor + 4]) as usize;
        let kind = &bytes[cursor + 4..header];
        if kind == b"IDAT" {
            if color == 3 && !palette {
                return Err(FilePreviewError::Malformed);
            }
            return Ok(image);
        }
        if matches!(kind, b"acTL" | b"fcTL" | b"fdAT") {
            return Err(FilePreviewError::Unsupported);
        }
        if matches!(kind, b"IHDR" | b"IEND") || !kind.iter().all(u8::is_ascii_alphabetic) {
            return Err(FilePreviewError::Malformed);
        }
        let end = header
            .checked_add(length)
            .and_then(|v| v.checked_add(4))
            .filter(|end| *end <= bytes.len())
            .ok_or(FilePreviewError::Incomplete)?;
        if crc32(&bytes[cursor + 4..end - 4]) != u32be(&bytes[end - 4..end]) {
            return Err(FilePreviewError::Malformed);
        }
        if kind == b"PLTE" {
            if palette || length == 0 || length > 768 || !length.is_multiple_of(3) {
                return Err(FilePreviewError::Malformed);
            }
            palette = true;
        } else if kind[0].is_ascii_uppercase() {
            return Err(FilePreviewError::Unsupported);
        }
        cursor = end;
    }
}
fn jpeg(bytes: &[u8]) -> Result<FileImageInfo, FilePreviewError> {
    if bytes.len() < 2 {
        return Err(FilePreviewError::Incomplete);
    }
    if &bytes[..2] != b"\xff\xd8" {
        return Err(FilePreviewError::Malformed);
    }
    let mut cursor = 2usize;
    let mut frame = None::<(FileImageInfo, u8, [u8; 3], usize)>;
    loop {
        if bytes.get(cursor) != Some(&0xff) {
            return Err(if cursor >= bytes.len() {
                FilePreviewError::Incomplete
            } else {
                FilePreviewError::Malformed
            });
        }
        while bytes.get(cursor) == Some(&0xff) {
            cursor += 1;
        }
        let marker = *bytes.get(cursor).ok_or(FilePreviewError::Incomplete)?;
        cursor += 1;
        if marker == 0x01 {
            continue;
        }
        if matches!(marker, 0x00 | 0xd0..=0xd9) {
            return Err(FilePreviewError::Malformed);
        }
        if matches!(marker, 0xdc | 0xde | 0xdf) {
            return Err(FilePreviewError::Unsupported);
        }
        let header = cursor
            .checked_add(2)
            .filter(|end| *end <= bytes.len())
            .ok_or(FilePreviewError::Incomplete)?;
        let length = u16::from_be_bytes(bytes[cursor..header].try_into().unwrap()) as usize;
        if length < 2 {
            return Err(FilePreviewError::Malformed);
        }
        let end = cursor
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or(FilePreviewError::Incomplete)?;
        let payload = &bytes[header..end];
        if marker == 0xda {
            let (image, mode, components, count) = frame.ok_or(FilePreviewError::Malformed)?;
            let scan = usize::from(*payload.first().ok_or(FilePreviewError::Malformed)?);
            if scan == 0 || scan > count || length != 6 + 2 * scan {
                return Err(FilePreviewError::Malformed);
            }
            for index in 0..scan {
                let component = payload[1 + index * 2];
                let tables = payload[2 + index * 2];
                if !components[..count].contains(&component)
                    || tables >> 4 > 3
                    || tables & 15 > 3
                    || (0..index).any(|old| payload[1 + old * 2] == component)
                {
                    return Err(FilePreviewError::Malformed);
                }
            }
            let spectral = &payload[1 + 2 * scan..];
            if mode == 0xc0 && spectral != [0, 63, 0] {
                return Err(FilePreviewError::Unsupported);
            }
            if mode == 0xc2
                && (spectral[0] > spectral[1]
                    || spectral[1] > 63
                    || spectral[2] >> 4 > 13
                    || spectral[2] & 15 > 13
                    || spectral[0] == 0 && spectral[1] != 0
                    || spectral[0] != 0 && scan != 1)
            {
                return Err(FilePreviewError::Unsupported);
            }
            return Ok(image);
        }
        if matches!(marker, 0xc0 | 0xc2) {
            if frame.is_some() || payload.len() < 6 {
                return Err(FilePreviewError::Malformed);
            }
            let count = usize::from(payload[5]);
            if payload[0] != 8 || !matches!(count, 1 | 3) {
                return Err(FilePreviewError::Unsupported);
            }
            if length != 8 + 3 * count {
                return Err(FilePreviewError::Malformed);
            }
            let mut components = [0; 3];
            for index in 0..count {
                let base = 6 + index * 3;
                let sampling = payload[base + 1];
                if !(1..=4).contains(&(sampling >> 4))
                    || !(1..=4).contains(&(sampling & 15))
                    || payload[base + 2] > 3
                    || components[..index].contains(&payload[base])
                {
                    return Err(FilePreviewError::Malformed);
                }
                components[index] = payload[base];
            }
            frame = Some((
                FileImageInfo {
                    format: FileImageFormat::Jpeg,
                    height: u32::from(u16::from_be_bytes([payload[1], payload[2]])),
                    width: u32::from(u16::from_be_bytes([payload[3], payload[4]])),
                },
                marker,
                components,
                count,
            ));
        } else if matches!(marker, 0xc1 | 0xc3 | 0xc5..=0xcf) {
            return Err(FilePreviewError::Unsupported);
        }
        cursor = end;
    }
}
pub fn inspect_file_preview(
    bytes: &[u8],
    file_size: u64,
    media_type: &str,
    limits: FilePreviewLimits,
) -> Result<FileImageInfo, FilePreviewError> {
    limits.validate()?;
    if file_size > limits.encoded_bytes {
        return Err(FilePreviewError::Encoded);
    }
    if bytes.len() > MAX_FILE_HEADER_BYTES || bytes.len() as u64 > file_size {
        return Err(FilePreviewError::Budget);
    }
    let image = match media_type {
        "image/png" => png(bytes)?,
        "image/jpeg" => jpeg(bytes)?,
        _ => return Err(FilePreviewError::Unsupported),
    };
    image.validate(limits)?;
    Ok(image)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(name: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&(data.len() as u32).to_be_bytes());
        v.extend_from_slice(name);
        v.extend_from_slice(data);
        let crc = crc32(&v[4..]);
        v.extend_from_slice(&crc.to_be_bytes());
        v
    }
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&width.to_be_bytes());
        data.extend_from_slice(&height.to_be_bytes());
        data.extend_from_slice(&[8, 6, 0, 0, 0]);
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        out.extend(chunk(b"IHDR", &data));
        out.extend_from_slice(&[0, 0, 0, 1, b'I', b'D', b'A', b'T']);
        out
    }
    fn jpeg_header(marker: u8, width: u16, height: u16) -> Vec<u8> {
        let mut out = vec![255, 216, 255, marker, 0, 17, 8];
        out.extend_from_slice(&height.to_be_bytes());
        out.extend_from_slice(&width.to_be_bytes());
        out.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        out.extend_from_slice(&[
            255,
            218,
            0,
            12,
            3,
            1,
            0,
            2,
            0x11,
            3,
            0x11,
            0,
            if marker == 0xc2 { 0 } else { 63 },
            0,
        ]);
        out
    }
    #[test]
    fn png_crc_dimensions_and_animation_admission() {
        let bytes = png_header(320, 240);
        let result = inspect_file_preview(&bytes, 1024, "image/png", Default::default()).unwrap();
        assert_eq!((result.width, result.height), (320, 240));
        for (w, h) in [(0, 1), (1, 0), (16385, 1), (5000, 5000)] {
            assert_eq!(
                inspect_file_preview(&png_header(w, h), 1024, "image/png", Default::default()),
                Err(FilePreviewError::Dimensions)
            )
        }
        let mut bad = bytes.clone();
        bad[16] ^= 1;
        assert_eq!(
            inspect_file_preview(&bad, 1024, "image/png", Default::default()),
            Err(FilePreviewError::Malformed)
        );
        let mut animated = bytes[..33].to_vec();
        animated.extend(chunk(b"acTL", &[0; 8]));
        animated.extend_from_slice(&bytes[33..]);
        assert_eq!(
            inspect_file_preview(&animated, 1024, "image/png", Default::default()),
            Err(FilePreviewError::Unsupported)
        );
        for length in 0..bytes.len() {
            assert!(
                inspect_file_preview(&bytes[..length], 1024, "image/png", Default::default())
                    .is_err()
            )
        }
    }
    #[test]
    fn jpeg_baseline_progressive_finite_segments_and_unsupported_modes() {
        for marker in [0xc0, 0xc2] {
            let bytes = jpeg_header(marker, 400, 300);
            let value =
                inspect_file_preview(&bytes, 1000, "image/jpeg", Default::default()).unwrap();
            assert_eq!((value.width, value.height), (400, 300));
            for length in 0..bytes.len() {
                assert!(
                    inspect_file_preview(&bytes[..length], 1000, "image/jpeg", Default::default())
                        .is_err()
                )
            }
        }
        let mut bad = jpeg_header(0xc0, 400, 300);
        bad[6] = 12;
        assert_eq!(
            inspect_file_preview(&bad, 1000, "image/jpeg", Default::default()),
            Err(FilePreviewError::Unsupported)
        );
        assert_eq!(
            inspect_file_preview(
                &jpeg_header(0xc1, 400, 300),
                1000,
                "image/jpeg",
                Default::default()
            ),
            Err(FilePreviewError::Unsupported)
        );
    }
    #[test]
    fn independent_header_encoded_pixel_and_unknown_type_limits() {
        let bytes = png_header(40, 30);
        assert_eq!(
            inspect_file_preview(
                &bytes,
                MAX_PREVIEW_ENCODED_BYTES + 1,
                "image/png",
                Default::default()
            ),
            Err(FilePreviewError::Encoded)
        );
        assert_eq!(
            inspect_file_preview(
                &vec![0; MAX_FILE_HEADER_BYTES + 1],
                100000,
                "image/png",
                Default::default()
            ),
            Err(FilePreviewError::Budget)
        );
        assert_eq!(
            inspect_file_preview(&bytes, 1000, "image/svg+xml", Default::default()),
            Err(FilePreviewError::Unsupported)
        );
        assert_eq!(
            inspect_file_preview(
                &bytes,
                1000,
                "image/png",
                FilePreviewLimits {
                    pixels: 1199,
                    ..Default::default()
                }
            ),
            Err(FilePreviewError::Dimensions)
        );
        assert_eq!(
            inspect_file_preview(
                &bytes,
                1000,
                "image/png",
                FilePreviewLimits {
                    dimension: 39,
                    ..Default::default()
                }
            ),
            Err(FilePreviewError::Dimensions)
        );
    }
}
