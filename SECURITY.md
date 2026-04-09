# Security policy

## Reporting a vulnerability

Found a security issue? Don't open a public issue. Email security@gremlin.group with:

- A description of the vulnerability
- Steps to reproduce
- The version you're running (`si --version`)

We'll get back to you as soon as we can.

## Scope

space-investigator is read-only. It doesn't modify files, open network connections, or need elevated privileges. The realistic attack surface is small:

- Path traversal or symlink following that exposes unintended data
- Denial of service through crafted directory structures
- Dependency vulnerabilities in upstream crates
