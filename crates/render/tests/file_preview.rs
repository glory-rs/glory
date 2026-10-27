use glory_render::*;
fn sof(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = vec![255, 192, 0, 17, 8];
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    bytes
}
#[test]
fn jpeg_preview_requires_first_scan_and_rejects_second_frame_before_it() {
    let mut bytes = vec![255, 216];
    bytes.extend(sof(20, 30));
    assert_eq!(
        inspect_file_preview(&bytes, 1024, "image/jpeg", Default::default()),
        Err(FilePreviewError::Incomplete)
    );
    let mut ambiguous = bytes.clone();
    ambiguous.extend(sof(10000, 10000));
    ambiguous.extend_from_slice(&[255, 218, 0, 12, 3, 1, 0, 2, 0x11, 3, 0x11, 0, 63, 0]);
    assert!(inspect_file_preview(&ambiguous, 1024, "image/jpeg", Default::default()).is_err());
    bytes.extend_from_slice(&[255, 218, 0, 12, 3, 1, 0, 2, 0x11, 3, 0x11, 0, 63, 0]);
    let image = inspect_file_preview(&bytes, 1024, "image/jpeg", Default::default()).unwrap();
    assert_eq!((image.width, image.height), (20, 30));
}

#[test]
fn native_preview_addresses_bind_every_token_field_and_never_accept_paths_or_foreign_uris() {
    let file = FileToken {
        document: FileDocumentId::new().unwrap(),
        node: NodeIds::new(RuntimeId::new()).allocate(),
        revision: 3,
        serial: 8,
    };
    let mut preview = FilePreview {
        file,
        image: FileImageInfo {
            format: FileImageFormat::Png,
            width: 32,
            height: 24,
        },
        source: FilePreview::native_source(file),
    };
    preview.validate().unwrap();
    for source in [
        "file:///tmp/image.png".to_owned(),
        "https://example.com/image.png".to_owned(),
        format!("{}?extra=1", preview.source),
        format!("{}#extra", preview.source),
        FilePreview::native_source(FileToken { serial: 9, ..file }),
        FilePreview::native_source(FileToken {
            document: FileDocumentId::new().unwrap(),
            ..file
        }),
        FilePreview::native_source(FileToken {
            revision: 4,
            ..file
        }),
        FilePreview::native_source(FileToken {
            node: NodeIds::new(RuntimeId::new()).allocate(),
            ..file
        }),
    ] {
        preview.source = source;
        assert!(preview.validate().is_err());
    }
    preview.source = "blob:https://example.com/document-owned".into();
    preview.validate().unwrap();
}
