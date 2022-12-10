use anyhow::Context;
use rand::Rng;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};
use std::num::NonZeroU128;

pub type Secret = [u8; 32];
pub type RoomId = [u8; 32];

/// Generates a secret used to generate custom identifiers. The CSPRNG ChaCha20
/// is used to generate a 256-bit secret.
pub fn generate_secret() -> Secret {
    let mut csprng = ChaCha20Rng::from_entropy();
    let mut secret = [0u8; 32];
    csprng.fill(&mut secret);

    secret
}

/// Generates a unique, non-guessable 256-bit custom identifier. The CSPRNG ChaCha20 is
/// used to generate a 128-bit non-zero integer which is combined with a 256-bit secret
/// and hashed using SHA-256.
pub fn generate_room_identifier(secret: &Secret) -> anyhow::Result<RoomId> {
    let mut csprng = ChaCha20Rng::from_entropy();
    let random_number = NonZeroU128::new(csprng.gen())
        .with_context(|| "Unable to generate random number".to_string())?;

    let mut hasher = Sha256::new();
    hasher.update(random_number.get().to_be_bytes());
    hasher.update(secret);

    // uses generic_array (https://docs.rs/generic-array/latest/generic_array/)
    let result = hasher.finalize();

    // convert GenericArray<u8, U32> into [u8; 32] because
    let result_ref: &[u8; 32] = result.as_ref();
    Ok(*result_ref)
}

#[cfg(test)]
mod tests {
    use crate::crypto::*;

    #[test]
    fn test_generate_secret() {
        let secret = generate_secret();
    }

    #[test]
    fn test_generate_room_identifier() {
        let secret = generate_secret();
        let room_id = generate_room_identifier(&secret).unwrap();
    }
}
