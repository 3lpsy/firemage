use anyhow::Context;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    anyhow::ensure!(
        (12..=1024).contains(&password.len()),
        "password must be 12-1024 bytes"
    );
    Argon2::default()
        .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|p| p.to_string())
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}
pub fn is_password_valid(password: &str, hash: &str) -> bool {
    password.len() <= 1024
        && PasswordHash::new(hash).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .is_ok()
        })
}
pub fn new_token(kind: &str) -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("fm_{kind}_{}", hex::encode(bytes))
}
pub fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
pub async fn authenticate(
    db: &firemage_queries::DatabaseConnection,
    token: &str,
) -> anyhow::Result<(firemage_orm::users::Model, firemage_orm::credentials::Model)> {
    anyhow::ensure!(token.len() <= 128, "invalid token");
    firemage_queries::authenticate(db, &token_hash(token))
        .await?
        .context("invalid or expired token")
}
