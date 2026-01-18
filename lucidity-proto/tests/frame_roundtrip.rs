use k9::assert_equal;
use lucidity_proto::frame::{encode_frame, DecodeError, Frame, FrameDecoder, MAX_FRAME_LEN};

#[test]
fn frame_roundtrips_single_chunk() {
    let frame = Frame {
        typ: 7,
        payload: b"hello".to_vec(),
    };
    let encoded = frame.encode_to_vec();

    let mut dec = FrameDecoder::new();
    dec.push(&encoded);

    let decoded = dec.next_frame().unwrap().unwrap();
    assert_equal!(decoded, frame);
    assert_equal!(dec.next_frame().unwrap(), None);
}

#[test]
fn frame_decodes_across_chunks() {
    let encoded = encode_frame(42, b"abcdef");

    let mut dec = FrameDecoder::new();
    dec.push(&encoded[..3]);
    assert_equal!(dec.next_frame().unwrap(), None);

    dec.push(&encoded[3..5]);
    assert_equal!(dec.next_frame().unwrap(), None);

    dec.push(&encoded[5..]);
    let f = dec.next_frame().unwrap().unwrap();
    assert_equal!(f.typ, 42);
    assert_equal!(f.payload, b"abcdef");
    assert_equal!(dec.next_frame().unwrap(), None);
}

#[test]
fn frame_rejects_length_too_large() {
    let mut dec = FrameDecoder::new();
    dec.push(&(MAX_FRAME_LEN + 1).to_le_bytes());
    dec.push(&[0u8; 8]);
    assert_equal!(
        dec.next_frame().unwrap_err(),
        DecodeError::LengthTooLarge(MAX_FRAME_LEN + 1)
    );
}

#[test]
fn frame_rejects_zero_length() {
    let mut dec = FrameDecoder::new();
    dec.push(&0u32.to_le_bytes());
    assert_equal!(dec.next_frame().unwrap_err(), DecodeError::InvalidLength(0));
}
