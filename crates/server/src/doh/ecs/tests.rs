use super::*;
use crate::doh::{
    tests::{answer, wire_query},
    validate_message,
};

fn opt_query(options: &[u8]) -> Bytes {
    let mut wire = wire_query().to_vec();
    append_opt(&mut wire, options);
    Bytes::from(wire)
}

fn append_opt(wire: &mut Vec<u8>, options: &[u8]) {
    let count = u16::from_be_bytes([wire[10], wire[11]]) + 1;
    wire[10..12].copy_from_slice(&count.to_be_bytes());
    wire.extend_from_slice(&[0, 0, 41, 4, 208, 0, 0, 0x80, 0]);
    wire.extend_from_slice(&(options.len() as u16).to_be_bytes());
    wire.extend_from_slice(options);
}

fn ecs_data(wire: &[u8]) -> Option<&[u8]> {
    let range = inspect(wire)?.opt?.ecs?;
    Some(&wire[range])
}

fn response_for(query: &[u8]) -> Bytes {
    let mut wire = answer();
    if let Some(opt) = inspect(query).unwrap().opt {
        wire[11] = 1;
        wire.extend_from_slice(&query[opt.start..opt.data.end]);
    }
    Bytes::from(wire)
}

#[test]
fn adds_masked_ipv4_ipv6_and_mapped_ipv4_without_changing_question() {
    for (ip, expected) in [
        ("8.8.8.129", vec![0, 1, 24, 0, 8, 8, 8]),
        (
            "2606:4700:1234:5678::abcd",
            vec![0, 2, 56, 0, 0x26, 6, 0x47, 0, 0x12, 0x34, 0x56],
        ),
        ("::ffff:8.8.8.129", vec![0, 1, 24, 0, 8, 8, 8]),
    ] {
        let original = wire_query();
        let query = prepare(original.clone(), ip.parse().unwrap()).unwrap();
        assert!(query.added);
        assert_eq!(ecs_data(&query.wire).unwrap(), expected);
        assert_eq!(
            validate_message(&query.wire, false),
            validate_message(&original, false)
        );
        assert_eq!(&query.wire[..10], &original[..10]);
        assert_eq!(&query.wire[12..original.len()], &original[12..]);
        assert_eq!(&query.wire[10..12], &[0, 1]);
    }
}

#[test]
fn retains_edns_flags_cookies_and_existing_ecs_including_privacy_opt_out() {
    let cookie = [0, 10, 0, 8, 1, 2, 3, 4, 5, 6, 7, 8];
    let original = opt_query(&cookie);
    let query = prepare(original.clone(), "8.8.8.9".parse().unwrap()).unwrap();
    assert!(query.added && query.had_opt);
    let opt = inspect(&query.wire).unwrap().opt.unwrap();
    assert_eq!(
        &query.wire[opt.fixed + 2..opt.fixed + 8],
        &original[opt.fixed + 2..opt.fixed + 8]
    );
    assert_eq!(
        &query.wire[opt.data.start..opt.data.start + cookie.len()],
        &cookie
    );
    assert!(validate_message(&query.wire, false).is_ok());
    for options in [
        vec![0, 8, 0, 6, 0, 1, 16, 0, 1, 1],
        vec![0, 8, 0, 4, 0, 1, 0, 0],
        vec![0, 8, 0, 4, 0, 2, 0, 0],
    ] {
        let original = opt_query(&options);
        let query = prepare(original.clone(), "8.8.8.9".parse().unwrap()).unwrap();
        assert!(!query.added);
        assert_eq!(query.wire, original);
    }
}

#[test]
fn does_not_add_nonpublic_or_missing_transport_addresses() {
    for ip in [
        "0.0.0.0",
        "127.0.0.1",
        "10.0.0.1",
        "100.64.0.1",
        "169.254.169.254",
        "192.0.2.1",
        "198.51.100.1",
        "203.0.113.1",
        "::1",
        "fc00::1",
        "fe80::1",
        "2001:db8::1",
    ] {
        let query = prepare(wire_query(), ip.parse().unwrap()).unwrap();
        assert_eq!(query.wire, wire_query(), "{ip}");
        assert!(!query.added);
    }
}

#[test]
fn rejects_malformed_and_duplicate_edns_or_ecs_options() {
    for options in [
        vec![0, 8, 0],
        vec![0, 8, 0, 4, 0, 1],
        vec![0, 8, 0, 4, 0, 1, 24, 0],
        vec![0, 8, 0, 7, 0, 1, 24, 1, 8, 8, 8],
        vec![0, 8, 0, 6, 0, 1, 13, 0, 8, 1],
        vec![0, 8, 0, 4, 0, 1, 0, 0, 0, 8, 0, 4, 0, 1, 0, 0],
    ] {
        assert!(
            prepare(opt_query(&options), "8.8.8.8".parse().unwrap()).is_err(),
            "{options:?}"
        );
    }
    let mut duplicate = opt_query(&[]).to_vec();
    append_opt(&mut duplicate, &[]);
    assert!(prepare(Bytes::from(duplicate), "8.8.8.8".parse().unwrap()).is_err());
}

#[test]
fn preserves_signed_nonfinal_opt_unknown_edns_versions_and_size_limits() {
    for kind in [24u16, 249, 250] {
        let mut wire = wire_query().to_vec();
        wire[11] = 1;
        wire.extend_from_slice(&[0]);
        wire.extend_from_slice(&kind.to_be_bytes());
        wire.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]);
        let query = prepare(Bytes::from(wire.clone()), "8.8.8.8".parse().unwrap()).unwrap();
        assert_eq!(query.wire, wire);
    }
    let mut nonfinal = opt_query(&[]).to_vec();
    nonfinal[11] += 1;
    nonfinal.extend_from_slice(&[0xc0, 12, 0, 16, 0, 1, 0, 0, 0, 0, 0, 0]);
    let mut version = opt_query(&[]).to_vec();
    let fixed = inspect(&version).unwrap().opt.unwrap().fixed;
    version[fixed + 5] = 1;
    let size = MAX_MESSAGE - wire_query().len() - 11 - 4;
    let mut padding = vec![0, 12];
    padding.extend_from_slice(&(size as u16).to_be_bytes());
    padding.resize(size + 4, 0);
    let full = opt_query(&padding);
    assert_eq!(full.len(), MAX_MESSAGE);
    for wire in [Bytes::from(nonfinal), Bytes::from(version), full] {
        assert!(validate_message(&wire, false).is_ok());
        let query = prepare(wire.clone(), "8.8.8.8".parse().unwrap()).unwrap();
        assert!(!query.added);
        assert_eq!(query.wire, wire);
    }
}

#[test]
fn validates_response_network_and_allows_resolvers_that_ignore_ecs() {
    let query = prepare(wire_query(), "8.8.8.9".parse().unwrap()).unwrap();
    let response = response_for(&query.wire);
    assert!(response_matches(&query.wire, &response));
    assert!(response_matches(&query.wire, &answer()));
    assert!(!response_matches(&wire_query(), &response));
    let range = inspect(&response).unwrap().opt.unwrap().ecs.unwrap();
    for offset in [0, 1, 2, 4, 5, 6] {
        let mut invalid = response.to_vec();
        invalid[range.start + offset] ^= 1;
        assert!(!response_matches(&query.wire, &invalid));
    }
    let mut invalid = response.to_vec();
    invalid[range.start + 3] = 33;
    assert!(!response_matches(&query.wire, &invalid));
    let mut unsolicited_signature = response.to_vec();
    unsolicited_signature[11] += 1;
    unsolicited_signature.extend_from_slice(&[0, 0, 250, 0, 1, 0, 0, 0, 0, 0, 0]);
    assert!(!response_matches(&query.wire, &unsolicited_signature));
}

#[test]
fn hides_automatic_ecs_from_clients_but_keeps_raw_cache_answers() {
    for original in [
        wire_query(),
        opt_query(&[0, 10, 0, 8, 1, 2, 3, 4, 5, 6, 7, 8]),
    ] {
        let query = prepare(original.clone(), "8.8.8.9".parse().unwrap()).unwrap();
        let raw = response_for(&query.wire);
        let result = query.for_client(raw.clone()).unwrap();
        assert!(validate_message(&result, true).is_ok());
        assert!(ecs_data(&result).is_none());
        assert!(ecs_data(&raw).is_some());
        assert_eq!(inspect(&result).unwrap().opt.is_some(), query.had_opt);
        if query.had_opt {
            let opt = inspect(&result).unwrap().opt.unwrap();
            assert_eq!(&result[opt.data], &[0, 10, 0, 8, 1, 2, 3, 4, 5, 6, 7, 8]);
        } else {
            assert_eq!(result, answer());
        }
        // A caller explicitly sending the identical ECS receives its echo.
        let explicit = prepare(query.wire.clone(), "9.9.9.9".parse().unwrap()).unwrap();
        assert_eq!(explicit.for_client(raw.clone()).unwrap(), raw);
    }
}

#[test]
fn stripping_nonfinal_response_ecs_preserves_compression_offsets() {
    let query = prepare(opt_query(&[]), "8.8.8.9".parse().unwrap()).unwrap();
    let mut raw = response_for(&query.wire).to_vec();
    raw[11] += 1;
    raw.extend_from_slice(&[0xc0, 12, 0, 16, 0, 1, 0, 0, 0, 0, 0, 0]);
    let result = query.for_client(Bytes::from(raw.clone())).unwrap();
    assert_eq!(result.len(), raw.len());
    assert!(validate_message(&result, true).is_ok());
    assert!(ecs_data(&result).is_none());
    let opt = inspect(&result).unwrap().opt.unwrap();
    assert_eq!(&result[opt.data.start..opt.data.start + 4], &[0, 12, 0, 7]);
    assert!(result[opt.data.start + 4..opt.data.end]
        .iter()
        .all(|byte| *byte == 0));
    let no_edns = prepare(wire_query(), "8.8.8.9".parse().unwrap()).unwrap();
    assert!(no_edns.for_client(Bytes::from(raw)).is_none());
}
