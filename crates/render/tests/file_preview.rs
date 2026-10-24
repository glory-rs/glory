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
