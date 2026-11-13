+++
title = "Sicherheit"
description = "Wie du eine Sicherheitslücke in {name} meldest und was du erwarten kannst."
eyebrow = "Sicherheit"
notice = "Der Kontakt für Sicherheitsmeldungen und die Regeln für Meldungen sind noch nicht festgelegt."
+++

## Eine Sicherheitslücke melden

Bitte melde Sicherheitsprobleme vertraulich und nicht in einem öffentlichen Ticket.

**Kontakt: {security_contact}**

Derselbe Kontakt steht maschinenlesbar in [`/.well-known/security.txt`](/.well-known/security.txt).

Hilfreich in einer Meldung:

- die Version oder der Commit, den du geprüft hast,
- was ein Angreifer tun kann, und unter welchen Bedingungen,
- Schritte zum Nachvollziehen oder ein Machbarkeitsnachweis,
- ob du genannt werden möchtest, wenn die Korrektur veröffentlicht wird.

## Was du erwarten kannst

**[PLACEHOLDER: Regeln für Meldungen, vom Betreiber zu entscheiden - ob und wie schnell der Eingang bestätigt wird, wie Korrekturen veröffentlicht und angekündigt werden, welche Frist vor einer Veröffentlichung von Einzelheiten erbeten wird, Bug-Bounty oder nicht]**

## Geltungsbereich

**[PLACEHOLDER: Geltungsbereich und unterstützte Versionen, vom Betreiber zu entscheiden - was gemeint ist, was nicht, welche Versionen Sicherheitskorrekturen erhalten]**

## Eine Instanz sicher betreiben

{name} ist für den Betrieb hinter einem Reverse Proxy mit TLS gedacht. Die <a href="{docs}" hreflang="en">Dokumentation</a> (englisch) beschreibt die Einstellungen für den Reverse Proxy, Sicherungen und den Geheimnisschlüssel.
<!-- shipped:security_headers -->
Sie nennt auch die Sicherheits-Header, die die Anwendung sendet.
<!-- /shipped -->
