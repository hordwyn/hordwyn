//! Email over SMTP (lettre, tokio, rustls).

use lettre::message::{Mailbox, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

use crate::deliver::{Message, SendError};
use crate::model::ChannelConfig;

/// How the SMTP connection is secured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpSecurity {
    /// TLS from the first byte (port 465).
    Tls,
    /// Plain connection upgraded with STARTTLS (port 587).
    StartTls,
    /// No encryption: only for a relay on localhost or in a private network.
    None,
}

#[derive(Clone)]
pub struct SmtpSettings {
    pub host: String,
    pub port: u16,
    pub security: SmtpSecurity,
    pub username: Option<String>,
    pub password: Option<String>,
    /// Sender, e.g. `Shelf <noreply@example.org>`.
    pub from: String,
}

impl std::fmt::Debug for SmtpSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmtpSettings")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("security", &self.security)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "***"))
            .field("from", &self.from)
            .finish()
    }
}

/// What a send is refused with when the server is on a private address and those are not
/// called (the admin API sorts failures into categories by this text).
pub const PRIVATE_ADDRESS: &str = "the mail server is on a private address";

/// May the mail server be on a private, loopback or link-local address right now: private
/// targets allowed as a whole, or the host on the operator's list (review SEC2-3)?
pub type PrivateRule = domain::net::PrivateAllow;

pub struct Email {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    settings: SmtpSettings,
    /// `None`: no rule, any address.
    allow_private: Option<PrivateRule>,
}

impl Email {
    /// # Errors
    /// Invalid host or sender address.
    pub fn new(s: &SmtpSettings) -> Result<Self, String> {
        Ok(Self {
            transport: Self::connect_to(s, &s.host)?,
            from: s.from.parse().map_err(|e| format!("sender address: {e}"))?,
            settings: s.clone(),
            allow_private: None,
        })
    }

    /// Ask `rule` before every send whether the server may be on a private address
    /// (SEC-4, DEC-534).
    #[must_use]
    pub fn with_private_rule(mut self, rule: PrivateRule) -> Self {
        self.allow_private = Some(rule);
        self
    }

    /// A transport that connects to `address` (the host itself, or one of its addresses)
    /// and checks the certificate against the host name of the settings.
    fn connect_to(
        s: &SmtpSettings,
        address: &str,
    ) -> Result<AsyncSmtpTransport<Tokio1Executor>, String> {
        let tls = || TlsParameters::new(s.host.clone()).map_err(|e| e.to_string());
        let mut b = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(address)
            .port(s.port)
            .tls(match s.security {
                SmtpSecurity::Tls => Tls::Wrapper(tls()?),
                SmtpSecurity::StartTls => Tls::Required(tls()?),
                SmtpSecurity::None => Tls::None,
            });
        if let (Some(u), Some(p)) = (&s.username, &s.password) {
            b = b.credentials(Credentials::new(u.clone(), p.clone()));
        }
        Ok(b.build())
    }

    /// May the mail server be on a private address: no rule, private targets allowed, or
    /// its host on the operator's list (`host:port`, or `host` for the ports 25, 465 and
    /// 587; review SEC2-3).
    fn private_allowed(&self) -> bool {
        let Some(rule) = &self.allow_private else {
            return true;
        };
        let s = &self.settings;
        let host = match s.host.parse::<std::net::Ipv6Addr>() {
            Ok(v6) => format!("[{v6}]"),
            Err(_) => s.host.clone(),
        };
        rule.host(&host, Some(s.port))
            || (matches!(s.port, 25 | 465 | 587) && rule.host(&host, None))
    }

    /// The transports for one send. Where the server may not be on a private address the
    /// host is resolved here, refused if one of its addresses is private, and there is one
    /// transport per checked address, tried in order (review SEC2-10), so a second DNS
    /// answer cannot differ (SEC-4).
    async fn transports(&self) -> Result<Vec<AsyncSmtpTransport<Tokio1Executor>>, String> {
        if self.private_allowed() {
            return Ok(vec![self.transport.clone()]);
        }
        let s = &self.settings;
        let addresses: Vec<std::net::IpAddr> = tokio::net::lookup_host((s.host.as_str(), s.port))
            .await
            .map_err(|e| format!("dns: {e}"))?
            .map(|a| a.ip())
            .collect();
        if addresses.is_empty() {
            return Err("dns: the mail server has no address".into());
        }
        if addresses.iter().any(|a| domain::net::is_private_ip(*a)) {
            return Err(PRIVATE_ADDRESS.into());
        }
        addresses
            .iter()
            .map(|a| Self::connect_to(s, &a.to_string()))
            .collect()
    }

    /// Send `mail` through the first transport whose server answers; a connection that
    /// fails moves on to the next address.
    async fn deliver(
        transports: &[AsyncSmtpTransport<Tokio1Executor>],
        mail: &lettre::Message,
    ) -> Result<(), Option<lettre::transport::smtp::Error>> {
        let answered = |e: &lettre::transport::smtp::Error| {
            e.is_response() || e.is_permanent() || e.is_transient()
        };
        try_in_order(transports, answered, |t| {
            let mail = mail.clone();
            async move { t.send(mail).await.map(|_| ()) }
        })
        .await
    }

    /// One plain-text message to `to` (the admin's "send a test").
    ///
    /// # Errors
    /// A bad address or the SMTP error, as text.
    pub async fn send_text(&self, to: &str, subject: &str, body: &str) -> Result<(), String> {
        let to: Mailbox = to.parse().map_err(|e| format!("recipient address: {e}"))?;
        let mail = lettre::Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body.to_string())
            .map_err(|e| e.to_string())?;
        let transports = self.transports().await?;
        Self::deliver(&transports, &mail)
            .await
            .map_err(|e| e.map_or_else(|| "no transport".to_string(), |e| e.to_string()))
    }

    /// # Errors
    /// See [`SendError`].
    pub async fn send(&self, channel: &ChannelConfig, m: &Message) -> Result<(), SendError> {
        let ChannelConfig::Email { to } = channel else {
            return Err(SendError::Permanent("not an email channel".into()));
        };
        let to: Mailbox = to
            .parse()
            .map_err(|e| SendError::Permanent(format!("recipient address: {e}")))?;
        let mut body = m.body.clone();
        if let Some(l) = &m.link {
            body.push_str("\n\n");
            body.push_str(l);
        }
        let mail = lettre::Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(m.title.clone())
            .header(ContentType::TEXT_PLAIN)
            .body(body)
            .map_err(|e| SendError::Permanent(e.to_string()))?;
        // a refused or unresolved host may be allowed or back later: retry
        let transports = self.transports().await.map_err(SendError::Retry)?;
        Self::deliver(&transports, &mail)
            .await
            .map_err(|e| match e {
                Some(e) if e.is_permanent() => SendError::Permanent(e.to_string()),
                Some(e) => SendError::Retry(e.to_string()),
                None => SendError::Retry("no transport".into()),
            })
    }
}

/// Try `items` in order: an attempt that fails before a server answered (`answered` is
/// false for its error) moves on to the next one. The result of the first answer, or the
/// last failure (`None` without items).
async fn try_in_order<'a, A, T, E, F, Fut>(
    items: &'a [A],
    answered: impl Fn(&E) -> bool,
    mut attempt: F,
) -> Result<T, Option<E>>
where
    F: FnMut(&'a A) -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut last = None;
    for item in items {
        match attempt(item).await {
            Ok(t) => return Ok(t),
            Err(e) if answered(&e) => return Err(Some(e)),
            Err(e) => last = Some(e),
        }
    }
    Err(last)
}

/// The email transport in use, replaceable at run time when an admin changes the SMTP
/// settings (addendum-1 3.2); `None` while mail is not configured.
#[derive(Default)]
pub struct Mailer {
    current: std::sync::RwLock<Option<std::sync::Arc<Email>>>,
    /// Given to every transport this mailer holds.
    rule: Option<PrivateRule>,
}

impl Mailer {
    #[must_use]
    pub fn new(email: Option<Email>) -> Self {
        Self {
            current: std::sync::RwLock::new(email.map(std::sync::Arc::new)),
            rule: None,
        }
    }

    /// A mailer whose transports, this one and every later one, ask `rule` before a send
    /// whether the server may be on a private address (SEC-4, DEC-534).
    #[must_use]
    pub fn with_private_rule(email: Option<Email>, rule: PrivateRule) -> Self {
        let mailer = Self {
            current: std::sync::RwLock::default(),
            rule: Some(rule),
        };
        mailer.set(email);
        mailer
    }

    /// Use `email` from now on (deliveries already sending finish with the old one).
    pub fn set(&self, email: Option<Email>) {
        let email = match (&self.rule, email) {
            (Some(rule), Some(e)) => Some(e.with_private_rule(rule.clone())),
            (_, e) => e,
        };
        *self
            .current
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = email.map(std::sync::Arc::new);
    }

    #[must_use]
    pub fn current(&self) -> Option<std::sync::Arc<Email>> {
        self.current
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UX3-26: the SMTP password never shows in a `{:?}`.
    fn local(host: &str, port: u16, allow_private: bool) -> Email {
        local_with(host, port, domain::net::PrivateAllow::fixed(allow_private))
    }

    fn local_with(host: &str, port: u16, rule: domain::net::PrivateAllow) -> Email {
        Email::new(&SmtpSettings {
            host: host.into(),
            port,
            security: SmtpSecurity::None,
            username: None,
            password: None,
            from: "Shelf <noreply@example.org>".into(),
        })
        .unwrap()
        .with_private_rule(rule)
    }

    /// SEC-4 (DEC-534): where private targets are not allowed, a mail server on a private
    /// address is not even connected to, by address or by name.
    #[tokio::test]
    async fn a_mail_server_on_a_private_address_is_refused_unless_allowed() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let connections = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        tokio::spawn({
            let connections = connections.clone();
            async move {
                while let Ok((socket, _)) = listener.accept().await {
                    connections.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    drop(socket);
                }
            }
        });
        for host in ["127.0.0.1", "localhost"] {
            let err = local(host, port, false)
                .send_text("anna@example.org", "Test", "Hello")
                .await
                .unwrap_err();
            assert!(err.contains(PRIVATE_ADDRESS), "{host}: {err}");
        }
        assert_eq!(connections.load(std::sync::atomic::Ordering::SeqCst), 0);
        // allowed: the connection is made (the listener speaks no SMTP, so the send fails)
        let err = local("127.0.0.1", port, true)
            .send_text("anna@example.org", "Test", "Hello")
            .await
            .unwrap_err();
        assert!(!err.contains(PRIVATE_ADDRESS), "{err}");
        assert_eq!(connections.load(std::sync::atomic::Ordering::SeqCst), 1);
        // review SEC2-3: private targets refused, but the mail host is on the operator's
        // list (`outbound.allow_private_hosts`): connected, by address and by name
        for (host, entry) in [
            ("127.0.0.1", format!("127.0.0.1:{port}")),
            ("localhost", format!("LocalHost:{port}")),
        ] {
            let rule = domain::net::PrivateAllow::new(
                std::sync::Arc::new(|| false),
                domain::net::PrivateHosts::new([entry]),
            );
            let err = local_with(host, port, rule)
                .send_text("anna@example.org", "Test", "Hello")
                .await
                .unwrap_err();
            assert!(!err.contains(PRIVATE_ADDRESS), "{host}: {err}");
        }
        assert_eq!(connections.load(std::sync::atomic::Ordering::SeqCst), 3);
        // another port of the listed host is not listed
        let rule = domain::net::PrivateAllow::new(
            std::sync::Arc::new(|| false),
            domain::net::PrivateHosts::new([format!("127.0.0.1:{}", port.wrapping_add(1))]),
        );
        let err = local_with("127.0.0.1", port, rule)
            .send_text("anna@example.org", "Test", "Hello")
            .await
            .unwrap_err();
        assert!(err.contains(PRIVATE_ADDRESS), "{err}");
    }

    /// Review SEC2-10: every checked address is tried until a server answers; an answer
    /// (accepted or refused) ends the round.
    #[tokio::test]
    async fn each_checked_address_is_tried_until_a_server_answers() {
        let tried = std::sync::Mutex::new(Vec::new());
        let r: Result<&str, Option<(u8, bool)>> = try_in_order(
            &[1u8, 2, 3],
            |e: &(u8, bool)| e.1,
            |a| {
                tried.lock().unwrap().push(*a);
                let out = if *a == 2 {
                    Ok("sent")
                } else {
                    Err((*a, false))
                };
                async move { out }
            },
        )
        .await;
        assert_eq!(r, Ok("sent"));
        assert_eq!(
            *tried.lock().unwrap(),
            [1, 2],
            "no connection at 1: 2 was tried"
        );
        tried.lock().unwrap().clear();
        let r: Result<&str, Option<(u8, bool)>> = try_in_order(
            &[1u8, 2],
            |e: &(u8, bool)| e.1,
            |a| {
                tried.lock().unwrap().push(*a);
                let out = Err((*a, true));
                async move { out }
            },
        )
        .await;
        assert_eq!(r, Err(Some((1, true))), "the server at 1 answered");
        assert_eq!(*tried.lock().unwrap(), [1]);
    }

    #[test]
    fn smtp_settings_hide_the_password_in_debug() {
        let s = SmtpSettings {
            host: "mail.example.org".into(),
            port: 587,
            security: SmtpSecurity::StartTls,
            username: Some("relay".into()),
            password: Some("smtp-secret-1".into()),
            from: "Shelf <noreply@example.org>".into(),
        };
        let printed = format!("{s:?}");
        assert!(!printed.contains("smtp-secret-1"), "{printed}");
        assert!(printed.contains("mail.example.org"), "{printed}");
    }
}
