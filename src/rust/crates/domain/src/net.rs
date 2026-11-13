//! Address classification for user-supplied targets (SSRF guard, DEC-139).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;

/// An address that is not to be called on behalf of a user unless an admin allows it:
/// private, loopback, link-local, unspecified, carrier-grade NAT, "this network" (0/8),
/// IETF protocol assignments (192.0.0/24), benchmarking (198.18/15), reserved (240/4, with
/// the broadcast address), multicast, IPv6 site-local and unique local, local-use NAT64
/// (`64:ff9b:1::/48`); and the IPv4 address inside an IPv4-mapped, IPv4-compatible, NAT64
/// (`64:ff9b::/96`) or 6to4 (`2002::/16`) IPv6 address is classified like itself (review
/// SEC2-8: a NAT64 gateway turns `64:ff9b::a9fe:a9fe` into 169.254.169.254).
#[must_use]
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => private_v4(v4),
        IpAddr::V6(v6) => private_v6(v6),
    }
}

fn private_v4(v4: Ipv4Addr) -> bool {
    let [a, b, c, _] = v4.octets();
    v4.is_private()
        || v4.is_loopback()
        || v4.is_link_local()
        || v4.is_multicast()
        // 0.0.0.0/8 "this network" (with the unspecified address)
        || a == 0
        // 100.64.0.0/10 (carrier-grade NAT)
        || (a == 100 && (b & 0xC0) == 64)
        // 192.0.0.0/24 (IETF protocol assignments)
        || (a == 192 && b == 0 && c == 0)
        // 198.18.0.0/15 (benchmarking)
        || (a == 198 && (b & 0xFE) == 18)
        // 240.0.0.0/4 (reserved) and the broadcast address in it
        || a >= 240
}

fn private_v6(v6: Ipv6Addr) -> bool {
    let s = v6.segments();
    let embedded = |hi: u16, lo: u16| {
        let ([o1, o2], [o3, o4]) = (hi.to_be_bytes(), lo.to_be_bytes());
        private_v4(Ipv4Addr::new(o1, o2, o3, o4))
    };
    v6.is_loopback()
        || v6.is_unspecified()
        || v6.is_multicast()
        // fc00::/7 unique local, fe80::/10 link-local, fec0::/10 site-local
        || (s[0] & 0xfe00) == 0xfc00
        || (s[0] & 0xffc0) == 0xfe80
        || (s[0] & 0xffc0) == 0xfec0
        // 64:ff9b:1::/48 local-use NAT64 (RFC 8215): translated to whatever the operator chose
        || (s[0] == 0x64 && s[1] == 0xff9b && s[2] == 1)
        // ::ffff:a.b.c.d (mapped), ::a.b.c.d (compatible), 64:ff9b::a.b.c.d (NAT64)
        || (s[..5] == [0; 5] && matches!(s[5], 0 | 0xffff) && embedded(s[6], s[7]))
        || (s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] && embedded(s[6], s[7]))
        // 2002:a.b.c.d::/48 (6to4)
        || (s[0] == 0x2002 && embedded(s[1], s[2]))
}

/// Hosts an operator allows on a private address (`outbound.allow_private_hosts`), the one
/// list every outgoing request honours (review SEC2-3): `host` (the default port of the
/// request), `host:port`, an IP address in URL form (`10.0.0.5`, `[fd00::1]:8080`), or
/// `*.domain` for every subdomain of `domain`. Entries are compared in lower case.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrivateHosts(Vec<String>);

impl PrivateHosts {
    #[must_use]
    pub fn new<I: IntoIterator<Item = S>, S: AsRef<str>>(entries: I) -> Self {
        Self(
            entries
                .into_iter()
                .map(|e| e.as_ref().trim().to_ascii_lowercase())
                .filter(|e| !e.is_empty())
                .collect(),
        )
    }

    /// Does an entry cover `host` (as a URL writes it, IPv6 in brackets) on `port`, where
    /// `None` is the default port of the request's scheme?
    #[must_use]
    pub fn covers(&self, host: &str, port: Option<u16>) -> bool {
        let host = host.to_ascii_lowercase();
        self.0.iter().any(|entry| {
            let (name, entry_port) = split_port(entry);
            if entry_port != port {
                return false;
            }
            match name.strip_prefix("*.") {
                Some(domain) => host
                    .strip_suffix(domain)
                    .is_some_and(|sub| sub.len() > 1 && sub.ends_with('.')),
                None => name == host,
            }
        })
    }
}

/// `host:port` into its parts; `[v6]` and `[v6]:port` keep the brackets.
fn split_port(entry: &str) -> (&str, Option<u16>) {
    match entry.rsplit_once(':') {
        Some((h, p)) if !p.contains(']') && (!h.contains(':') || h.ends_with(']')) => {
            match p.parse() {
                Ok(p) => (h, Some(p)),
                Err(_) => (entry, None),
            }
        }
        _ => (entry, None),
    }
}

/// May a host be on a private address right now: when private targets of this kind are
/// allowed as a whole (`any`, read at the time of the request), or when the host is on the
/// operator's [`PrivateHosts`].
#[derive(Clone)]
pub struct PrivateAllow {
    any: Arc<dyn Fn() -> bool + Send + Sync>,
    hosts: Arc<PrivateHosts>,
}

impl std::fmt::Debug for PrivateAllow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivateAllow")
            .field("any", &(self.any)())
            .field("hosts", &self.hosts)
            .finish()
    }
}

impl PrivateAllow {
    #[must_use]
    pub fn new(any: Arc<dyn Fn() -> bool + Send + Sync>, hosts: PrivateHosts) -> Self {
        Self {
            any,
            hosts: Arc::new(hosts),
        }
    }

    /// Always (`true`) or never (`false`), without a host list.
    #[must_use]
    pub fn fixed(any: bool) -> Self {
        Self::new(Arc::new(move || any), PrivateHosts::default())
    }

    /// Are private targets of this kind allowed as a whole right now?
    #[must_use]
    pub fn any(&self) -> bool {
        (self.any)()
    }

    /// May `host` on `port` (`None`: the default port of the scheme) be on a private
    /// address?
    #[must_use]
    pub fn host(&self, host: &str, port: Option<u16>) -> bool {
        self.any() || self.hosts.covers(host, port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_ranges() {
        for ip in [
            "10.0.0.1",
            "192.168.1.2",
            "127.0.0.1",
            "169.254.1.1",
            "100.64.0.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:10.0.0.1",
        ] {
            assert!(is_private_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["93.184.216.34", "2001:db8::1", "100.128.0.1"] {
            assert!(!is_private_ip(ip.parse().unwrap()), "{ip}");
        }
    }

    /// Review SEC2-3 and SEC2-11: one list, entries per host, lower case, `*.` for
    /// subdomains only, ports exact.
    #[test]
    fn private_host_entries() {
        let h = PrivateHosts::new([
            "IdP.lan",
            "10.0.0.5:8443",
            "[fd00::1]:8080",
            "*.home.arpa",
            "relay.lan:25",
        ]);
        assert!(h.covers("idp.lan", None));
        assert!(h.covers("IDP.LAN", None));
        assert!(
            !h.covers("idp.lan", Some(8443)),
            "no port: the default port only"
        );
        assert!(h.covers("10.0.0.5", Some(8443)));
        assert!(!h.covers("10.0.0.5", None));
        assert!(h.covers("[fd00::1]", Some(8080)));
        assert!(h.covers("nas.home.arpa", None));
        assert!(h.covers("a.b.home.arpa", None));
        assert!(!h.covers("home.arpa", None), "`*.` names subdomains only");
        assert!(!h.covers("evilhome.arpa", None));
        assert!(h.covers("relay.lan", Some(25)));
        assert!(!h.covers("relay.lan", None));
        let allow = PrivateAllow::new(Arc::new(|| false), h);
        assert!(allow.host("idp.lan", None));
        assert!(!allow.host("other.lan", None));
        assert!(PrivateAllow::fixed(true).host("other.lan", None));
    }

    /// Review SEC2-8: special ranges and IPv6 forms that carry an IPv4 address.
    #[test]
    fn special_ranges_and_embedded_ipv4() {
        let private = [
            ("0.1.2.3", "this network, 0/8"),
            ("192.0.0.8", "IETF protocol assignments, 192.0.0/24"),
            ("198.18.0.1", "benchmarking, 198.18/15"),
            ("198.19.255.254", "benchmarking, 198.18/15"),
            ("240.0.0.1", "reserved, 240/4"),
            ("255.255.255.255", "broadcast"),
            ("224.0.0.251", "multicast, 224/4"),
            ("239.255.255.250", "multicast, 224/4"),
            ("ff02::1", "IPv6 multicast"),
            ("fec0::1", "site-local, fec0::/10"),
            ("64:ff9b::a9fe:a9fe", "NAT64 of 169.254.169.254"),
            ("64:ff9b::a00:1", "NAT64 of 10.0.0.1"),
            ("64:ff9b:1::5db8:d822", "local-use NAT64, 64:ff9b:1::/48"),
            ("::a00:1", "IPv4-compatible 10.0.0.1"),
            ("::7f00:1", "IPv4-compatible 127.0.0.1"),
            ("2002:a00:1::1", "6to4 of 10.0.0.1"),
            ("2002:a9fe:a9fe::", "6to4 of 169.254.169.254"),
        ];
        for (ip, what) in private {
            assert!(is_private_ip(ip.parse().unwrap()), "{ip}: {what}");
        }
        let public = [
            ("64:ff9b::5db8:d822", "NAT64 of 93.184.216.34"),
            ("2002:5db8:d822::1", "6to4 of 93.184.216.34"),
            ("198.20.0.1", "just above 198.18/15"),
            ("223.255.255.255", "just below 224/4"),
            ("2606:4700::1111", "global unicast"),
        ];
        for (ip, what) in public {
            assert!(!is_private_ip(ip.parse().unwrap()), "{ip}: {what}");
        }
    }
}
