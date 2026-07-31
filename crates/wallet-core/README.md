# wallet-core

Rust library that owns BDK wallet lifecycle for the Mac app.

Public surface: `WalletApp` (`exists` / `create` / `restore` / `load` / `sync` / `summary` / `receive_address` / `send`) with serde DTOs only. See [`../../docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md).

BIP84 descriptors + `PersistedWallet` (`rusqlite`); mnemonic via `SecretStore` (file-backed `wallet.mnemonic`, mode 0600). Sync/send use Electrum-LTC (`bdk_electrum`).

Smoke against testnet with `wallet-cli` (see root README).
