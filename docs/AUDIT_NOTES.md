# Internal security review — findings and status

Internal review performed August 2026 alongside the security-hardening pass
(TLS validation, CSP, wipe gate, supply-chain pinning, release checksums).
This is not a third-party audit; it is the working findings list and the scope
document for one. Reviewed: `wallet-core` (secrets, seed parsing, Electrum,
RPC, MWEB orchestration), the Tauri command surface, the UI's secret handling,
and both GitHub workflows. An automated diff-focused security review of the
hardening changes found no medium-or-higher issues.

Severity: **H**igh / **M**edium / **L**ow / **I**nformational.

## Fixed in the hardening pass

| # | Sev | Finding | Fix |
| --- | --- | --- | --- |
| F1 | H | Electrum TLS ran with `validate_domain(false)` — any MITM could impersonate the server (hide transactions, delay broadcasts, feed a false chain view) | Validation on by default (`crates/wallet-core/src/electrum.rs`); per-wallet opt-out in Settings with a warning; CA-certified `cipig.net` servers added as first mainnet defaults (verified serving Litecoin genesis over valid TLS); `server.version` handshake added for strict ElectrumX servers |
| F2 | M | Webview CSP was `null` — any script injection had full latitude | Strict CSP (`default-src 'self'`, no inline script) + separate dev CSP in `src-tauri/tauri.conf.json`; verified baked into the release binary |
| F3 | M | `wipe_wallet` was callable with a bare `invoke()`; UI confirm was the only gate | Typed phrase `DELETE WALLET` required, enforced at the IPC boundary in `src-tauri/src/lib.rs` |
| F4 | L | RPC transport errors could echo `user:pass@` from the configured URL into UI error text | `redact_userinfo` in `crates/wallet-core/src/rpc.rs` + tests |
| F5 | L | `com.apple.security.cs.allow-jit` entitlement was unnecessary (WKWebView JIT lives in Apple's XPC process) | Removed; release build + launch verified |
| F6 | L | Secret files were created with umask permissions, then chmod'd to 0600 (brief exposure window) | Created with 0600 atomically (`OpenOptionsExt::mode`) in `crates/wallet-core/src/secrets.rs` |
| F7 | M | Supply chain: `Cargo.lock` untracked, actions pinned by tag, no CI, no artifact checksums | Lockfile tracked; all actions pinned to commit SHAs; `ci.yml` runs build/clippy/tests/`cargo audit`/`cargo deny`/`npm audit`; releases attach `SHA256SUMS-<platform>.txt`; Apple signing is drop-in via secrets |

## Accepted risks (by design — documented in SECURITY.md)

| # | Sev | Risk | Rationale |
| --- | --- | --- | --- |
| A1 | M | TLS validation can be disabled by the user | Most community Electrum-LTC servers (and all known testnet servers) are self-signed; the toggle requires an unlocked wallet and shows a warning |
| A2 | M | Wipe works without the passphrase | Only recovery path when the passphrase is lost; destroys data, not funds (mnemonic backup restores); gated by the typed phrase |
| A3 | L | CLI accepts `--passphrase` / `WALLET_PASSPHRASE` (visible in shell history / `ps` / env) | Convenience for scripting; interactive hidden prompt is the default when omitted |
| A4 | L | CLI prints the mnemonic once on `create` | Deliberate one-time backup output, sent to stderr |

## Open findings (deferred — future work, external-audit scope)

| # | Sev | Finding | Suggested direction |
| --- | --- | --- | --- |
| O1 | M | `wallet.sqlite` / `mweb.sqlite` / MWEB sync+history files are unencrypted at rest (addresses, balances, history — not keys). `sealing_key()` exists in the secret store but nothing consumes it | Encrypt MWEB store + metadata under the sealing key, or document FileVault/LUKS reliance permanently |
| O2 | M | MWEB sync trusts a single peer per pass — a malicious peer can omit UTXOs (understate balance) | Cross-check leaf counts/roots across ≥2 peers, or verify against Electrum-reported MWEB kernel data |
| O3 | M | Electrum fallback silently appends public default servers after the user-configured one (`electrum_candidates` in `app.rs`). A user running their own server for privacy leaks their addresses to public servers whenever theirs is briefly down; the switch is only logged to stderr | Add a "use public fallback servers" setting (default on), surface the active server in the UI |
| O4 | L | Argon2id parameters (19 MiB, t=2, p=1) are the OWASP minimum | Raise (e.g. 64 MiB+) or calibrate to ~250 ms on target hardware; needs a re-encryption migration |
| O5 | L | Mnemonic crosses the IPC boundary as a plain `String` on create/restore and lives in webview memory until backup confirm | Inherent to showing the phrase for backup; keep the DOM-clear behavior; consider a native reveal window later |
| O6 | L | `tcp://` Electrum URLs are allowed (plaintext protocol) | Warn in UI when a non-localhost `tcp://` URL is configured |
| O7 | I | No Tor/proxy support; DNS-seed MWEB discovery reveals the user's IP | Roadmap item |
| O8 | I | Builds are not bit-for-bit reproducible (`.dmg`/`.AppImage` timestamps, toolchain drift) | Pin toolchain everywhere (done in workflows), strip timestamps, publish build environment details |
| O9 | I | Release workflow does not require green CI on the same commit before building | Protect the `release` branch with required status checks |
| O10 | I | macOS releases unsigned until Apple Developer credentials exist | CI signing is already drop-in; enroll and add the six secrets |

## Scope notes for a future third-party audit

1. **Treat the sibling forks as primary targets**: `IndigoNakamoto/bdk`
   (`crates/mweb` especially), `bdk_wallet`, `rust-litecoin`,
   `rust-electrum-client`, `rust-miniscript` at the SHAs in `deps/pins.env` /
   `Cargo.lock`. All consensus and MWEB cryptography lives there, not in this
   repo.
2. Priority order inside this repo: `crates/wallet-core/src/secrets.rs`,
   `seed.rs`, `aezeed.rs`, `descriptors.rs`, `mweb.rs`, `electrum.rs`,
   `rpc.rs`, `discovery.rs`, then the Tauri command surface
   (`src-tauri/src/lib.rs`) and `ui/src/main.ts`.
3. Freeze a tag before the engagement; the pins file makes the full source
   tree reproducible for the auditors.
