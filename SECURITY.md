# Security

This document describes what LTC Wallet protects, what it does not, and how to
report vulnerabilities. For verifying that a downloaded release matches the
source code, see [`docs/VERIFYING.md`](docs/VERIFYING.md).

## Reporting a vulnerability

Report security issues privately via
[GitHub Security Advisories](https://github.com/LitecoinDevKit/ltc-wallet-mac/security/advisories/new)
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
| Recovery phrase / seed | Encrypted at rest in `wallet.mnemonic.enc`: Argon2id (64 MiB, t=3) key derivation + ChaCha20-Poly1305 AEAD, file mode `0600`. Files created by older versions (19 MiB, t=2) are transparently re-encrypted with the stronger parameters on the next unlock. Decrypted only into process memory while unlocked; zeroized on lock. |
| Passphrase | Never stored; used only to derive the encryption key. Wrong passphrases fail AEAD authentication. |
| MWEB data at rest | The MWEB coin store, sync state, receive index and history are sealed with ChaCha20-Poly1305 under a random key stored inside the encrypted seed file. Plaintext-era files are migrated to the sealed format and deleted on the first sync after unlock. |
| Idle sessions | The wallet auto-locks (default 15 minutes, configurable, 0 = off) after no user input, wiping decrypted key material from memory. |
| Transactions | Built and signed locally; keys never leave the process. Broadcast goes to your configured Electrum server (transparent) or MWEB P2P peers / litecoind RPC. |
| Network transport | Electrum connections use TLS. Certificate validation (CA chain + hostname) is **on by default**; it can be disabled in Settings for self-signed community servers, which trades MITM protection for availability. The UI warns before saving a non-localhost `tcp://` (unencrypted) server. |
| Server honesty | After each sync the wallet asks a second, independent Electrum server for block headers only (never your addresses) and warns if the servers disagree or the sync server appears to be withholding blocks. Each successful MWEB sync is cross-checked by verifying the downloaded UTXO leafset against the MWEB header reported by up to two peers. |
| Fallback privacy | If you run your own Electrum server you can disable public-server fallback in Settings, so your addresses are never sent to public servers; the active server is always shown in Settings. |
| Destructive actions | Wiping wallet data requires typing a confirmation phrase, enforced at the IPC boundary, not just in the UI. |

### What the wallet does NOT protect against

- **A compromised machine.** Malware running as your user can read process
  memory while the wallet is unlocked, keylog your passphrase, or replace the
  app binary. No desktop wallet survives this; use a hardware wallet or an
  offline machine for large amounts.
- **Unencrypted transparent-wallet metadata.** `wallet.sqlite` (the BDK
  transparent-side database) stores addresses, balances, and transaction
  history in plaintext. Someone with access to your data directory learns your
  transparent financial history (but not your keys, and not your MWEB data,
  which is sealed). Use full-disk encryption (FileVault/LUKS).
- **Network privacy.** There is no Tor/proxy support. Your Electrum server
  learns your addresses and IP; DNS-discovered MWEB peers learn your IP.
  The first-party explorer ([litview.space](https://litview.space) by default,
  or a self-hosted LRK URL in Settings) learns your IP when the wallet
  automatically fetches spot price (~60s while unlocked) or fee hints (send
  form). It learns IP + a specific txid when you open a transaction detail
  (enrichment) or click “View on litview” (system browser). Fiat and fee-hint
  fetches can be disabled in Settings. Wallet address lists are never uploaded
  to the explorer. See [`docs/LITVIEW.md`](docs/LITVIEW.md).
- **Colluding servers.** The post-sync cross-checks compare independent
  sources, which turns "one dishonest server can lie to you" into "two
  independent sources must collude". They compare only chain tips and MWEB
  UTXO roots; a server can still hide an individual unconfirmed transaction
  from you until it confirms.
- **Physical attackers with your passphrase**, shoulder surfing, or coerced
  disclosure.

### Trusted computing base

Beyond this repository, the wallet's correctness depends on the pinned fork
dependencies (rev-pinned in the Cargo manifests and `Cargo.lock`):

- [`LitecoinDevKit/bdk`](https://github.com/LitecoinDevKit/bdk) (+ `bdk_wallet`) — wallet logic, MWEB crypto
- [`LitecoinDevKit/rust-litecoin`](https://github.com/LitecoinDevKit/rust-litecoin) — consensus types and serialization
- upstream crates locked in `Cargo.lock` (audited in CI by `cargo audit` / `cargo deny`)

An external audit should treat those forks as first-class audit targets, not
vendored dependencies.

## Known limitations (accepted for now)

- The transparent-side `wallet.sqlite` is not encrypted at rest (the seed and
  all MWEB data are).
- No Tor or proxy support.
- Testnet Electrum servers use self-signed certificates, so testnet generally
  requires disabling TLS validation in Settings.
- Electrum cross-checking compares block headers only; it detects a server on
  a wrong chain or withholding blocks, not omission of individual mempool
  transactions.
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
its notes point at the dependency pins it was built from. CI builds only from
the revs pinned in the manifests and `Cargo.lock`. Additionally:

- **Build provenance attestations**: every artifact digest is attested by
  GitHub's build provenance service, linking it to the exact commit and
  workflow run. Verify with `gh attestation verify <file> --repo
  LitecoinDevKit/ltc-wallet-mac`.
- **Minisign signatures** (once the signing key is published): each
  `SHA256SUMS` file is signed with an offline-verifiable minisign key, so
  verification does not depend on GitHub's infrastructure.

See [`docs/VERIFYING.md`](docs/VERIFYING.md) for the full verification guide.
