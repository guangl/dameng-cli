use super::*;

#[test]
fn imports_ciphertext_from_previous_crypto_dependencies() {
    // Generated using aes-gcm 0.10, pbkdf2 0.12 and sha2 0.10:
    // plaintext [], salt [3; 16], nonce [7; 12], 600_000 rounds.
    let temp = TempDir::new().unwrap();
    let target = context(&temp, "legacy");
    let document = ExportDocument {
        version: 1,
        count: 0,
        connections: None,
        encrypted_payload: Some(
            "0707070707070707070707072e6e13fc9c8dd7deea45584d20c25eb6fde3".into(),
        ),
        salt: Some("03".repeat(16)),
    };
    assert_eq!(
        import_document(
            &target,
            document,
            false,
            Some(&prompter("legacy-passphrase"))
        )
        .unwrap(),
        0
    );
}
