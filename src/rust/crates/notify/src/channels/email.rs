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

#[derive(Debug, Clone)]
pub struct SmtpSettings {
    pub host: String,
    pub port: u16,
    pub security: SmtpSecurity,
    pub username: Option<String>,
    pub password: Option<String>,
    /// Sender, e.g. `Shelf <noreply@example.org>`.
    pub from: String,
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
