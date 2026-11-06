# Notification channels (M13 research, 2026-11-06)

Facts gathered by a research agent of this session from primary sources, written down in our
own words. No source code of any project was read or copied.

## Licences (rule 6)

| Item | Docs / spec | Code / crate | Use |
|---|---|---|---|
| ntfy | same repo, dual Apache-2.0 / GPLv2 | Apache-2.0 OR GPLv2 | HTTP API only, taken under Apache-2.0 |
| Gotify | website repo without licence, API spec (spec.json) MIT | MIT | HTTP API only; site text not copied |
| Discord webhooks | CC BY-SA 4.0 | samples MIT | HTTP API only; doc text not copied |
| Matrix client-server spec | Apache-2.0 | - | HTTP API only |
| apprise-api | MIT (since 2019-10-27) | MIT | HTTP API only |
| Apprise library | BSD-2-Clause since 2023-10-13 (GPLv3 until 2019-02-01) | - | not used; pre-2019 material avoided |
| RFC 8030 / 8291 / 8292 | IETF Trust | - | protocol |
| Standard Webhooks | Apache-2.0 | Apache-2.0 | signing scheme |
| web-push-native 0.5 | - | MIT OR Apache-2.0, pure Rust (RustCrypto) | not used: pins crypto-bigint 0.5 (RUSTSEC-2026-0387), see DEC-141 |
| web-push 0.11 | - | Apache-2.0, needs OpenSSL | not used (no rustls) |
| lettre 0.11 | - | MIT | SMTP |
| croner 4 / chrono-tz 0.10 | - | MIT / MIT OR Apache-2.0 | digest schedule in the user's timezone |

## Protocols as implemented

- **ntfy** (https://docs.ntfy.sh/publish/): JSON publish with `POST` to the server root, body
  `{topic, title, message, click, tags, priority}`; access token as `Authorization: Bearer`.
- **Gotify** (https://gotify.net/docs/pushmsg, spec.json in gotify/server): `POST /message`
  with header `X-Gotify-Key: <app token>`, body `{title, message, priority, extras}`; a click
  URL goes in `extras["client::notification"].click.url`.
- **Discord** (https://docs.discord.com/developers/resources/webhook): `POST` to the webhook
  URL with `{content}` (at most 2000 characters). A 429 answer carries `retry_after` in
  seconds (https://docs.discord.com/developers/topics/rate-limits): retried later.
- **Matrix** (https://spec.matrix.org/latest/client-server-api/):
  `PUT /_matrix/client/v3/rooms/{roomId}/send/m.room.message/{txnId}` with
  `Authorization: Bearer`, body `{msgtype: "m.text", body}`. The txnId makes retries
  idempotent; we use the delivery id.
- **Apprise API** (https://github.com/caronc/apprise-api): stateless `POST /notify/` with
  `{urls, title, body, type}`.
- **Signed webhook** (Standard Webhooks spec, github.com/standard-webhooks): headers
  `webhook-id` (stable across retries), `webhook-timestamp` (Unix seconds),
  `webhook-signature: v1,<base64 HMAC-SHA256 of "id.timestamp.body">`; secret `whsec_` +
  base64 of random bytes. 2xx is success, 410 disables the endpoint.
- **Web push** (planned, DEC-141; RFC 8030, 8291, 8292): `aes128gcm` encrypted body (at most 3993 bytes of
  plaintext), VAPID ES256 token, required `TTL` header. 201 accepted; 404 (and 410 by common
  practice) mean the subscription is gone and the channel is disabled.
- **Email**: SMTP through lettre with tokio and rustls.

## Gaps

- pushicino (an alternative web-push crate) not vetted; not used.
- Exact IETF licence wording not fetched.
