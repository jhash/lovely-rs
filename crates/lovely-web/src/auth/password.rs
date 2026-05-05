use crate::WebError;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand::rngs::OsRng;

pub fn hash_password(password: &str) -> Result<String, WebError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let hash = argon
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| WebError::Other(anyhow::anyhow!("hash: {e}")))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(p) => p,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip() {
        let h = hash_password("hunter2hunter2").unwrap();
        assert!(verify_password("hunter2hunter2", &h));
        assert!(!verify_password("wrong", &h));
    }
}
