// SPDX-License-Identifier: Apache-2.0

//! Device-identity signatures over canonical payloads.
//!
//! Software keys (PKCS#8 ECDSA P-256, the same keys `rcgen` writes) sign and
//! verify here. TPM keys are not exported; callers that only have a TPM blob
//! leave `signature` empty rather than pretending a software key signed.

use anyhow::Context;
use p256::ecdsa::signature::{Signer, Verifier};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::pkcs8::DecodePrivateKey;
use p256::EncodedPoint;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureBlock {
    pub algorithm: String,
    pub digest_sha256: String,
    pub signature_hex: String,
    /// Uncompressed SEC1 public key, so a verifier does not need the private key.
    pub public_key_sec1_hex: String,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_encode(&digest)
}

pub fn sign_pem(private_key_pem: &str, payload: &[u8]) -> anyhow::Result<SignatureBlock> {
    let key = SigningKey::from_pkcs8_pem(private_key_pem)
        .context("parsing device private key for signature")?;
    let signature: Signature = key.sign(payload);
    let point = key.verifying_key().to_encoded_point(false);
    Ok(SignatureBlock {
        algorithm: "ecdsa-p256".into(),
        digest_sha256: sha256_hex(payload),
        signature_hex: hex_encode(&signature.to_bytes()),
        public_key_sec1_hex: hex_encode(point.as_bytes()),
    })
}

pub fn verify(block: &SignatureBlock, payload: &[u8]) -> anyhow::Result<()> {
    if block.algorithm != "ecdsa-p256" {
        anyhow::bail!("unsupported signature algorithm {}", block.algorithm);
    }
    if block.digest_sha256 != sha256_hex(payload) {
        anyhow::bail!("digest does not match payload");
    }
    let public_bytes = hex_decode(&block.public_key_sec1_hex).context("public key hex")?;
    let point = EncodedPoint::from_bytes(public_bytes).context("SEC1 public key")?;
    let verifying = VerifyingKey::from_encoded_point(&point)
        .map_err(|_| anyhow::anyhow!("public key is not a P-256 point"))?;
    let sig_bytes = hex_decode(&block.signature_hex).context("signature hex")?;
    let signature = Signature::from_slice(&sig_bytes).context("P-256 signature")?;
    verifying
        .verify(payload, &signature)
        .context("signature verification failed")?;
    Ok(())
}

/// Sign with the on-disk software key when it is a PEM private key.
/// TPM JSON blobs and missing files return `Ok(None)`.
pub fn sign_with_key_file(
    key_file: &str,
    payload: &[u8],
) -> anyhow::Result<Option<SignatureBlock>> {
    let Ok(pem) = std::fs::read_to_string(key_file) else {
        return Ok(None);
    };
    if !pem.contains("PRIVATE KEY") {
        return Ok(None);
    }
    Ok(Some(sign_pem(&pem, payload)?))
}

pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

pub fn hex_decode(text: &str) -> anyhow::Result<Vec<u8>> {
    if text.len() % 2 != 0 {
        anyhow::bail!("odd hex length");
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_nibble(byte: u8) -> anyhow::Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => anyhow::bail!("invalid hex"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_key_round_trip() {
        let key = rcgen::KeyPair::generate().unwrap();
        let payload = b"{\"deviceId\":\"ZY-1\",\"sequence\":1}";
        let block = sign_pem(&key.serialize_pem(), payload).unwrap();
        verify(&block, payload).unwrap();
        let mut tampered = payload.to_vec();
        tampered[0] ^= 0xff;
        assert!(verify(&block, &tampered).is_err());
    }
}
