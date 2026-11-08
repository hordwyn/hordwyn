//! Email over SMTP (lettre, tokio, rustls).

use lettre::message::{Mailbox, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
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

pub struct Email {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl Email {
    /// # Errors
    /// Invalid host or sender address.
    pub fn new(s: &SmtpSettings) -> Result<Self, String> {
        let mut b = match s.security {
            SmtpSecurity::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&s.host),
            SmtpSecurity::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&s.host),
            SmtpSecurity::None => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
                s.host.clone(),
            )),
        }
        .map_err(|e| e.to_string())?
        .port(s.port);
        if let (Some(u), Some(p)) = (&s.username, &s.password) {
            b = b.credentials(Credentials::new(u.clone(), p.clone()));
        }
        Ok(Self {
            transport: b.build(),
            from: s.from.parse().map_err(|e| format!("sender address: {e}"))?,
        })
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
        self.transport
            .send(mail)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
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
        self.transport.send(mail).await.map(|_| ()).map_err(|e| {
            if e.is_permanent() {
                SendError::Permanent(e.to_string())
            } else {
                SendError::Retry(e.to_string())
            }
        })
    }
}

/// The email transport in use, replaceable at run time when an admin changes the SMTP
/// settings (addendum-1 3.2); `None` while mail is not configured.
#[derive(Default)]
pub struct Mailer(std::sync::RwLock<Option<std::sync::Arc<Email>>>);

impl Mailer {
    #[must_use]
    pub fn new(email: Option<Email>) -> Self {
        Self(std::sync::RwLock::new(email.map(std::sync::Arc::new)))
    }

    /// Use `email` from now on (deliveries already sending finish with the old one).
    pub fn set(&self, email: Option<Email>) {
        *self
            .0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = email.map(std::sync::Arc::new);
    }

    #[must_use]
    pub fn current(&self) -> Option<std::sync::Arc<Email>> {
        self.0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UX3-26: the SMTP password never shows in a `{:?}`.
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
