// SPDX-License-Identifier: Apache-2.0

//! Short-lived stream tickets for browser surfaces that cannot set
//! `Authorization` (`<img>` camera URLs). SSE itself uses `fetch()` with the
//! bearer header. The long-lived bearer is never accepted from a query string.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

const STREAM_EXACT: &[&str] = &["/api/v1/events", "/api/v1/can/frames/stream"];
const STREAM_PREFIX: &str = "/api/v1/camera/";

#[derive(Clone)]
struct Ticket {
    hash: [u8; 32],
    path_prefix: String,
    expires_at_unix_ms: u64,
}

pub struct TicketStore {
    inner: Mutex<Vec<Ticket>>,
}

impl Default for TicketStore {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Vec::new()),
        }
    }
}

impl TicketStore {
    pub fn issue(&self, path_prefix: &str, ttl_seconds: u64) -> Result<String, &'static str> {
        if !issuable(path_prefix) {
            return Err("stream tickets are only issued for SSE and camera paths");
        }
        let raw = random_token();
        let ttl_ms = ttl_seconds.max(1).saturating_mul(1000);
        let ticket = Ticket {
            hash: hash_token(&raw),
            path_prefix: path_prefix.to_string(),
            expires_at_unix_ms: now_ms().saturating_add(ttl_ms),
        };
        self.inner
            .lock()
            .map_err(|_| "ticket store poisoned")?
            .push(ticket);
        Ok(raw)
    }

    pub fn verify(&self, token: &str, path: &str) -> Result<(), &'static str> {
        if !is_stream_path(path) {
            return Err("stream ticket is not valid for this path");
        }
        let hash = hash_token(token);
        let now = now_ms();
        let mut store = self.inner.lock().map_err(|_| "ticket store poisoned")?;
        store.retain(|ticket| ticket.expires_at_unix_ms > now);
        let mut matched = subtle::Choice::from(0u8);
        for ticket in store.iter() {
            let hash_ok = ticket.hash.ct_eq(&hash);
            let path_ok = path == ticket.path_prefix || path.starts_with(&ticket.path_prefix);
            if bool::from(hash_ok) && path_ok {
                matched = subtle::Choice::from(1);
            }
        }
        if bool::from(matched) {
            Ok(())
        } else {
            Err("invalid or expired stream ticket")
        }
    }
}

pub fn is_stream_path(path: &str) -> bool {
    STREAM_EXACT.contains(&path) || path.starts_with(STREAM_PREFIX)
}

fn issuable(path_prefix: &str) -> bool {
    is_stream_path(path_prefix) || path_prefix == STREAM_PREFIX
}

fn hash_token(token: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hasher.finalize().into()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    if let Ok(mut file) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        if file.read_exact(&mut bytes).is_ok() {
            return bytes.iter().map(|b| format!("{b:02x}")).collect();
        }
    }
    getrandom_or_time(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn getrandom_or_time(bytes: &mut [u8; 32]) {
    // Avoid a new RNG crate: mix the clock with the address of the buffer.
    // Tickets are short-lived and hashed at rest; this is not a long-term key.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mixed = nanos ^ (bytes.as_ptr() as u128) ^ (std::process::id() as u128);
    for (i, slot) in bytes.iter_mut().enumerate() {
        let shift = (i % 16) * 8;
        *slot = ((mixed >> shift) as u8).wrapping_add(i as u8);
    }
    // Stir so two tickets issued in the same nanosecond still differ.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    bytes[0] ^= n as u8;
    bytes[1] ^= (n >> 8) as u8;
    bytes[2] ^= (n >> 16) as u8;
    bytes[3] ^= (n >> 24) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_authorizes_only_its_path() {
        let store = TicketStore::default();
        let token = store.issue("/api/v1/camera/dock/stream", 60).unwrap();
        assert!(store.verify(&token, "/api/v1/camera/dock/stream").is_ok());
        assert!(store.verify(&token, "/api/v1/inventory").is_err());
        assert!(store.issue("/api/v1/inventory", 60).is_err());
    }

    #[test]
    fn expired_ticket_is_rejected() {
        let store = TicketStore::default();
        let token = store.issue("/api/v1/events", 60).unwrap();
        store.inner.lock().unwrap()[0].expires_at_unix_ms = 1;
        assert!(store.verify(&token, "/api/v1/events").is_err());
    }
}
