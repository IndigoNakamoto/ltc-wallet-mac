# wallet-core

Rust library that owns BDK wallet lifecycle for the Mac app.

**Not implemented yet.** See [`../../docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md) for the intended API (`WalletApp`, DTOs, Electrum sync, `keyring` mnemonic storage).

Planned first slice: BIP84 descriptor generation + `PersistedWallet` create/load with `rusqlite`, path-depending sibling `bdk` / `bdk_wallet` `litecoin` branches.
