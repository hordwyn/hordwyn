//! Name resolution that refuses private addresses at connect time, so a DNS answer cannot
//! change between a check and the request (G2-7, SEC-4). IP literals in a URL do not go
//! through a resolver; the clients check those themselves.

/// The name resolved to a private, loopback or link-local address.
#[derive(Debug)]
pub(crate) struct PrivateAddress(String);

impl std::fmt::Display for PrivateAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} resolves to a private address", self.0)
    }
}

impl std::error::Error for PrivateAddress {}

/// DNS that only answers with public addresses: one private address among the answers
/// refuses the name.
pub(crate) struct PublicOnly;

impl reqwest::dns::Resolve for PublicOnly {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async move {
            let addrs: Vec<std::net::SocketAddr> =
                tokio::net::lookup_host((name.as_str(), 0)).await?.collect();
            if addrs.iter().any(|a| domain::net::is_private_ip(a.ip())) {
                return Err(PrivateAddress(name.as_str().to_string()).into());
            }
            let addrs: reqwest::dns::Addrs = Box::new(addrs.into_iter());
            Ok(addrs)
        })
    }
}

/// Did the request fail because [`PublicOnly`] refused the name?
pub(crate) fn refused_as_private(e: &reqwest::Error) -> bool {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(s) = source {
        if s.is::<PrivateAddress>() {
            return true;
        }
        source = s.source();
    }
    false
}
