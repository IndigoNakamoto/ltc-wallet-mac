# ltc-wallet-mac

Native macOS Litecoin wallet built on the Litecoin BDK fork ([`IndigoNakamoto/bdk`](https://github.com/IndigoNakamoto/bdk) + [`bdk_wallet`](https://github.com/IndigoNakamoto/bdk_wallet)), with a Tauri 2 shell.

## Status

**v0.1 in progress** — BIP84 create/load, Electrum sync/send, Tauri commands + minimal boot UI. **v0.2** = MWEB.

Read [`docs/CHAT_HANDOFF.md`](docs/CHAT_HANDOFF.md). Blueprint: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Expected sibling checkouts

```text
../bdk              # branch litecoin (includes nested bdk_wallet)
../rust-litecoin    # litecoin 0.32.8-rc.2 (workspace [patch])
```

## Layout

| Path | Role |
| --- | --- |
| `crates/wallet-core` | BDK boundary, DTOs, secrets file, Electrum |
| `crates/wallet-cli` | Smoke CLI: create → sync → address → send |
| `src-tauri` | Tauri 2 commands → `wallet-core` |
| `ui` | Minimal onboarding / home UI |

## Dev

```bash
npm install
npm run tauri dev
```

Wallet data: `~/Library/Application Support/com.indigonakamoto.ltc-wallet/`.

## Mainnet CLI smoke

```bash
cargo run -p wallet-cli -- --data-dir .wallet-data create
cargo run -p wallet-cli -- --data-dir .wallet-data address
# fund the ltc1… address
cargo run -p wallet-cli -- --data-dir .wallet-data sync
cargo run -p wallet-cli -- --data-dir .wallet-data send \
  --address <ltc1…> --amount-sats 5000 --fee-rate 1
```

Use `--network testnet` for testnet (`tltc1…`, Electrum `:51002`).

## Next step

Packaging (icon / notarization).
