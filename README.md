# ltc-wallet-mac

Native macOS Litecoin wallet built on the Litecoin BDK fork ([`IndigoNakamoto/bdk`](https://github.com/IndigoNakamoto/bdk) + [`bdk_wallet`](https://github.com/IndigoNakamoto/bdk_wallet)), with a Tauri shell.

## Status

**v0.1 in progress** — transparent BIP84 create/load + Electrum sync/send via `wallet-core` and `wallet-cli`. **v0.2** = MWEB. Tauri/UI not scaffolded yet.

Read [`docs/CHAT_HANDOFF.md`](docs/CHAT_HANDOFF.md) before implementing. Full blueprint: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Expected sibling checkouts

```text
../bdk              # branch litecoin (includes nested bdk_wallet)
../rust-litecoin    # litecoin 0.32.8-rc.2 (workspace [patch])
```

## Layout

| Path | Role |
| --- | --- |
| `crates/wallet-core` | BDK boundary, DTOs, Keychain-backed mnemonic, Electrum sync/send |
| `crates/wallet-cli` | Smoke CLI: create → sync → address → send |
| `src-tauri` | Tauri commands (not scaffolded yet) |
| `ui` | Frontend (not scaffolded yet) |

## Testnet smoke (Electrum)

```bash
cargo run -p wallet-cli -- --data-dir .wallet-data create
cargo run -p wallet-cli -- --data-dir .wallet-data address
# fund the tltc1… address (e.g. CypherFaucet)
cargo run -p wallet-cli -- --data-dir .wallet-data sync
cargo run -p wallet-cli -- --data-dir .wallet-data send \
  --address <tltc1…> --amount-sats 5000 --fee-rate 1
cargo run -p wallet-cli -- --data-dir .wallet-data sync
```

Default Electrum: `ssl://electrum-ltc.bysh.me:51002` (testnet).

## Next step

Scaffold Tauri; wire commands to `wallet-core`; then UI screens.
