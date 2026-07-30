# ltc-wallet-mac

Native macOS Litecoin wallet built on the Litecoin BDK fork ([`IndigoNakamoto/bdk`](https://github.com/IndigoNakamoto/bdk) + [`bdk_wallet`](https://github.com/IndigoNakamoto/bdk_wallet)), with a Tauri shell.

## Status

Scaffold + architecture only. **v0.1** = transparent BIP84 (Electrum sync / send). **v0.2** = MWEB.

Read [`docs/CHAT_HANDOFF.md`](docs/CHAT_HANDOFF.md) before implementing. Full blueprint: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Expected sibling checkouts

```text
../bdk          # branch litecoin
../bdk_wallet   # branch litecoin
```

## Layout

| Path | Role |
| --- | --- |
| `crates/wallet-core` | BDK boundary, DTOs, Keychain-backed mnemonic |
| `src-tauri` | Tauri commands (not scaffolded yet) |
| `ui` | Frontend (not scaffolded yet) |

## Next step

Implement `wallet-core` BIP84 `PersistedWallet` create/load + Electrum sync against Litecoin testnet.
