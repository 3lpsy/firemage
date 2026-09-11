use super::*;

#[test]
fn frames_round_trip_and_reject_truncation_or_oversize() {
    let message = ClientMessage::Open {
        version: VERSION,
        command: vec!["/bin/sh".into(), "-i".into()],
        rows: 24,
        cols: 80,
    };
    let mut bytes = Vec::new();
    write_frame(&mut bytes, &message).unwrap();
    assert_eq!(
        read_frame::<_, ClientMessage>(&mut bytes.as_slice()).unwrap(),
        message
    );
    assert!(read_frame::<_, ClientMessage>(&mut &bytes[..bytes.len() - 1]).is_err());
    assert!(
        read_frame::<_, ClientMessage>(&mut &((MAX_FRAME_BYTES + 1) as u32).to_be_bytes()[..])
            .is_err()
    );
}

#[test]
fn rejects_invalid_open_and_input() {
    assert!(validate_open(VERSION, &[], 24, 80).is_ok());
    assert!(validate_open(VERSION + 1, &[], 24, 80).is_err());
    assert!(validate_open(VERSION, &["/bin/sh\0".into()], 24, 80).is_err());
    assert!(validate_open(VERSION, &["".into()], 24, 80).is_err());
    assert!(validate_open(VERSION, &vec!["arg".into(); 65], 24, 80).is_err());
    assert!(validate_size(0, 80).is_err());
    assert!(validate_size(24, 1001).is_err());
    assert_eq!(
        decode_data(&encode_data(&[0, 255, 13])).unwrap(),
        vec![0, 255, 13]
    );
    assert!(decode_data("%").is_err());
    assert!(decode_data(&encode_data(&vec![0; MAX_DATA_BYTES + 1])).is_err());
}
