# Chat handoff — Litecoin Mac wallet (v0.1)

Paste or `@`-reference this file when starting a new Cursor chat in this repo.

## Decision summary

- **Product:** Native Mac Litecoin wallet (Tauri 2 + Rust core + web UI).
- **v0.1:** Transparent BIP84 only (receive / sync / send). No MWEB.
- **v0.2:** MWEB via `bdk_wallet` `mweb` feature + `bdk_mweb` + LIP-0006 peer.
- **Sync backend (v0.1):** Electrum-LTC first. Esplora optional later (testnet Esplora has lagged).
- **Library deps:** Litecoin forks — path-dep sibling checkouts preferred:
  - `../bdk` (`IndigoNakamoto/bdk`, branch `litecoin`)
  - `../bdk_wallet` (`IndigoNakamoto/bdk_wallet`, branch `litecoin`)
- **Alias rule:** Cargo `bitcoin` → `litecoin` crate. In API terms:
  - Litecoin **mainnet** = `Network::Bitcoin`, BIP84 coin type **`2`** → `m/84'/2'/0'`
  - Litecoin **testnet** = `Network::Testnet4`, coin type **`1`** → `m/84'/1'/0'`
- **Boundary:** UI/Tauri never see BDK types. `wallet-core` exposes serde DTOs only.
- **Secrets:** Mnemonic in App Support as `wallet.mnemonic` (mode `0600`) via `FileSecretStore`. Keychain/`keyring` was abandoned: set succeeded but did not persist across Entry/process restart on current macOS. Never store mnemonic in SQLite.
- **Concurrency:** Electrum/BDK calls are blocking → `tauri::async_runtime::spawn_blocking` + `Mutex<WalletState>`.
- **UX:** No optimistic balance after send — sync, then refresh summary. Explicit fee rate (sat/vB). Label amounts as LTC/litoshis (rust-litecoin may still print “BTC”).

## Default endpoints

| Network | Electrum |
| --- | --- |
| testnet | `ssl://electrum-ltc.bysh.me:51002` |
| mainnet | `ssl://electrum-ltc.bysh.me:50002` |

Public servers often need `validate_domain(false)` (self-signed).

## `wallet-core` surface (v0.1)

- `exists` / `create` / `restore` / `load`
- `sync` (full_scan on restore/first run; incremental sync after)
- `summary` / `receive_address`
- `send({ address, amount_sats, fee_rate_sat_vb })`

DTOs: `WalletSummary`, `SyncResult`, `SendRequest`, `SendResult`, `CreateWalletRequest/Response`, `RestoreWalletRequest`. Errors as a small `WalletError` enum mapped to strings at the Tauri boundary.

## Screens (v0.1)

Boot → Onboarding (create/restore) → Mnemonic backup (create only) → Home (balance + sync) → Receive / Send.

## Proven E2E reference (libraries)

See sibling BDK docs (do not vendor):

- `../bdk/docs/LITECOIN_E2E.md` — Electrum receive/spend loop
- `../bdk/docs/MWEB_ARCHITECTURE.md` / `MWEB_PEER_OPS.md` — v0.2 only
- `../bdk/examples/ltc-scan` — watch-only sync smoke

## Next implementation slice

1. ~~Implement `crates/wallet-core` BIP84 descriptor generation + `PersistedWallet` create/load (`rusqlite`).~~
2. ~~Tiny CLI exercising create → sync → address → send (mainnet default; `--network testnet` still available).~~
3. ~~Scaffold Tauri; wire commands; polish UI; mainnet create/restore default.~~ Next: packaging.

## Out of scope for v0.1

MWEB, peg-in/out, Esplora-as-default, multi-wallet, fee estimation UI, notarization polish.
