# ltc-wallet-mac

Native Litecoin wallet for macOS and Linux, built on the Litecoin BDK fork ([`IndigoNakamoto/bdk`](https://github.com/IndigoNakamoto/bdk) + [`bdk_wallet`](https://github.com/IndigoNakamoto/bdk_wallet)), with a Tauri 2 shell.

## Status

**v0.1** — BIP84 create/load, Electrum sync/send, encrypted mnemonic, receive QR, history, LTC amounts, auto-refresh.

**v0.2 (in progress)** — MWEB peg-in / private send / peg-out via LIP-0006 P2P + optional litecoind RPC.

Read [`docs/CHAT_HANDOFF.md`](docs/CHAT_HANDOFF.md). Blueprint: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

Security: threat model and disclosure policy in [`SECURITY.md`](SECURITY.md);
how to verify a release in [`docs/VERIFYING.md`](docs/VERIFYING.md).

## Expected sibling checkouts

```text
../bdk              # branch litecoin
../bdk/bdk_wallet   # separate repo, cloned inside ../bdk (gitignored there)
../rust-litecoin    # litecoin 0.32.8-rc.2 (workspace [patch])
```

The IndigoNakamoto/bdk, bdk_wallet, and rust-litecoin forks are pinned by rev directly in the Cargo manifests (see `crates/wallet-core/Cargo.toml` and the root `[patch.crates-io]`), backed by the committed `Cargo.lock`. Update those revs when intentionally bumping the forks; for day-to-day development against local sibling checkouts, add `[patch]` overrides in `.cargo/config.toml` (see `docs/LOCAL_DEV.md`).

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

Wallet data:

| OS | Path |
| --- | --- |
| macOS | `~/Library/Application Support/com.indigonakamoto.ltc-wallet/` |
| Linux | `~/.local/share/com.indigonakamoto.ltc-wallet/` |

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

## Packaging

Icon source: `app-icon.png` (regenerate with `npx tauri icon app-icon.png`).

Bundle targets: macOS `.app` + `.dmg`, Linux `.deb` + `.AppImage`.

### Local macOS build

```bash
npm run tauri build
```

Artifacts under `src-tauri/target/release/bundle/` (or the workspace `target/` equivalent). Share the `.dmg`, or zip the `.app`.

Unsigned builds trip Gatekeeper: recipients use Right-click → Open (or Privacy & Security → Open Anyway).

Signed + notarized release (Apple Developer Program):

```bash
export APPLE_ID='you@example.com'
export APPLE_PASSWORD='app-specific-password'
export APPLE_TEAM_ID='XXXXXXXXXX'
export APPLE_SIGNING_IDENTITY='Developer ID Application: …'
npm run tauri build
```

CI signing is drop-in: add `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID` as
repository secrets and the release workflow signs + notarizes automatically —
no workflow edits needed.

Hardened runtime + network client entitlement are enabled via [`src-tauri/Entitlements.plist`](src-tauri/Entitlements.plist). Universal (x86_64 + arm64) builds are deferred until sibling deps cross-compile cleanly.

### Local Linux build

Build on Linux (not cross from macOS). On Ubuntu/Debian:

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf xdg-utils
npm install
npm run tauri build
```

Share the `.AppImage` (`chmod +x LTC\ Wallet_*.AppImage && ./LTC\ Wallet_*.AppImage`) or install the `.deb` (`sudo dpkg -i …`).

### GitHub Releases (CI)

[`.github/workflows/release.yml`](.github/workflows/release.yml) builds **macOS (Apple Silicon)** and **Linux x64** artifacts and attaches them to a **draft** GitHub Release.

1. Keep the fork revs in the Cargo manifests pointed at known-good commits.
2. Push to the `release` branch, or run **Actions → Release → Run workflow**.
3. Open the draft release, edit notes, publish.

Each platform job attaches a `SHA256SUMS-<platform>.txt`; recipients can check
their download against it (see [`docs/VERIFYING.md`](docs/VERIFYING.md)).

## Next step

Polish MWEB UX against a live archive peer + RPC; then notarized distribution.
