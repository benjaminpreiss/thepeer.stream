//! Sole target routing offer codec. Never log or persist its live capability.
//! wpa2: + canonical unpadded base64url of:
//! version(2), flags(relay=2, bound=4, observer=8), pin(32), grant(32),
//! direct, optional relay, bound and observer addresses. Flag 1 is retired.
//! Older decoders reject the new flags; new helpers require a rebuilt phone app.
//! Address = family(4/6), IP octets(4/16), nonzero big-endian port(2).
use super::{Offer, valid_candidate};
use crate::{Failure, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use zeroize::Zeroizing;

const PREFIX: &str = "wpa2:";
const MAX_PACKET: usize = 142;
pub(super) const MAX_TEXT: usize = 195;
const FORMAT: Failure = Failure("route-offer-format");

impl Offer {
    pub const MAX_ENCODED_LEN: usize = MAX_TEXT;

    /// For in-memory QR transfer only. The returned text contains the grant.
    /// Encoding neither renews the grant nor makes it reusable.
    pub fn encode(&self) -> Result<Zeroizing<String>> {
        if !valid_candidate(self.direct)
            || self.bound.is_some_and(|a| !valid_candidate(a))
            || self.observer.is_some_and(|a| !valid_candidate(a))
            || self.relay.is_some_and(|relay| !valid_candidate(relay))
            || self.grant.len() != 64
            || !self
                .grant
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(FORMAT);
        }
        let mut packet = Zeroizing::new(Vec::with_capacity(MAX_PACKET));
        packet.push(2);
        packet.push(
            (u8::from(self.relay.is_some()) << 1)
                | (u8::from(self.bound.is_some()) << 2)
                | (u8::from(self.observer.is_some()) << 3),
        );
        packet.extend_from_slice(&self.pin);
        let mut grant = Zeroizing::new([0; 32]);
        hex::decode_to_slice(self.grant.as_bytes(), grant.as_mut()).map_err(|_| FORMAT)?;
        packet.extend_from_slice(grant.as_ref());
        put_address(&mut packet, self.direct);
        if let Some(relay) = self.relay {
            put_address(&mut packet, relay);
        }
        if let Some(bound) = self.bound {
            put_address(&mut packet, bound);
        }
        if let Some(observer) = self.observer {
            put_address(&mut packet, observer);
        }
        let mut text = Zeroizing::new(String::with_capacity(MAX_TEXT));
        text.push_str(PREFIX);
        URL_SAFE_NO_PAD.encode_string(&packet[..], &mut text);
        Ok(text)
    }

    /// Pure, bounded decoding of untrusted input. No DNS, connection or action.
    /// Caller owns and must clear the borrowed input. Only wpa2 is accepted;
    /// legacy formats, alternate encodings and trailing bytes are not negotiated.
    /// Parsing does not authenticate the computer or authorize the grant.
    pub fn decode(text: &str) -> Result<Self> {
        if !(103..=MAX_TEXT).contains(&text.len()) {
            return Err(FORMAT);
        }
        let encoded = text.strip_prefix(PREFIX).ok_or(FORMAT)?;
        let mut packet = Zeroizing::new([0; MAX_PACKET]);
        let len = URL_SAFE_NO_PAD
            .decode_slice(encoded, packet.as_mut())
            .map_err(|_| FORMAT)?;
        let mut rest = &packet[..len];
        let header = take(&mut rest, 2)?;
        if header[0] != 2 || header[1] & !14 != 0 {
            return Err(FORMAT);
        }
        let flags = header[1];
        let pin = take(&mut rest, 32)?.try_into().map_err(|_| FORMAT)?;
        let grant = Zeroizing::new(hex::encode(take(&mut rest, 32)?));
        let direct = address(&mut rest)?;
        let relay = if flags & 2 != 0 {
            Some(address(&mut rest)?)
        } else {
            None
        };
        let bound = if flags & 4 != 0 {
            Some(address(&mut rest)?)
        } else {
            None
        };
        let observer = if flags & 8 != 0 {
            Some(address(&mut rest)?)
        } else {
            None
        };
        if !rest.is_empty() {
            return Err(FORMAT);
        }
        Ok(Self {
            direct,
            bound,
            observer,
            pin,
            grant,
            relay,
        })
    }
}

fn take<'a>(rest: &mut &'a [u8], len: usize) -> Result<&'a [u8]> {
    let (head, tail) = rest.split_at_checked(len).ok_or(FORMAT)?;
    *rest = tail;
    Ok(head)
}
fn address(rest: &mut &[u8]) -> Result<SocketAddr> {
    let family = take(rest, 1)?[0];
    let ip = match family {
        4 => IpAddr::V4(Ipv4Addr::from(
            <[u8; 4]>::try_from(take(rest, 4)?).map_err(|_| FORMAT)?,
        )),
        6 => IpAddr::V6(Ipv6Addr::from(
            <[u8; 16]>::try_from(take(rest, 16)?).map_err(|_| FORMAT)?,
        )),
        _ => return Err(FORMAT),
    };
    let port = u16::from_be_bytes(take(rest, 2)?.try_into().map_err(|_| FORMAT)?);
    let address = SocketAddr::new(ip, port);
    if !valid_candidate(address) {
        return Err(FORMAT);
    }
    Ok(address)
}
fn put_address(packet: &mut Vec<u8>, address: SocketAddr) {
    match address.ip() {
        IpAddr::V4(ip) => {
            packet.push(4);
            packet.extend_from_slice(&ip.octets());
        }
        IpAddr::V6(ip) => {
            packet.push(6);
            packet.extend_from_slice(&ip.octets());
        }
    }
    packet.extend_from_slice(&address.port().to_be_bytes());
}
