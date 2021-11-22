use super::test_veilid_config::*;
use crate::dht::crypto::*;
use crate::dht::envelope::*;
use crate::dht::key::*;
use crate::dht::receipt::*;
use crate::xx::*;
use crate::*;

pub async fn test_envelope_round_trip() {
    info!("--- test envelope round trip ---");
    let veilid_core = VeilidCore::new();
    let api = veilid_core
        .startup(setup_veilid_core())
        .await
        .expect("startup failed");
    // Get crypto
    let crypto = veilid_core.crypto();

    // Create envelope
    let ts = 0x12345678ABCDEF69u64;
    let nonce = Crypto::get_random_nonce();
    let (sender_id, sender_secret) = generate_secret();
    let (recipient_id, recipient_secret) = generate_secret();
    let envelope = Envelope::new(0, ts, nonce, sender_id, recipient_id);

    // Create arbitrary body
    let body = b"This is an arbitrary body";

    // Serialize to bytes
    let enc_data = envelope
        .to_encrypted_data(crypto.clone(), body, &sender_secret)
        .expect("failed to encrypt data");

    // Deserialize from bytes
    let envelope2 =
        Envelope::from_data(&enc_data).expect("failed to deserialize envelope from data");

    let body2 = envelope2
        .decrypt_body(crypto.clone(), &enc_data, &recipient_secret)
        .expect("failed to decrypt envelope body");

    envelope2
        .decrypt_body(crypto.clone(), &enc_data, &sender_secret)
        .expect_err("should have failed to decrypt using wrong secret");

    // Compare envelope and body
    assert_eq!(envelope, envelope2);
    assert_eq!(body.to_vec(), body2);
    api.shutdown().await;
}

pub async fn test_receipt_round_trip() {
    info!("--- test receipt round trip ---");
    // Create arbitrary body
    let body = b"This is an arbitrary body";

    // Create receipt
    let nonce = Crypto::get_random_nonce();
    let (sender_id, sender_secret) = generate_secret();
    let receipt = Receipt::try_new(0, nonce, sender_id, body).expect("should not fail");

    // Serialize to bytes
    let mut enc_data = receipt
        .to_signed_data(&sender_secret)
        .expect("failed to make signed data");

    // Deserialize from bytes
    let receipt2 =
        Receipt::from_signed_data(&enc_data).expect("failed to deserialize envelope from data");

    // Should not validate even when a single bit is changed
    enc_data[5] = 0x01;
    Receipt::from_signed_data(&enc_data)
        .expect_err("should have failed to decrypt using wrong secret");

    // Compare receipts
    assert_eq!(receipt, receipt2);
}

pub async fn test_all() {
    test_envelope_round_trip().await;
    test_receipt_round_trip().await;
}
