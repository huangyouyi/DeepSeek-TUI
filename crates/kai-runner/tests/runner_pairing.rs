use kai_runner::pairing::{PairingError, PairingManager};
use std::time::{Duration, Instant};

#[test]
fn creates_pairing_code_with_expiration() {
    let now = Instant::now();
    let mut manager = PairingManager::with_generators(
        Duration::from_secs(60),
        || "CODE-123456".to_string(),
        || "token-1".to_string(),
    );

    let code = manager.create_at(now);

    assert_eq!(code.value(), "CODE-123456");
    assert_eq!(code.expires_at(), now + Duration::from_secs(60));
}

#[test]
fn redeems_pairing_code_once_into_token() {
    let now = Instant::now();
    let mut manager = PairingManager::with_generators(
        Duration::from_secs(60),
        || "CODE-123456".to_string(),
        || "token-1".to_string(),
    );
    let code = manager.create_at(now);

    let token = manager.redeem_at(code.value(), now + Duration::from_secs(10));

    assert!(token.is_ok());
    let token = token.unwrap();
    assert_eq!(token.value(), "token-1");
    assert_eq!(token.descriptor().label(), "pairing-token");
}

#[test]
fn redeeming_pairing_code_twice_is_rejected() {
    let now = Instant::now();
    let mut token_index = 0;
    let mut manager = PairingManager::with_generators(
        Duration::from_secs(60),
        || "CODE-123456".to_string(),
        || {
            token_index += 1;
            format!("token-{token_index}")
        },
    );
    let code = manager.create_at(now);

    assert!(manager.redeem_at(code.value(), now).is_ok());

    assert_eq!(
        manager.redeem_at(code.value(), now),
        Err(PairingError::AlreadyUsed)
    );
}

#[test]
fn expired_pairing_code_is_rejected() {
    let now = Instant::now();
    let mut manager = PairingManager::with_generators(
        Duration::from_secs(60),
        || "CODE-123456".to_string(),
        || "token-1".to_string(),
    );
    let code = manager.create_at(now);

    assert_eq!(
        manager.redeem_at(code.value(), now + Duration::from_secs(61)),
        Err(PairingError::Expired)
    );
}

#[test]
fn unknown_pairing_code_is_rejected() {
    let now = Instant::now();
    let mut manager = PairingManager::with_generators(
        Duration::from_secs(60),
        || "CODE-123456".to_string(),
        || "token-1".to_string(),
    );

    assert_eq!(
        manager.redeem_at("CODE-000000", now),
        Err(PairingError::Unknown)
    );
}
