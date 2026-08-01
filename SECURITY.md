# Security

This document describes what LTC Wallet protects, what it does not, and how to
report vulnerabilities. For verifying that a downloaded release matches the
source code, see [`docs/VERIFYING.md`](docs/VERIFYING.md).

## Reporting a vulnerability

Report security issues privately via
[GitHub Security Advisories](https://github.com/IndigoNakamoto/ltc-wallet-mac/security/advisories/new)
("Report a vulnerability"). Please do not open public issues for bugs that
could put user funds at risk. You should receive an initial response within
7 days.

In scope: anything that can lose, steal, or silently misdirect funds; secret
key or mnemonic disclosure; remote code execution; transaction malleation the
wallet fails to detect. Out of scope: attacks requiring an already-compromised
machine (see threat model below), denial of service against public Electrum
servers, and social engineering.

## Threat model

### What the wallet protects

| Asset | Protection |
| --- | --- |
| Recovery phrase / seed | Encrypted at rest in `wallet.mnemonic.enc`: Argon2id (19 MiB, t=2) key derivation + ChaCha20-Poly1305 AEAD, file mode `0600`. Decrypted only into process memory while unlocked; zeroized on lock. |
| Passphrase | Never stored; used only to derive the encryption key. Wrong passphrases fail AEAD authentication. |
| Transactions | Built and signed locally; keys never leave the process. Broadcast goes to your configured Electrum server (transparent) or MWEB P2P peers / litecoind RPC. |
| Network transport | Electrum connections use TLS. Certificate validation (CA chain + hostname) is **on by default**; it can be disabled in Settings for self-signed community servers, which trades MITM protection for availability. |
| Destructive actions | Wiping wallet data requires typing a confirmation phrase, enforced at the IPC boundary, not just in the UI. |

### What the wallet does NOT protect against

- **A compromised machine.** Malware running as your user can read process
  memory while the wallet is unlocked, keylog your passphrase, or replace the
  app binary. No desktop wallet survives this; use a hardware wallet or an
  offline machine for large amounts.
- **Unencrypted metadata.** `wallet.sqlite`, `mweb.sqlite`, and the history/
  sync files store addresses, balances, and transaction history in plaintext.
  Someone with access to your data directory learns your financial history
  (but not your keys). Use full-disk encryption (FileVault/LUKS).
- **Network privacy.** There is no Tor/proxy support. Your Electrum server
  learns your addresses and IP; DNS-discovered MWEB peers learn your IP.
- **Server lies by omission.** A malicious Electrum server can hide incoming
  transactions or delay broadcast (it cannot steal funds or forge history
  without breaking the transaction chain). MWEB sync trusts the connected peer
  for the UTXO set; multi-peer cross-checking is not implemented.
- **Physical attackers with your passphrase**, shoulder surfing, or coerced
  disclosure.

### Trusted computing base

Beyond this repository, the wallet's correctness depends on the pinned sibling
forks (see [`deps/pins.env`](deps/pins.env)):

- [`IndigoNakamoto/bdk`](https://github.com/IndigoNakamoto/bdk) (+ `bdk_wallet`) — wallet logic, MWEB crypto
- [`IndigoNakamoto/rust-litecoin`](https://github.com/IndigoNakamoto/rust-litecoin) — consensus types and serialization
- upstream crates locked in `Cargo.lock` (audited in CI by `cargo audit` / `cargo deny`)

An external audit should treat those forks as first-class audit targets, not
vendored dependencies.

## Known limitations (accepted for now)

- Wallet SQLite databases are not encrypted at rest (seed file is).
- No Tor or proxy support.
- Testnet Electrum servers use self-signed certificates, so testnet generally
  requires disabling TLS validation in Settings.
- MWEB sync trusts a single peer per sync pass.
- macOS releases are unsigned until Apple Developer credentials are set up
  (CI is already wired to sign automatically once the secrets exist).
- The `wipe` escape hatch intentionally works without the passphrase — it is
  the only recovery path when a passphrase is lost. It deletes data only
  (funds are recoverable from a mnemonic backup) and requires a typed
  confirmation phrase.

## Supported versions

Only the latest release receives security fixes.

## Release integrity

Every release attaches `SHA256SUMS-<platform>.txt` files generated in CI, and
its notes list the sibling dependency SHAs it was built from. CI builds only
from the pinned SHAs in `deps/pins.env`. See
[`docs/VERIFYING.md`](docs/VERIFYING.md) for the full verification guide.
