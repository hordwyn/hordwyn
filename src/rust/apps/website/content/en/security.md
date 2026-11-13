+++
title = "Security"
description = "How to report a vulnerability in {name}, and what to expect."
eyebrow = "Security"
notice = "The contact for security reports and the disclosure policy have not been settled yet."
+++

## Reporting a vulnerability

Please report security problems privately and not in a public issue.

**Contact: {security_contact}**

The same contact is published in machine-readable form in [`/.well-known/security.txt`](/.well-known/security.txt).

Helpful in a report:

- the version or commit you tested,
- what an attacker can do, and under which conditions,
- steps to reproduce, or a proof of concept,
- whether you want to be named when the fix is published.

## What to expect

**[PLACEHOLDER: disclosure policy, to be decided by the owner - whether and how fast a report is acknowledged, how fixes are released and announced, the time asked for before details are published, bug bounty or none]**

## Scope

**[PLACEHOLDER: scope and supported versions, to be decided by the owner - what is in scope, what is not, which releases receive security fixes]**

## Running an instance safely

{name} is meant to run behind a reverse proxy with TLS. The [documentation]({docs}) describes the reverse-proxy settings, backups and the secrets key.
<!-- shipped:security_headers -->
It also lists the security headers the application sends.
<!-- /shipped -->
