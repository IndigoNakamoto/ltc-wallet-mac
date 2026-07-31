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

MWEB peers default to `127.0.0.1:9333` (user-configurable). Public Electrum servers often need `validate_domain(false)`.

## `wallet-core` surface

- `exists` / `create` / `restore` / `load` / `wipe`
- `unlock` / `lock` / `migrate_encrypt` / `is_locked` / `needs_migration`
- `sync` (transparent + best-effort MWEB tip sync)
- `summary` / `combined_summary` / `receive_address` / `mweb_receive_address`
- `transactions` / `send` (optional `drain`)
- `settings` / `update_settings`
- `pegin` / `mweb_send` / `pegout` / `resync_mweb`

## Peg-in UX model

Peg-in is a **self-transfer** from the wallet’s own transparent UTXOs. Exchanges fund the normal `ltc1` address; the app then offers “Move to private (peg-in)”. Maturity: 6 blocks.

## Screens

Boot → Unlock | Migrate | Onboarding → Mnemonic backup → Home (balance, QR, send, history, MWEB, settings).

## Implementation status

1. ~~wallet-core BIP84 + CLI + Tauri + UI polish + mainnet default~~
2. ~~Usability: history, LTC amounts, send-max, auto-refresh~~
3. ~~Hardening: encrypted mnemonic, Electrum settings~~
4. ~~Packaging prep: icon, bundle metadata, entitlements, release docs~~
5. ~~MWEB store + tip seam + peg-in/send/pegout commands + UI~~
6. Live MWEB E2E against archive peer + RPC; notarized ship

## Out of scope (still)

Multi-peer UTXO-omission detection, embedded litecoind, hardware wallets, fine-window sync in UI, universal binary.
