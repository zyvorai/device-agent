// SPDX-License-Identifier: Apache-2.0

//! The default identity backend: a plain PKCS#8 key on disk, generated and
//! signed with by `rcgen`/`ring` directly. Always compiled, unlike the
//! optional `tpm` backend.

use anyhow::Context;

pub struct SoftwareKey {
    pub key_pair: rcgen::KeyPair,
}

impl SoftwareKey {
    pub fn generate() -> anyhow::Result<Self> {
        Ok(Self {
            key_pair: rcgen::KeyPair::generate().context("generating device keypair")?,
        })
    }

    pub fn from_pem(pem: &str) -> anyhow::Result<Self> {
        Ok(Self {
            key_pair: rcgen::KeyPair::from_pem(pem).context("parsing PEM device key")?,
        })
    }

    pub fn serialize_pem(&self) -> String {
        self.key_pair.serialize_pem()
    }
}
