// SPDX-License-Identifier: Apache-2.0

//! TPM2-backed identity (`identity.backend = "tpm"`, `--features tpm2`).
//!
//! The private key is generated inside the TPM (`TransientKeyContext::create_key`)
//! and never leaves it in plaintext: what's persisted to `auth.mtls.key_file` is a
//! small JSON envelope (TPM `KeyMaterial` - a public key plus an *encrypted* private
//! blob only the same TPM can unwrap - and the key's auth value), not a raw PKCS#8
//! key. Every sign operation re-opens a `TransientKeyContext` against the configured
//! TCTI and asks the TPM to sign; the key's sensitive area is decrypted by the TPM
//! itself and never touches this process's memory in usable form.
//!
//! Fixed to ECC NIST P-256 with ECDSA/SHA-256 - the combination every TPM2 chip and
//! `swtpm` supports well - rather than exposing a wider algorithm choice here.

use std::sync::Arc;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tss_esapi::{
    abstraction::transient::{KeyMaterial, KeyParams, TransientKeyContextBuilder},
    interface_types::{algorithm::EccSchemeAlgorithm, algorithm::HashingAlgorithm, ecc::EccCurve},
    structures::{Auth, Digest, EccScheme, Signature},
    tcti_ldr::TctiNameConf,
    utils::PublicKey as TpmPublicKey,
};

fn key_params() -> anyhow::Result<KeyParams> {
    let scheme = EccScheme::create(
        EccSchemeAlgorithm::EcDsa,
        Some(HashingAlgorithm::Sha256),
        None,
    )
    .map_err(|error| anyhow::anyhow!("building TPM ECC signing scheme: {error}"))?;
    Ok(KeyParams::Ecc {
        curve: EccCurve::NistP256,
        scheme,
    })
}

fn resolve_tcti(tcti: &str) -> anyhow::Result<TctiNameConf> {
    if tcti.is_empty() {
        TctiNameConf::from_environment_variable()
            .context("resolving the default TCTI (set identity.tpm_tcti to name one explicitly)")
    } else {
        tcti.parse::<TctiNameConf>()
            .map_err(|error| anyhow::anyhow!("invalid identity.tpm_tcti {tcti:?}: {error:?}"))
    }
}

fn left_pad_32(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let take = bytes.len().min(32);
    let start = bytes.len() - take;
    out[32 - take..].copy_from_slice(&bytes[start..]);
    out
}

/// Minimal, correct DER encoding of an ECDSA-Sig-Value: `SEQUENCE { r, s }`,
/// each an ASN.1 INTEGER. X.509/CSR signatures require this format; the TPM
/// only gives us the raw big-endian `r`/`s` components.
fn der_encode_ecdsa_signature(r: &[u8], s: &[u8]) -> Vec<u8> {
    fn der_len(len: usize) -> Vec<u8> {
        if len < 0x80 {
            return vec![len as u8];
        }
        let be = len.to_be_bytes();
        let first_nonzero = be.iter().position(|&b| b != 0).unwrap_or(be.len() - 1);
        let trimmed = &be[first_nonzero..];
        let mut out = vec![0x80 | trimmed.len() as u8];
        out.extend_from_slice(trimmed);
        out
    }
    fn der_integer(bytes: &[u8]) -> Vec<u8> {
        let mut b = bytes;
        while b.len() > 1 && b[0] == 0 {
            b = &b[1..];
        }
        let mut content = Vec::new();
        if b.first().is_some_and(|byte| byte & 0x80 != 0) {
            content.push(0);
        }
        content.extend_from_slice(b);
        let mut out = vec![0x02u8];
        out.extend(der_len(content.len()));
        out.extend(content);
        out
    }
    let r_der = der_integer(r);
    let s_der = der_integer(s);
    let mut content = Vec::with_capacity(r_der.len() + s_der.len());
    content.extend(r_der);
    content.extend(s_der);
    let mut out = vec![0x30u8];
    out.extend(der_len(content.len()));
    out.extend(content);
    out
}

#[derive(Serialize, Deserialize)]
struct TpmKeyFile {
    backend: String,
    key_material: KeyMaterial,
    auth_bytes: Vec<u8>,
}

#[derive(Debug)]
struct TpmKeyInner {
    tcti: String,
    key_material: KeyMaterial,
    auth: Option<Auth>,
    /// Cached uncompressed SEC1 point (`0x04 || X || Y`), so
    /// `rcgen::PublicKeyData::der_bytes` can return a plain `&[u8]`.
    public_point: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TpmKey(Arc<TpmKeyInner>);

impl TpmKey {
    pub fn generate(tcti: &str) -> anyhow::Result<Self> {
        let tcti_conf = resolve_tcti(tcti)?;
        let mut ctx = TransientKeyContextBuilder::new()
            .with_tcti(tcti_conf)
            .build()
            .map_err(|error| anyhow::anyhow!("opening TPM context: {error}"))?;
        let (key_material, auth) = ctx
            .create_key(key_params()?, 32)
            .map_err(|error| anyhow::anyhow!("creating TPM signing key: {error}"))?;
        let public_point = public_point_of(&key_material)?;
        Ok(Self(Arc::new(TpmKeyInner {
            tcti: tcti.to_string(),
            key_material,
            auth,
            public_point,
        })))
    }

    pub fn load(file_contents: &str, tcti: &str) -> anyhow::Result<Self> {
        let file: TpmKeyFile =
            serde_json::from_str(file_contents).context("parsing TPM key file")?;
        let auth = if file.auth_bytes.is_empty() {
            None
        } else {
            Some(
                Auth::try_from(file.auth_bytes)
                    .map_err(|error| anyhow::anyhow!("invalid TPM key auth value: {error}"))?,
            )
        };
        let public_point = public_point_of(&file.key_material)?;
        Ok(Self(Arc::new(TpmKeyInner {
            tcti: tcti.to_string(),
            key_material: file.key_material,
            auth,
            public_point,
        })))
    }

    pub fn serialize_json(&self) -> anyhow::Result<String> {
        let file = TpmKeyFile {
            backend: "tpm".to_string(),
            key_material: self.0.key_material.clone(),
            auth_bytes: self
                .0
                .auth
                .as_ref()
                .map(|auth| auth.value().to_vec())
                .unwrap_or_default(),
        };
        serde_json::to_string_pretty(&file).context("serializing TPM key file")
    }

    fn sign_der(&self, msg: &[u8]) -> anyhow::Result<Vec<u8>> {
        let hash: [u8; 32] = Sha256::digest(msg).into();
        let tcti_conf = resolve_tcti(&self.0.tcti)?;
        let mut ctx = TransientKeyContextBuilder::new()
            .with_tcti(tcti_conf)
            .build()
            .map_err(|error| anyhow::anyhow!("opening TPM context: {error}"))?;
        let digest = Digest::try_from(hash.to_vec())
            .map_err(|error| anyhow::anyhow!("building TPM digest: {error}"))?;
        let signature = ctx
            .sign(
                self.0.key_material.clone(),
                key_params()?,
                self.0.auth.clone(),
                digest,
            )
            .map_err(|error| anyhow::anyhow!("TPM sign operation failed: {error}"))?;
        let Signature::EcDsa(ecc_signature) = signature else {
            bail!("TPM returned an unexpected signature type for an ECDSA key");
        };
        Ok(der_encode_ecdsa_signature(
            ecc_signature.signature_r().value(),
            ecc_signature.signature_s().value(),
        ))
    }
}

fn public_point_of(key_material: &KeyMaterial) -> anyhow::Result<Vec<u8>> {
    let TpmPublicKey::Ecc { x, y } = key_material.public() else {
        bail!("TPM key material is not an ECC key");
    };
    let mut point = Vec::with_capacity(65);
    point.push(0x04);
    point.extend(left_pad_32(x));
    point.extend(left_pad_32(y));
    Ok(point)
}

impl rcgen::PublicKeyData for TpmKey {
    fn der_bytes(&self) -> &[u8] {
        &self.0.public_point
    }

    fn algorithm(&self) -> &'static rcgen::SignatureAlgorithm {
        &rcgen::PKCS_ECDSA_P256_SHA256
    }
}

impl rcgen::SigningKey for TpmKey {
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, rcgen::Error> {
        self.sign_der(msg).map_err(|error| {
            tracing::error!(%error, "TPM signing failed while generating a CSR");
            rcgen::Error::RemoteKeyError
        })
    }
}

impl rustls::sign::SigningKey for TpmKey {
    fn choose_scheme(
        &self,
        offered: &[rustls::SignatureScheme],
    ) -> Option<Box<dyn rustls::sign::Signer>> {
        offered
            .contains(&rustls::SignatureScheme::ECDSA_NISTP256_SHA256)
            .then(|| Box::new(self.clone()) as Box<dyn rustls::sign::Signer>)
    }

    fn public_key(&self) -> Option<rustls::pki_types::SubjectPublicKeyInfoDer<'_>> {
        Some(rustls::sign::public_key_to_spki(
            &rustls::pki_types::alg_id::ECDSA_P256,
            &self.0.public_point,
        ))
    }

    fn algorithm(&self) -> rustls::SignatureAlgorithm {
        rustls::SignatureAlgorithm::ECDSA
    }
}

impl rustls::sign::Signer for TpmKey {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, rustls::Error> {
        self.sign_der(message).map_err(|error| {
            tracing::error!(%error, "TPM signing failed during a TLS handshake");
            rustls::Error::General("TPM signing failed".to_string())
        })
    }

    fn scheme(&self) -> rustls::SignatureScheme {
        rustls::SignatureScheme::ECDSA_NISTP256_SHA256
    }
}
