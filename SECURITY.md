# Security Policy

## Reporting a vulnerability

Please report security issues privately, not as a public issue.

Use GitHub's private vulnerability reporting:
**[Report a vulnerability](https://github.com/danmolitor/forme/security/advisories/new)**
(the "Security" tab → "Report a vulnerability"). If that isn't available to you,
email **dan@formepdf.com**.

## What's in scope

Forme parses untrusted input — HTML, CSS, JSON, fonts, and images — and turns it
into PDFs, often on a server or at the edge. The things worth reporting:

- Memory-safety or panics in the Rust engine reachable from crafted input
- Denial of service from a small input (pathological layout, decompression, or
  parsing blowups)
- Sandbox escape in any path that evaluates user-supplied templates
- Anything that lets crafted input read or exfiltrate data it shouldn't

A rendered PDF containing exactly the content you fed it is not a vulnerability,
and neither is unsupported CSS being ignored with a warning — that's the
documented contract.

## What to expect

Forme is maintained by one person. I'll acknowledge a valid report as soon as I
can and give you an honest read on timing — there's no paid SLA. Please give me
a reasonable window to ship a fix before disclosing publicly, and I'll credit you
in the advisory unless you'd rather stay anonymous.
