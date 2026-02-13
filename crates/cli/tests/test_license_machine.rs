use revet_cli::license::machine::{hex_encode, machine_id};

#[test]
fn machine_id_is_deterministic() {
    let id1 = machine_id();
    let id2 = machine_id();
    assert_eq!(id1, id2);
}

#[test]
fn machine_id_is_16_hex_chars() {
    let id = machine_id();
    assert_eq!(id.len(), 16, "Expected 16 hex chars, got: {id}");
    assert!(
        id.chars().all(|c| c.is_ascii_hexdigit()),
        "Non-hex char in: {id}"
    );
}

#[test]
fn hex_encode_empty() {
    assert_eq!(hex_encode(&[]), "");
}

#[test]
fn hex_encode_known_values() {
    assert_eq!(hex_encode(&[0x00]), "00");
    assert_eq!(hex_encode(&[0xff]), "ff");
    assert_eq!(hex_encode(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
}
