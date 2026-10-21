# Security policy

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability, exposed
credential, or signing-key incident. Use GitHub's private vulnerability
reporting flow from the repository's **Security** tab. If that flow is not
available, email the maintainer at <itpkcn@gmail.com>.

Include the affected version or commit, reproduction steps, impact, and any
known mitigations. Do not include real access tokens, passwords, private keys,
or personal data in the report.

## Supported versions

Security fixes are made on the latest revision of `main`. Older snapshots are
not maintained separately.

## Credential handling

Release keystores, signing passwords, Android SDK paths, service-account files,
and local environment files must remain outside Git. If a real credential is
ever committed, revoke or rotate it immediately; removing it from a later
commit is not sufficient.
