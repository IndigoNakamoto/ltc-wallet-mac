# Local development against sibling fork checkouts

The committed manifests pin `IndigoNakamoto/bdk`, `bdk_wallet`, and
`rust-litecoin` by git rev, so a plain clone builds standalone and CI/releases
are reproducible. When hacking on the forks and the wallet together, override
the pins locally instead of editing the manifests.

Clone the forks as siblings of this repo:

```text
Dev/
├── ltc-wallet-mac/   this repo
├── bdk/              IndigoNakamoto/bdk        (litecoin)
├── bdk_wallet/       IndigoNakamoto/bdk_wallet (litecoin)
└── rust-litecoin/    IndigoNakamoto/rust-litecoin
```

Then add a gitignored `.cargo/config.toml` at this repo's root:

```toml
[patch."https://github.com/IndigoNakamoto/bdk.git"]
bdk_chain = { path = "../bdk/crates/chain" }
bdk_mweb = { path = "../bdk/crates/mweb" }
bdk_electrum = { path = "../bdk/crates/electrum" }

[patch."https://github.com/IndigoNakamoto/bdk_wallet.git"]
bdk_wallet = { path = "../bdk_wallet" }
```

`cargo build` now uses your local checkouts. Delete the file to go back to the
pinned revs. Do not commit it: the pins in `Cargo.toml`/`Cargo.lock` are what
CI and releases build from.

Note that patching `litecoin` locally is different: it is already patched in
the root `Cargo.toml` (`[patch.crates-io]`), and Cargo rejects patching the
same package from both the manifest and config. To hack on rust-litecoin,
temporarily change the manifest patch to
`litecoin = { path = "../rust-litecoin/litecoin" }` and revert before
committing.

## Bumping the pins for real

Order matters (leaves to root):

1. Push the `bdk` change; note the SHA.
2. `bdk_wallet`: update its `bdk.git` revs to that SHA, push; note the SHA.
3. This repo: update the `bdk_wallet.git` rev and the `bdk.git` revs in
   `crates/wallet-core/Cargo.toml` (they must match what `bdk_wallet` pins,
   or the build fails on duplicate-crate type mismatches), then
   `cargo check --workspace` to refresh `Cargo.lock`, and commit both.
