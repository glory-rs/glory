//! Reproducible, streaming release sidecars. Original URLs and bytes stay valid.
use crate::Result;
use flate2::{Compression, GzBuilder};
use std::{fs, path::Path};

pub(crate) fn compress(directory: &Path) -> Result<()> {
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let path = entry.path();
        if kind.is_symlink() {
            return Err(format!("release asset is a symlink: {}", path.display()));
        }
        if kind.is_dir() {
            compress(&path)?;
            continue;
        }
        if !kind.is_file()
            || !matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("html" | "js" | "mjs" | "wasm" | "css" | "json" | "svg" | "txt")
            )
            || entry.metadata().map_err(|e| e.to_string())?.len() < 256
        {
            continue;
        }
        let mut name = path.as_os_str().to_owned();
        name.push(".gz");
        let target = Path::new(&name);
        // Never overwrite an explicitly supplied application asset.
        let output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)
            .map_err(|e| format!("compressed asset {}: {e}", target.display()))?;
        let mut encoder = GzBuilder::new()
            .mtime(0)
            .operating_system(255)
            .write(output, Compression::best());
        let mut input = fs::File::open(&path).map_err(|e| e.to_string())?;
        let size = std::io::copy(&mut input, &mut encoder).map_err(|e| e.to_string())?;
        let output = encoder.finish().map_err(|e| e.to_string())?;
        let compressed = output.metadata().map_err(|e| e.to_string())?.len();
        drop(output);
        if compressed >= size {
            fs::remove_file(target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Missing quality is one. Invalid values are refused rather than rounded into
/// permission to send an encoding that a client explicitly disabled.
fn quality(value: &str) -> Option<u16> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if !matches!(whole, "0" | "1")
        || fraction.len() > 3
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || whole == "1" && fraction.bytes().any(|b| b != b'0')
    {
        return None;
    }
    if whole == "1" {
        return Some(1000);
    }
    let part = fraction.parse::<u16>().unwrap_or(0);
    Some(part * 10_u16.pow(3 - fraction.len() as u32))
}

/// The identity fallback is always available. A wildcard zero also excludes it
/// unless identity is explicitly allowed. Prefer gzip on equal quality.
pub(crate) fn negotiate(header: &str, has_gzip: bool) -> Result<bool> {
    let mut gzip = None;
    let mut identity = None;
    let mut wildcard = None;
    for item in header.split(',') {
        let mut parts = item.trim().split(';');
        let name = parts.next().unwrap_or("").trim();
        let mut q = 1000;
        let mut seen_quality = false;
        for parameter in parts {
            if let Some((key, value)) = parameter.trim().split_once('=')
                && key.trim().eq_ignore_ascii_case("q")
                && !seen_quality
            {
                q = quality(value.trim()).unwrap_or(0);
                seen_quality = true;
            } else {
                q = 0;
            }
        }
        let slot = if name.eq_ignore_ascii_case("gzip") {
            &mut gzip
        } else if name.eq_ignore_ascii_case("identity") {
            &mut identity
        } else if name == "*" {
            &mut wildcard
        } else {
            continue;
        };
        *slot = Some(slot.unwrap_or(0).max(q));
    }
    let identity = identity.unwrap_or(if wildcard == Some(0) { 0 } else { 1000 });
    let gzip = if has_gzip {
        gzip.or(wildcard).unwrap_or(0)
    } else {
        0
    };
    if gzip > 0 && gzip >= identity {
        Ok(true)
    } else if identity > 0 {
        Ok(false)
    } else if gzip > 0 {
        Ok(true)
    } else {
        Err("no acceptable content encoding".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn release_sidecars_round_trip_are_reproducible_and_preserve_supplied_assets() {
        let first = crate::test_support::Scratch::new();
        let second = crate::test_support::Scratch::new();
        let source = "/* 中文 */ body { color: green; }\n".repeat(100);
        for root in [&first, &second] {
            root.write("nested/app.css", &source);
            root.write("tiny.js", "hello");
            root.write("image.png", &"png".repeat(500));
            compress(&root.0).unwrap();
            assert!(!root.0.join("tiny.js.gz").exists());
            assert!(!root.0.join("image.png.gz").exists());
        }
        let compressed = fs::read(first.0.join("nested/app.css.gz")).unwrap();
        assert_eq!(
            compressed,
            fs::read(second.0.join("nested/app.css.gz")).unwrap()
        );
        assert!(compressed.len() < source.len());
        let mut decoded = String::new();
        flate2::read::GzDecoder::new(compressed.as_slice())
            .read_to_string(&mut decoded)
            .unwrap();
        assert_eq!(decoded, source);
        // An explicitly configured .gz must never be silently replaced.
        assert!(compress(&first.0).is_err());
        assert_eq!(
            fs::read(first.0.join("nested/app.css.gz")).unwrap(),
            compressed
        );
    }

    #[test]
    fn encoding_quality_exclusions_wildcards_and_identity_fallback() {
        for header in [
            "gzip",
            "br, GZip;q=1.000",
            "*",
            "gzip;q=0.8,identity;q=0.5",
            "identity;q=0,gzip;q=0.1",
        ] {
            assert!(negotiate(header, true).unwrap(), "{header}");
        }
        for header in [
            "",
            "br",
            "gzip;q=0",
            "gzip;q=0.5",
            "gzip;q=0,*;q=1",
            "gzip;q=1.001",
            "gzip;q=0.0001",
            "gzip;q=bogus",
        ] {
            assert!(!negotiate(header, true).unwrap(), "{header}");
        }
        assert!(!negotiate("gzip", false).unwrap());
        assert!(negotiate("*;q=0", true).is_err());
        assert!(negotiate("identity;q=0,gzip;q=0", true).is_err());
        assert!(negotiate("identity;q=0,br", false).is_err());
        assert!(!negotiate("*;q=0,identity;q=0.1", true).unwrap());
    }
}
