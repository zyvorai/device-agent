// SPDX-License-Identifier: Apache-2.0

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Loads a hex-encoded SHA-256 hash from `path`. Returns `None` (never a hard error) so
/// a misconfigured/missing token file simply fails every bearer check closed rather than
/// panicking the daemon at startup.
pub fn load_token_hash(path: &str) -> Option<[u8; 32]> {
    let raw = std::fs::read_to_string(path)
        .inspect_err(|error| {
            tracing::warn!(%path, %error, "could not read bearer token hash file");
        })
        .ok()?;
    decode_hex_32(raw.trim())
}

/// Hashes `token` and compares it against `expected` in constant time.
pub fn verify(token: &str, expected: &[u8; 32]) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(token.trim().as_bytes());
    let actual: [u8; 32] = hasher.finalize().into();
    actual.ct_eq(expected).into()
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        tracing::warn!(
            len = value.len(),
            "bearer token hash file is not 64 hex chars"
        );
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_matching_token() {
        let mut hasher = Sha256::new();
        hasher.update(b"super-secret-token");
        let hash: [u8; 32] = hasher.finalize().into();
        assert!(verify("super-secret-token", &hash));
        assert!(!verify("wrong-token", &hash));
    }

    #[test]
    fn decodes_hex_hash() {
        let hex = "a".repeat(64);
        let decoded = decode_hex_32(&hex).unwrap();
        assert_eq!(decoded, [0xaa; 32]);
        assert!(decode_hex_32("too-short").is_none());
    }
}
