use ori_bridge_server::{read_frame, FrameError, PROTOCOL_MAGIC};
use std::io::Cursor;

#[test]
fn test_rejects_invalid_magic() {
    let bad_magic = b"NOPE\x04\x00\x00\x00test";
    let mut cursor = Cursor::new(bad_magic);

    let err = read_frame(&mut cursor).expect_err("should reject invalid magic");
    assert!(matches!(err, FrameError::InvalidMagic));
}

#[test]
fn test_rejects_frame_too_large() {
    let mut header = Vec::new();
    header.extend_from_slice(&PROTOCOL_MAGIC);
    // Declare 128 MiB frame (> 64 MiB limit)
    header.extend_from_slice(&(128 * 1024 * 1024u32).to_le_bytes());

    let mut cursor = Cursor::new(header);
    let err = read_frame(&mut cursor).expect_err("should reject oversized frame");
    assert!(matches!(err, FrameError::FrameTooLarge { .. }));
}

#[test]
fn test_rejects_truncated_payload() {
    let mut header = Vec::new();
    header.extend_from_slice(&PROTOCOL_MAGIC);
    // Declare 10 bytes, but only provide 3
    header.extend_from_slice(&10u32.to_le_bytes());
    header.extend_from_slice(b"abc");

    let mut cursor = Cursor::new(header);
    let err = read_frame(&mut cursor).expect_err("should reject truncated payload");
    assert!(matches!(err, FrameError::Truncated { expected: 10 }));
}

#[test]
fn test_rejects_empty_payload() {
    let mut header = Vec::new();
    header.extend_from_slice(&PROTOCOL_MAGIC);
    header.extend_from_slice(&0u32.to_le_bytes());

    let mut cursor = Cursor::new(header);
    let err = read_frame(&mut cursor).expect_err("should reject empty payload");
    assert!(matches!(err, FrameError::EmptyPayload));
}
