//! ECS is added only for a verified public transport address. The original
//! client's EDNS capabilities and explicit privacy choices remain authoritative.
use std::{net::IpAddr, ops::Range};

use bytes::Bytes;

use super::{name, MAX_MESSAGE};

pub(super) struct Query {
    pub wire: Bytes,
    added: bool,
    had_opt: bool,
}

struct Opt {
    start: usize,
    fixed: usize,
    data: Range<usize>,
    ecs: Option<Range<usize>>,
}

struct Message {
    opt: Option<Opt>,
    signed: bool,
}

fn inspect(packet: &[u8]) -> Option<Message> {
    let mut cursor = 12;
    name(packet, &mut cursor).ok()?;
    cursor += 4;
    let answers = u16::from_be_bytes(packet.get(6..8)?.try_into().ok()?) as usize;
    let authorities = u16::from_be_bytes(packet.get(8..10)?.try_into().ok()?) as usize;
    let additional = u16::from_be_bytes(packet.get(10..12)?.try_into().ok()?) as usize;
    let mut message = Message {
        opt: None,
        signed: false,
    };
    for index in 0..answers + authorities + additional {
        let start = cursor;
        let owner = name(packet, &mut cursor).ok()?;
        let fixed = cursor;
        let fields = packet.get(fixed..fixed + 10)?;
        let kind = u16::from_be_bytes([fields[0], fields[1]]);
        let end = fixed + 10 + u16::from_be_bytes([fields[8], fields[9]]) as usize;
        packet.get(fixed + 10..end)?;
        message.signed |= matches!(kind, 24 | 249 | 250);
        if kind == 41 {
            if message.opt.is_some() || owner != [0] || index < answers + authorities {
                return None;
            }
            let mut opt = Opt {
                start,
                fixed,
                data: fixed + 10..end,
                ecs: None,
            };
            let mut option = opt.data.start;
            while option < end {
                let header = packet.get(option..option + 4)?;
                let length = u16::from_be_bytes([header[2], header[3]]) as usize;
                let next = option + 4 + length;
                if next > end {
                    return None;
                }
                if header[..2] == [0, 8] {
                    if opt.ecs.is_some() {
                        return None;
                    }
                    opt.ecs = Some(option + 4..next);
                }
                option = next;
            }
            message.opt = Some(opt);
        }
        cursor = end;
    }
    (cursor == packet.len()).then_some(message)
}

pub(super) fn authenticated(packet: &[u8]) -> bool {
    inspect(packet).is_some_and(|message| message.signed)
}

fn valid_ecs(data: &[u8], query: bool) -> bool {
    if data.len() < 4 {
        return false;
    }
    let max = match data[..2] {
        [0, 1] => 32,
        [0, 2] => 128,
        _ => return false,
    };
    let prefix = data[2];
    prefix <= max
        && data[3] <= max
        && (!query || data[3] == 0)
        && data.len() == 4 + (prefix as usize).div_ceil(8)
        && (prefix.is_multiple_of(8)
            || data
                .last()
                .is_some_and(|last| last & ((1 << (8 - prefix % 8)) - 1) == 0))
}

pub(super) fn prepare(wire: Bytes, ip: IpAddr) -> Result<Query, ()> {
    let message = inspect(&wire).ok_or(())?;
    let had_opt = message.opt.is_some();
    if let Some(data) = message.opt.as_ref().and_then(|opt| opt.ecs.as_ref()) {
        if !valid_ecs(&wire[data.clone()], true) {
            return Err(());
        }
        // Includes /0: never replace a client's explicit opt-out with its IP.
        return Ok(Query {
            wire,
            added: false,
            had_opt,
        });
    }
    let ip = match ip {
        IpAddr::V6(ip) => ip.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip.into()),
        other => other,
    };
    if !crate::download::public_ip(ip)
        || message.signed
        || message
            .opt
            .as_ref()
            .is_some_and(|opt| opt.data.end != wire.len() || wire[opt.fixed + 5] != 0)
    {
        // Inserting bytes ahead of later records could invalidate compressed
        // names, while changing a signed message would invalidate its MAC.
        return Ok(Query {
            wire,
            added: false,
            had_opt,
        });
    }
    let mut option = vec![0, 8];
    match ip {
        IpAddr::V4(ip) => {
            option.extend_from_slice(&[0, 7, 0, 1, 24, 0]);
            option.extend_from_slice(&ip.octets()[..3]);
        }
        IpAddr::V6(ip) => {
            option.extend_from_slice(&[0, 11, 0, 2, 56, 0]);
            option.extend_from_slice(&ip.octets()[..7]);
        }
    }
    let growth = option.len() + if had_opt { 0 } else { 11 };
    if wire.len() + growth > MAX_MESSAGE {
        return Ok(Query {
            wire,
            added: false,
            had_opt,
        });
    }
    let mut packet = wire.to_vec();
    if let Some(opt) = message.opt {
        let length = u16::try_from(opt.data.len() + option.len()).map_err(|_| ())?;
        packet[opt.fixed + 8..opt.fixed + 10].copy_from_slice(&length.to_be_bytes());
    } else {
        let count = u16::from_be_bytes([packet[10], packet[11]])
            .checked_add(1)
            .ok_or(())?;
        packet[10..12].copy_from_slice(&count.to_be_bytes());
        // EDNS0, 1232-byte advertised UDP payload, no extended flags.
        packet.extend_from_slice(&[0, 0, 41, 4, 208, 0, 0, 0, 0]);
        packet.extend_from_slice(&(option.len() as u16).to_be_bytes());
    }
    packet.extend_from_slice(&option);
    Ok(Query {
        wire: Bytes::from(packet),
        added: true,
        had_opt,
    })
}

/// A response cannot assert that it belongs to a different client network.
/// Resolvers that ignore ECS and omit it in the answer remain compatible.
pub(super) fn response_matches(query: &[u8], response: &[u8]) -> bool {
    let Some(request) = inspect(query) else {
        return false;
    };
    let Some(answer) = inspect(response) else {
        return false;
    };
    if answer.signed && !request.signed {
        return false;
    }
    let requested = request.opt.and_then(|opt| opt.ecs).map(|data| &query[data]);
    let returned = answer
        .opt
        .and_then(|opt| opt.ecs)
        .map(|data| &response[data]);
    match (requested, returned) {
        (_, None) => true,
        (Some(expected), Some(actual)) => {
            valid_ecs(actual, false) && expected[..3] == actual[..3] && expected[4..] == actual[4..]
        }
        (None, Some(_)) => false,
    }
}

impl Query {
    /// Keep raw upstream answers in the ECS-aware cache. Sanitize only when
    /// sending to the caller, since explicit and auto-added ECS may share a key.
    pub fn for_client(&self, wire: Bytes) -> Option<Bytes> {
        if !self.added {
            return Some(wire);
        }
        let message = inspect(&wire)?;
        let Some(opt) = message.opt else {
            return Some(wire);
        };
        let mut packet = wire.to_vec();
        if !self.had_opt {
            // Removing a final OPT cannot invalidate offsets in earlier RRs.
            if opt.data.end != packet.len() {
                return None;
            }
            packet.truncate(opt.start);
            let count = u16::from_be_bytes([packet[10], packet[11]]).checked_sub(1)?;
            packet[10..12].copy_from_slice(&count.to_be_bytes());
        } else if let Some(data) = opt.ecs {
            let option = data.start - 4..data.end;
            if opt.data.end == packet.len() {
                let length = opt.data.len() - option.len();
                packet[opt.fixed + 8..opt.fixed + 10]
                    .copy_from_slice(&(length as u16).to_be_bytes());
                packet.drain(option);
            } else {
                // Keep later compression offsets intact by replacing ECS with
                // zero EDNS padding (option 12) of exactly the same length.
                packet[option.start..option.start + 2].copy_from_slice(&12u16.to_be_bytes());
                packet[data].fill(0);
            }
        }
        Some(Bytes::from(packet))
    }
}

#[cfg(test)]
mod tests;
