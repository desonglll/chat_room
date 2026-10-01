//! TG-408 SSRF policy for link-preview fetches. Every hop is checked here before any socket is
//! opened: the scheme, the port, the absence of credentials, and **every** address the host
//! resolves to. The fetcher then connects to the address checked here (pinned), so a DNS answer
//! that changes between the check and the connection (rebinding) cannot reach anything else.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use reqwest::Url;

/// Ports a preview may be fetched from.
pub const ALLOWED_PORTS: [u16; 4] = [80, 443, 8080, 8443];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Scheme,
    Port,
    Credentials,
    NoHost,
    /// The host resolved to (or is) an address outside the public internet.
    PrivateAddress(IpAddr),
    Unresolvable,
}

/// Whether `ip` is a globally routable unicast address. Anything else — loopback, private,
/// link-local (incl. cloud metadata at 169.254.169.254), CGNAT, multicast, documentation,
/// benchmarking, reserved, unique-local, and IPv4 embedded in IPv6 forms of those — is refused.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => is_public_v6(v6),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || a == 0
        || (a == 100 && (64..128).contains(&b)) // CGNAT 100.64/10
        || (a == 192 && b == 0 && c == 0) // IETF protocol assignments 192.0.0/24
        || (a == 198 && (b == 18 || b == 19)) // benchmarking 198.18/15
        || a >= 240) // reserved 240/4
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_public_v4(v4);
    }
    let segments = ip.segments();
    // NAT64 (64:ff9b::/96) and 6to4 (2002::/16) embed an IPv4 address: judge that address.
    if segments[0] == 0x64 && segments[1] == 0xff9b && segments[2..6] == [0; 4] {
        let [a, b] = segments[6].to_be_bytes();
        let [c, d] = segments[7].to_be_bytes();
        return is_public_v4(Ipv4Addr::new(a, b, c, d));
    }
    if segments[0] == 0x2002 {
        let [a, b] = segments[1].to_be_bytes();
        let [c, d] = segments[2].to_be_bytes();
        return is_public_v4(Ipv4Addr::new(a, b, c, d));
    }
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
        || (segments[0] & 0xffc0) == 0xfe80 // link local fe80::/10
        || (segments[0] & 0xffc0) == 0xfec0 // site local (deprecated) fec0::/10
        || (segments[0] == 0x2001 && segments[1] == 0x0db8) // documentation
        || segments[0..6] == [0; 6]) // IPv4-compatible ::a.b.c.d (deprecated)
}

/// What may be reached. Production uses [`Policy::default`]; tests exempt exactly the socket of
/// their local fixture server (and its port), never a range.
#[derive(Debug, Clone)]
pub struct Policy {
    pub ports: Vec<u16>,
    pub exempt: Vec<SocketAddr>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            ports: ALLOWED_PORTS.to_vec(),
            exempt: Vec::new(),
        }
    }
}

impl Policy {
    fn allows(&self, address: SocketAddr) -> bool {
        is_public_ip(address.ip()) || self.exempt.contains(&address)
    }
}

/// The static checks on a URL, before any lookup.
pub fn check_url(url: &Url, policy: &Policy) -> Result<(), Refusal> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Refusal::Scheme);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Refusal::Credentials);
    }
    let port = url.port_or_known_default().ok_or(Refusal::Port)?;
    if !policy.ports.contains(&port) {
        return Err(Refusal::Port);
    }
    let host = url
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or(Refusal::NoHost)?;
    // The URL parser has already normalised decimal/hex/short IPv4 spellings to dotted form.
    match host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
    {
        Ok(ip) if !policy.allows(SocketAddr::new(ip, port)) => Err(Refusal::PrivateAddress(ip)),
        _ => Ok(()),
    }
}

/// Picks the address to connect to from a lookup's answers: refused if **any** answer is not
/// public (a mixed answer is a classic rebinding setup), or if there is none.
pub fn pick_address(answers: &[SocketAddr], policy: &Policy) -> Result<SocketAddr, Refusal> {
    let first = *answers.first().ok_or(Refusal::Unresolvable)?;
    if let Some(bad) = answers.iter().find(|address| !policy.allows(**address)) {
        return Err(Refusal::PrivateAddress(bad.ip()));
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(value: &str) -> Url {
        Url::parse(value).unwrap()
    }

    #[test]
    fn internal_and_metadata_addresses_are_not_public() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "255.255.255.255",
            "224.0.0.1",
            "198.18.0.1",
            "240.0.0.1",
            "::1",
            "::",
            "fc00::1",
            "fd12::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "64:ff9b::a9fe:a9fe",
            "2002:7f00:1::1",
            "2001:db8::1",
        ] {
            assert!(!is_public_ip(ip.parse().unwrap()), "{ip} must be refused");
        }
        for ip in ["93.184.216.34", "1.1.1.1", "2606:4700:4700::1111"] {
            assert!(is_public_ip(ip.parse().unwrap()), "{ip} is public");
        }
    }

    #[test]
    fn only_plain_http_and_https_urls_to_public_hosts_on_standard_ports_pass() {
        assert_eq!(
            check_url(&parsed("file:///etc/passwd"), &Policy::default()),
            Err(Refusal::Scheme)
        );
        assert_eq!(
            check_url(&parsed("gopher://example.com/"), &Policy::default()),
            Err(Refusal::Scheme)
        );
        assert_eq!(
            check_url(&parsed("ftp://example.com/"), &Policy::default()),
            Err(Refusal::Scheme)
        );
        assert_eq!(
            check_url(&parsed("http://user:pw@example.com/"), &Policy::default()),
            Err(Refusal::Credentials)
        );
        assert_eq!(
            check_url(&parsed("http://example.com:22/"), &Policy::default()),
            Err(Refusal::Port)
        );
        assert!(matches!(
            check_url(&parsed("http://127.0.0.1/"), &Policy::default()),
            Err(Refusal::PrivateAddress(_))
        ));
        assert!(matches!(
            check_url(
                &parsed("http://169.254.169.254/latest/meta-data"),
                &Policy::default()
            ),
            Err(Refusal::PrivateAddress(_))
        ));
        assert!(matches!(
            check_url(&parsed("http://[::1]/"), &Policy::default()),
            Err(Refusal::PrivateAddress(_))
        ));
        // Decimal / hex spellings are normalised by the URL parser before the check.
        assert!(matches!(
            check_url(&parsed("http://2130706433/"), &Policy::default()),
            Err(Refusal::PrivateAddress(_))
        ));
        assert!(matches!(
            check_url(&parsed("http://0x7f.1/"), &Policy::default()),
            Err(Refusal::PrivateAddress(_))
        ));
        assert_eq!(
            check_url(&parsed("https://example.com/a?b"), &Policy::default()),
            Ok(())
        );
        assert_eq!(
            check_url(&parsed("http://example.com:8080/"), &Policy::default()),
            Ok(())
        );
    }

    #[test]
    fn a_lookup_with_any_private_answer_is_refused() {
        let public: SocketAddr = "93.184.216.34:443".parse().unwrap();
        let private: SocketAddr = "127.0.0.1:443".parse().unwrap();
        let strict = Policy::default();
        assert_eq!(pick_address(&[public], &strict), Ok(public));
        assert!(matches!(
            pick_address(&[public, private], &strict),
            Err(Refusal::PrivateAddress(_))
        ));
        assert!(matches!(
            pick_address(&[private], &strict),
            Err(Refusal::PrivateAddress(_))
        ));
        assert_eq!(pick_address(&[], &strict), Err(Refusal::Unresolvable));
        let fixture = Policy {
            exempt: vec![private],
            ..Policy::default()
        };
        assert_eq!(pick_address(&[private], &fixture), Ok(private));
        let other: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        assert!(
            pick_address(&[other], &fixture).is_err(),
            "only the exact socket is exempt"
        );
    }
}
