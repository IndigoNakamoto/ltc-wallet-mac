# Chat handoff — Litecoin Mac wallet (v0.1 / v0.2)

Paste or `@`-reference this file when starting a new Cursor chat in this repo.

## Decision summary

- **Product:** Native Mac Litecoin wallet (Tauri 2 + Rust core + web UI).
- **v0.1:** Transparent BIP84 (receive / sync / send / history). Encrypted mnemonic at rest.
- **v0.2:** MWEB via `bdk_wallet` `mweb` + `bdk_mweb` + LIP-0006 peer (peg-in, private send, peg-out).
- **Sync backend (transparent):** Electrum-LTC first.
- **MWEB sync:** LIP-0006 P2P to archive litecoind (not Electrum). Pure MWEB broadcast requires litecoind RPC; track **wtxid**.
- **Library deps:** Path-dep sibling checkouts:
  - `../bdk` (`IndigoNakamoto/bdk`, branch `litecoin`)
  - nested `../bdk/bdk_wallet`
  - `../rust-litecoin` via workspace `[patch]`
- **Alias rule:** Cargo `bitcoin` → `litecoin` crate.
  - Litecoin **mainnet** = `Network::Bitcoin`, BIP84 coin type **`2`**
  - Litecoin **testnet** = `Network::Testnet4`, coin type **`1`**
- **Boundary:** UI/Tauri never see BDK types. `wallet-core` exposes serde DTOs only.
- **Secrets:** Argon2id + ChaCha20-Poly1305 `wallet.mnemonic.enc` (legacy plaintext migrated on unlock). Mode `0600`. Never store mnemonic in SQLite.
- **Concurrency:** Electrum/BDK/MWEB calls are blocking → `spawn_blocking` + `Mutex<WalletState>`.
- **UX:** No optimistic balance after send — sync, then refresh. Amounts in LTC (string decimal → litoshis). Dust floor ~2940 litoshis for `ltc1`. Auto-sync every 60s (status-line errors only).

## Default endpoints

| Network | Electrum |
| --- | --- |
| mainnet | `ssl://electrum-ltc.bysh.me:50002` |
| testnet | `ssl://electrum-ltc.bysh.me:51002` |

MWEB peers default to `127.0.0.1:9333` (user-configurable). Electrum TLS certificate validation is on by default (cipig.net defaults have CA certs); the self-signed community servers need the Settings toggle off.

## `wallet-core` surface

- `exists` / `create` / `restore` / `load` / `wipe`
- `unlock` / `lock` / `migrate_encrypt` / `is_locked` / `needs_migration`
- `sync` (transparent + best-effort MWEB tip sync)
- `summary` / `combined_summary` / `receive_address` / `mweb_receive_address`
- `transactions` / `send` (optional `drain`)
- `settings` / `update_settings` (includes `explorer_base_url`, `show_fiat`, `use_explorer_fee_hints`)
- `explorer_tx_url` / `open_explorer_url` / `fetch_tx_detail` / `fetch_spot_price` / `fetch_fee_ladder`
- `get_tx_labels` / `set_tx_label` / `export_history`
- `list_contacts` / `upsert_contact` / `delete_contact`
- `list_unspent` / `set_utxo_locked` (Public coin control; `SendRequest.selected_outpoints`)
- `pegin` / `mweb_send` / `pegout` / `resync_mweb`

## Peg-in UX model

Peg-in is a **self-transfer** from the wallet’s own transparent UTXOs. Exchanges fund the normal `ltc1` address; the app then offers “Move to private (peg-in)”. Maturity: 6 blocks.

## Screens

Boot → Unlock | Migrate | Onboarding → Mnemonic backup → verify quiz → Home (balance, QR, send, history, MWEB, settings).

## UX backlog ([`docs/UX_REVIEW.md`](UX_REVIEW.md))

- **Done (P0):** Hard-gated recovery-phrase quiz after create; wallet passphrase minimum 8 chars + strength meter on Create/Migrate/Restore; first-funds backup banner for unverified/legacy installs; send/swap review shows full destination, type badge, total leaving wallet, and high-fee warning (≥50% of amount).
- **Done (P1):** Public/Private first-use coach (`ltc-mweb-coach-seen`); empty-wallet funding CTA → Public Receive; peg-in maturity as spendable vs maturing (+ unconfirmed private, History maturing pill); Swap dual-fee labels + MWEB “no explorer” success copy / kind-aware litview; progressive security checklist at ≥1 LTC (`ltc-security-checklist-dismissed`).
- **Done (P2):** Display unit LTC|litoshis (`ltc-display-unit`, Settings + hero tap); Public BIP21 amount/label QR + copy payment link + Send URI parse; fee chips time labels + Economy + custom sat/vB + `estimate_fee` when explorer hints off; receive toast/history pulse + first-receive modal (`ltc-first-receive-seen`).
- **Done (P3):** Hide balances (`ltc-hide-balances`, Settings + hero LTC→litoshis→hidden); send-side transparent reuse warn via `address_reuse_hint` (warn-only; Private never warns); Settings “What leaves this computer” panel; tx labels in wipeable `tx_labels.json` sidecar (confirm note + History/detail edit).
- **Done (P4 shippable):** History search/filter + CSV/JSON export; contacts (`contacts.json`, name + one address + Public/Private, Send picker); Public coin control (`list_unspent`, freeze, opt-in `selected_outpoints`).
- **Next (P4 deferred):** Multi-wallet, hardware wallets, Tor/proxy — future architecture.

## Implementation status

1. ~~wallet-core BIP84 + CLI + Tauri + UI polish + mainnet default~~
2. ~~Usability: history, LTC amounts, send-max, auto-refresh~~
3. ~~Hardening: encrypted mnemonic, Electrum settings~~
4. ~~Packaging prep: icon, bundle metadata, entitlements, release docs~~
5. ~~MWEB store + tip seam + peg-in/send/pegout commands + UI~~
6. Live MWEB E2E against archive peer + RPC; notarized ship
7. UX P0 fund-loss safety (backup verify, passphrase gate, send confirm)
8. UX P1 MWEB comprehension (coach, funding CTA, maturity, Swap fees/explorer, security checklist)
9. UX P2 payment polish (units, BIP21, fee clarity, receive feedback)
10. UX P3 privacy hardening (hide balance, reuse warn, disclosure, labels)
11. UX P4 shippable (history search/export, contacts, Public coin control)

## Litview / LRK

First-party explorer at [litview.space](https://litview.space). Design and privacy
matrix: [`docs/LITVIEW.md`](LITVIEW.md). Deep links + optional enrichment/price/fees
via Rust `ureq`; never scan wallet addresses against litview.

## Out of scope (still)

Multi-peer UTXO-omission detection, embedded litecoind, hardware wallets, fine-window sync in UI, universal binary.
