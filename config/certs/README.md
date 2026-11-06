# Extra CA certificates (optional)

Behind a TLS-inspecting proxy (corporate network security, e.g. Cisco Secure Access), put the
proxy's root CA certificates here as PEM files ending in `.crt`, for example:

    cp /usr/local/share/ca-certificates/*.crt config/certs/

The Docker build trusts them while it downloads build tools and crates, and the runtime image
trusts them for outgoing connections (metadata providers, notification channels). The `.crt`
files are git-ignored and never committed. Without files here nothing changes.
