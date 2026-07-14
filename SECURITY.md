# Security Policy

## Supported versions

Security fixes target latest ivygrep release and `main`. Upgrade before
reporting when practical.

## Report a vulnerability

Use [GitHub private vulnerability reporting][report]. Do not open public issue,
discussion, or pull request for undisclosed vulnerability.

Include:

- affected version, operating system, architecture, and installation method
- impact and required attacker access
- minimal reproduction or proof of concept
- suggested mitigation, when known
- whether public disclosure has occurred

Remove unrelated source code, credentials, indexes, and personal paths. Reports
will be acknowledged privately. Fix and disclosure timing depends on severity,
exploitability, and release availability.

## Scope

Security reports can cover CLI, daemon, local web server, MCP server, index
storage, installers, update path, and published release artifacts. Vulnerabilities
in upstream models or dependencies remain useful when ivygrep exposes or
amplifies their impact.

[report]: https://github.com/bvolpato/ivygrep/security/advisories/new
