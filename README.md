# ltc-wallet-mac

Native macOS Litecoin wallet built on the Litecoin BDK fork ([`IndigoNakamoto/bdk`](https://github.com/IndigoNakamoto/bdk) + [`bdk_wallet`](https://github.com/IndigoNakamoto/bdk_wallet)), with a Tauri 2 shell.

## Status

**v0.1** — BIP84 create/load, Electrum sync/send, encrypted mnemonic, receive QR, history, LTC amounts, auto-refresh.

**v0.2 (in progress)** — MWEB peg-in / private send / peg-out via LIP-0006 P2P + optional litecoind RPC.

Read [`docs/CHAT_HANDOFF.md`](docs/CHAT_HANDOFF.md). Blueprint: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Expected sibling checkouts

```text
../bdk              # branch litecoin (includes nested bdk_wallet)
../rust-litecoin    # litecoin 0.32.8-rc.2 (workspace [patch])
```

Pin exact commits of these siblings in release notes for reproducible builds.

## Layout

| Path | Role |
| --- | --- |
| `crates/wallet-core` | BDK boundary, DTOs, encrypted secrets, Electrum, MWEB |
| `crates/wallet-cli` | Smoke CLI: create → sync → address → send |
| `src-tauri` | Tauri 2 commands → `wallet-core` |
| `ui` | Onboarding / home / settings UI |

## Dev

```bash
npm install
npm run tauri dev
```

Wallet data: `~/Library/Application Support/com.indigonakamoto.ltc-wallet/`.

Mnemonic is stored encrypted (`wallet.mnemonic.enc`). Existing plaintext `wallet.mnemonic` files are migrated on first unlock.

## Mainnet CLI smoke

```bash
cargo run -p wallet-cli -- --data-dir .wallet-data create --passphrase '…'
cargo run -p wallet-cli -- --data-dir .wallet-data --passphrase '…' address
cargo run -p wallet-cli -- --data-dir .wallet-data --passphrase '…' sync
cargo run -p wallet-cli -- --data-dir .wallet-data --passphrase '…' send \
  --address <ltc1…> --amount-sats 5000 --fee-rate 1
```

Use `--network testnet` for testnet. Passphrase can also come from `WALLET_PASSPHRASE`.

## Packaging (macOS)

1. Icon source: `app-icon.png` (regenerate with `npx tauri icon app-icon.png`).
2. Unsigned local build (Apple Silicon):

```bash
npm run tauri build
```

Artifacts under `src-tauri/target/release/bundle/`.

3. Signed + notarized release (requires Apple Developer Program):

```bash
export APPLE_ID='you@example.com'
export APPLE_PASSWORD='app-specific-password'
export APPLE_TEAM_ID='XXXXXXXXXX'
# Set bundle.macOS.signingIdentity in tauri.conf.json to your "Developer ID Application: …"
npm run tauri build
```

Hardened runtime + network client entitlement are enabled via [`src-tauri/Entitlements.plist`](src-tauri/Entitlements.plist). Universal (x86_64 + arm64) builds are deferred until sibling deps cross-compile cleanly.

## Next step

Polish MWEB UX against a live archive peer + RPC; then notarized distribution.
