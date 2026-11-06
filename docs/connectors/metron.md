# Connector: Metron (`metron`) - not implemented (owner decision needed)

Date checked: 2026-11-06

| Item | Value |
|---|---|
| Kinds | comic |
| Licence of the service software | GPL-3.0 (github.com/Metron-Project/metron) |
| Status | **not implemented: KICKOFF rule 6 conflict** |

## Why not

KICKOFF rule 6 forbids looking at source code, schemas, docs, UI text or assets of any
GPL-family project. The software behind this service is GPL-family and its API
documentation is part of that project, so it was not read (only the licence was checked from
repository metadata). Implementing a client without the documentation would mean guessing
the API, which rule 1 forbids.

Owner decision (DEC-106): if public API documentation and terms of a GPL-licensed service may
be read to write an independent HTTP client (no code copied), this connector can be built in
a follow-up.
