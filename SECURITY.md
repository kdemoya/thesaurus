# Security policy

Thesaurus is pre-release software. Security fixes target the latest code on the default branch; there are no maintained older release branches yet.

## Reporting a vulnerability

Use GitHub's [private vulnerability reporting](https://github.com/kdemoya/thesaurus/security/advisories/new) to report a suspected vulnerability. Do not disclose vulnerabilities in public issues. If the private reporting form is unavailable, contact [@kdemoya](https://github.com/kdemoya) to arrange a private channel before sharing details.

Include the affected revision or version, operating system, expected and observed behavior, and minimal reproduction steps using synthetic data. Do not send real workspace databases, transcripts, tokens, signing keys, or grant files. There is no guaranteed response time or bug bounty program.

## Data and trust boundaries

Workspaces contain private source material and credentials. SQLite workspace data is currently stored without application-level encryption. Source grants and external-sharing controls govern access through Thesaurus; they do not protect files from other processes running as the same user. Backups need the same protection as the original workspace.

Installed executable plugins and configured command adapters run locally as the user. Install only code you trust. The HTTP API binds to loopback and requires authentication; do not expose it through a public proxy. Development build tools, local app signing, and synthetic acceptance checks do not establish that an installer is ready for public distribution.
