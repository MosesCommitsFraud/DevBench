//! Wake-on-LAN: sends the "magic packet" (6 × 0xFF, then the MAC 16 times) as a UDP broadcast.

use anyhow::{Context, Result, bail};

pub fn parse_mac(text: &str) -> Result<[u8; 6]> {
    let hex: String = text.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let seps_ok = text.chars().all(|c| c.is_ascii_hexdigit() || matches!(c, ':' | '-' | '.' | ' '));
    if hex.len() != 12 || !seps_ok {
        bail!("'{text}' is not a MAC address (expected e.g. AA:BB:CC:DD:EE:FF)");
    }
    let mut mac = [0u8; 6];
    for (i, b) in mac.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)?;
    }
    Ok(mac)
}

pub fn magic_packet(mac: [u8; 6]) -> Vec<u8> {
    let mut p = vec![0xFF; 6];
    for _ in 0..16 {
        p.extend_from_slice(&mac);
    }
    p
}

pub async fn wake(mac: &str, broadcast: &str, port: u16) -> Result<()> {
    let packet = magic_packet(parse_mac(mac)?);
    let target = format!("{}:{}", broadcast.trim(), port);
    let socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;
    // A few copies: UDP may drop one, and some NICs only listen briefly after power-off.
    for _ in 0..3 {
        socket
            .send_to(&packet, &target)
            .await
            .with_context(|| format!("sending the magic packet to {target}"))?;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_formats() {
        let want = [0xAA, 0xBB, 0xCC, 0x01, 0x02, 0x03];
        assert_eq!(parse_mac("aa:bb:cc:01:02:03").unwrap(), want);
        assert_eq!(parse_mac("AA-BB-CC-01-02-03").unwrap(), want);
        assert_eq!(parse_mac("aabb.cc01.0203").unwrap(), want);
        assert!(parse_mac("aa:bb:cc:01:02").is_err());
        assert!(parse_mac("zz:bb:cc:01:02:03").is_err());
    }

    #[test]
    fn packet_layout() {
        let p = magic_packet([1, 2, 3, 4, 5, 6]);
        assert_eq!(p.len(), 102);
        assert_eq!(&p[..6], &[0xFF; 6]);
        assert_eq!(&p[96..], &[1, 2, 3, 4, 5, 6]);
    }
}
