//! DNS wire responses stay in bounded process memory; never in the disk cache.
use std::{collections::HashMap, sync::Mutex, time::Instant};

use axum::http::{header, HeaderMap};
use bytes::Bytes;
use sha2::{Digest, Sha256};

use super::{name, Answer};

const MAX_ENTRIES: usize = 1024;
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct Cache {
    entries: Mutex<HashMap<[u8; 32], Entry>>,
    pub(super) runtime: super::runtime::Runtime,
}

struct Entry {
    wire: Bytes,
    inserted: Instant,
    ttl: u32,
}

pub(super) fn key(upstreams: &[String], query: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(upstreams.len().to_be_bytes());
    for endpoint in upstreams {
        hash.update(endpoint.len().to_be_bytes());
        hash.update(endpoint.as_bytes());
    }
    // Preserve flags, case, QTYPE/QCLASS and every EDNS option (including ECS).
    // Only the transaction ID is independent of the answer.
    hash.update(&query[2..]);
    if super::ecs::authenticated(query) {
        hash.update(&query[..2]);
    }
    hash.finalize().into()
}

impl Cache {
    pub(super) fn get(&self, upstreams: &[String], query: &[u8], now: Instant) -> Option<Bytes> {
        let key = key(upstreams, query);
        let mut entries = self.entries.lock().expect("DoH cache lock poisoned");
        let entry = entries.get(&key)?;
        let elapsed = now.saturating_duration_since(entry.inserted).as_secs();
        if elapsed >= u64::from(entry.ttl) {
            entries.remove(&key);
            return None;
        }
        let mut wire = entry.wire.to_vec();
        age(&mut wire, elapsed as u32)?;
        wire[..2].copy_from_slice(&query[..2]);
        Some(Bytes::from(wire))
    }

    pub(super) fn insert(&self, upstreams: &[String], query: &[u8], answer: &Answer, now: Instant) {
        let Some(ttl) = answer.cache_ttl.filter(|ttl| *ttl > 0) else {
            return;
        };
        let mut entries = self.entries.lock().expect("DoH cache lock poisoned");
        entries.retain(|_, entry| {
            now.saturating_duration_since(entry.inserted).as_secs() < u64::from(entry.ttl)
        });
        let size = answer.wire.len();
        let mut total: usize = entries.values().map(|entry| entry.wire.len()).sum();
        while entries.len() >= MAX_ENTRIES || total + size > MAX_BYTES {
            let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.inserted)
                .map(|(key, _)| *key)
            else {
                return;
            };
            total -= entries.remove(&oldest).unwrap().wire.len();
        }
        entries.insert(
            key(upstreams, query),
            Entry {
                wire: answer.wire.clone(),
                inserted: now,
                ttl,
            },
        );
    }
}

struct Record {
    kind: u16,
    ttl_offset: usize,
    data: std::ops::Range<usize>,
    authority: bool,
}

fn records(wire: &[u8]) -> Option<Vec<Record>> {
    let mut cursor = 12;
    name(wire, &mut cursor).ok()?;
    cursor += 4;
    let answers = u16::from_be_bytes(wire.get(6..8)?.try_into().ok()?) as usize;
    let authorities = u16::from_be_bytes(wire.get(8..10)?.try_into().ok()?) as usize;
    let additional = u16::from_be_bytes(wire.get(10..12)?.try_into().ok()?) as usize;
    let mut records = Vec::new();
    for index in 0..answers + authorities + additional {
        name(wire, &mut cursor).ok()?;
        let fixed = wire.get(cursor..cursor + 10)?;
        let end = cursor + 10 + u16::from_be_bytes([fixed[8], fixed[9]]) as usize;
        wire.get(cursor + 10..end)?;
        records.push(Record {
            kind: u16::from_be_bytes([fixed[0], fixed[1]]),
            ttl_offset: cursor + 4,
            data: cursor + 10..end,
            authority: index >= answers && index < answers + authorities,
        });
        cursor = end;
    }
    Some(records)
}

fn ttl(wire: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(wire[offset..offset + 4].try_into().unwrap())
}

fn age(wire: &mut [u8], seconds: u32) -> Option<()> {
    for record in records(wire)? {
        // OPT's TTL field contains extended RCODE, EDNS version and flags.
        if record.kind != 41 {
            let remaining = ttl(wire, record.ttl_offset).saturating_sub(seconds);
            wire[record.ttl_offset..record.ttl_offset + 4]
                .copy_from_slice(&remaining.to_be_bytes());
        }
    }
    Some(())
}

pub(super) fn prepare(wire: &mut [u8], headers: &HeaderMap) -> Option<u32> {
    // Only successful complete answers or SOA-backed negative responses.
    if wire[2] & 2 != 0 || !matches!(wire[3] & 15, 0 | 3) {
        return None;
    }
    let records = records(wire)?;
    if records.iter().any(|record| {
        matches!(record.kind, 24 | 250)
            || (record.kind == 41 && ttl(wire, record.ttl_offset) >> 24 != 0)
    }) {
        return None;
    }
    let mut lifetime = records
        .iter()
        .filter(|record| record.kind != 41)
        .map(|record| ttl(wire, record.ttl_offset))
        .min()?;
    if wire[3] & 15 == 3 || wire[6..8] == [0, 0] {
        let soa = records
            .iter()
            .find(|record| record.authority && record.kind == 6)?;
        let mut cursor = soa.data.start;
        name(wire, &mut cursor).ok()?;
        name(wire, &mut cursor).ok()?;
        if cursor + 20 != soa.data.end {
            return None;
        }
        lifetime = lifetime.min(ttl(wire, cursor + 16));
    }
    for value in headers.get_all(header::CACHE_CONTROL) {
        for directive in value.to_str().ok()?.split(',') {
            let directive = directive.trim().to_ascii_lowercase();
            let (name, value) = directive.split_once('=').unwrap_or((&directive, ""));
            if matches!(name.trim(), "no-store" | "no-cache" | "private") {
                return None;
            }
            if matches!(name.trim(), "max-age" | "s-maxage") {
                lifetime = lifetime.min(value.trim().trim_matches('"').parse::<u32>().ok()?);
            }
        }
    }
    let elapsed = headers
        .get(header::AGE)
        .map(|value| value.to_str().ok()?.parse::<u32>().ok())
        .unwrap_or(Some(0))?;
    age(wire, elapsed)?;
    let lifetime = lifetime.saturating_sub(elapsed);
    (lifetime > 0).then_some(lifetime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doh::tests::{answer, wire_query};

    fn cached(wire: Vec<u8>) -> Answer {
        let mut wire = wire;
        let cache_ttl = prepare(&mut wire, &HeaderMap::new());
        Answer {
            wire: Bytes::from(wire),
            cache_ttl,
        }
    }

    #[test]
    fn ttl_cache_restores_ids_ages_records_and_expires_at_the_boundary() {
        let cache = Cache::default();
        let upstreams = vec!["https://1.1.1.1/dns-query".into()];
        let now = Instant::now();
        let query = wire_query();
        cache.insert(&upstreams, &query, &cached(answer()), now);
        let mut next = query.to_vec();
        next[..2].copy_from_slice(&[0x12, 0x34]);
        let result = cache
            .get(&upstreams, &next, now + std::time::Duration::from_secs(12))
            .unwrap();
        let mut expected = answer();
        expected[..2].copy_from_slice(&next[..2]);
        age(&mut expected, 12).unwrap();
        assert_eq!(result, expected);
        assert_eq!(ttl(&result, records(&result).unwrap()[0].ttl_offset), 48);
        assert!(cache
            .get(&upstreams, &next, now + std::time::Duration::from_secs(60))
            .is_none());
    }

    #[test]
    fn cache_keys_isolate_query_flags_types_edns_and_upstream_order() {
        let query = wire_query();
        let upstreams = vec![
            "https://1.1.1.1/dns-query".into(),
            "https://dns.google/dns-query".into(),
        ];
        let cache = Cache::default();
        let now = Instant::now();
        cache.insert(&upstreams, &query, &cached(answer()), now);
        let mut reversed = upstreams.clone();
        reversed.reverse();
        assert!(cache.get(&reversed, &query, now).is_none());
        for offset in [2, 3, 15, query.len() - 3, query.len() - 1] {
            let mut other = query.to_vec();
            other[offset] ^= 1;
            assert!(cache.get(&upstreams, &other, now).is_none());
        }
        let mut signed = query.to_vec();
        signed[11] = 1;
        signed.extend_from_slice(&[0, 0, 250, 0, 255, 0, 0, 0, 0, 0, 0]);
        let first = key(&upstreams, &signed);
        signed[0] = 1;
        assert_ne!(first, key(&upstreams, &signed));
        let mut edns = query.to_vec();
        edns[11] = 1;
        edns.extend_from_slice(&[0, 0, 41, 0x10, 0, 0, 0, 0x80, 0, 0, 0]);
        assert!(cache.get(&upstreams, &edns, now).is_none());
    }

    #[test]
    fn http_age_and_max_age_bound_dns_lifetime_without_touching_edns_flags() {
        let mut wire = answer();
        wire[11] = 1;
        wire.extend_from_slice(&[0, 0, 41, 0x10, 0, 0, 0, 0x80, 0, 0, 0]);
        let mut headers = HeaderMap::new();
        headers.insert(header::AGE, "10".parse().unwrap());
        headers.insert(header::CACHE_CONTROL, "max-age=30".parse().unwrap());
        assert_eq!(prepare(&mut wire, &headers), Some(20));
        let records = records(&wire).unwrap();
        assert_eq!(ttl(&wire, records[0].ttl_offset), 50);
        assert_eq!(ttl(&wire, records[1].ttl_offset), 0x8000);
    }

    #[test]
    fn errors_zero_ttl_http_prohibitions_and_authenticated_messages_are_not_cached() {
        for policy in ["no-store", "private", "no-cache=\"answer\"", "max-age=0"] {
            let mut headers = HeaderMap::new();
            headers.insert(header::CACHE_CONTROL, policy.parse().unwrap());
            assert!(prepare(&mut answer(), &headers).is_none(), "{policy}");
        }
        for (offset, value) in [(3, 0x82), (2, 0x83), (wire_query().len() + 9, 0)] {
            let mut wire = answer();
            wire[offset] = value;
            assert!(prepare(&mut wire, &HeaderMap::new()).is_none());
        }
        for kind in [24u16, 250] {
            let mut wire = answer();
            wire[11] = 1;
            wire.push(0);
            wire.extend_from_slice(&kind.to_be_bytes());
            wire.extend_from_slice(&[0, 1, 0, 0, 0, 60, 0, 0]);
            assert!(prepare(&mut wire, &HeaderMap::new()).is_none());
        }
    }

    #[test]
    fn negative_cache_requires_soa_and_uses_its_minimum_ttl() {
        let mut wire = wire_query().to_vec();
        wire[2] = 0x81;
        wire[3] = 0x83;
        assert!(prepare(&mut wire, &HeaderMap::new()).is_none());
        wire[9] = 1;
        wire.extend_from_slice(&[0xc0, 12, 0, 6, 0, 1, 0, 0, 0, 60, 0, 24]);
        wire.extend_from_slice(&[0xc0, 12, 0xc0, 12]);
        for number in [1u32, 3600, 600, 86400, 20] {
            wire.extend_from_slice(&number.to_be_bytes());
        }
        assert_eq!(prepare(&mut wire, &HeaderMap::new()), Some(20));
    }

    #[test]
    fn cache_enforces_entry_and_byte_budgets() {
        let cache = Cache::default();
        let now = Instant::now();
        let response = cached(answer());
        for index in 0..MAX_ENTRIES + 10 {
            let mut query = wire_query().to_vec();
            query.extend_from_slice(&index.to_be_bytes());
            cache.insert(&[], &query, &response, now);
        }
        assert_eq!(cache.entries.lock().unwrap().len(), MAX_ENTRIES);
        let large = Answer {
            wire: Bytes::from(vec![0; 65_535]),
            cache_ttl: Some(60),
        };
        for index in 0..300usize {
            let mut query = wire_query().to_vec();
            query.extend_from_slice(&index.to_be_bytes());
            cache.insert(&[], &query, &large, now);
        }
        assert!(
            cache
                .entries
                .lock()
                .unwrap()
                .values()
                .map(|entry| entry.wire.len())
                .sum::<usize>()
                <= MAX_BYTES
        );
    }
}
