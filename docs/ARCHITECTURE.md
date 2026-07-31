# Architecture — Litecoin Mac wallet

Status: blueprint for v0.1 (transparent) and v0.2 (MWEB).  
Derived from planning against the Litecoin BDK fork (`IndigoNakamoto/bdk` + `bdk_wallet`).

## Why this shape

1. **Transparent-first** — Lock Tauri, SQLite, and key storage around BIP84 before MWEB crypto/LIP sync.
2. **Electrum-first** — Avoid testnet Esplora tip lag during UI development.
3. **Rust-native shell** — Tauri consumes `bdk_wallet` directly; no Swift/UniFFI until needed.

Rejected for the library backend (see BDK `MWEB_ARCHITECTURE.md`): embedding Nexus/GPL, Go `mwebd`, inventing proprietary PSBT maps.

## Repository layout

```text
ltc-wallet-mac/
  docs/
    CHAT_HANDOFF.md       # short decisions for new Cursor chats
    ARCHITECTURE.md       # this file
  crates/wallet-core/     # BDK boundary + DTOs + file secrets
  src-tauri/              # Tauri app (commands → wallet-core)
  ui/                     # Frontend (receive / balance / send)
```

Sibling checkouts (expected on a dev machine):

```text
/Users/indigo/Dev/bdk
/Users/indigo/Dev/bdk_wallet
/Users/indigo/Dev/ltc-wallet-mac   ← this repo
```

## Layering

```text
┌─────────────────────────────────────┐
│  ui/  (TS/HTML)                     │
│  invoke("sync_wallet") etc.         │
└─────────────────┬───────────────────┘
                  │ serde JSON DTOs
┌─────────────────▼───────────────────┐
│  src-tauri  commands                │
│  spawn_blocking → WalletApp         │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│  wallet-core                        │
│  PersistedWallet + Electrum        │
│  FileSecretStore (wallet.mnemonic)  │
│  (v0.2) MwebStore + LIP peer        │
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│  bdk_wallet / bdk_electrum / …      │
│  litecoin (aliased as bitcoin)      │
└─────────────────────────────────────┘
```

## Network and descriptors

| User-facing | `bitcoin::Network` (litecoin crate) | BIP84 path |
| --- | --- | --- |
| Litecoin mainnet | `Network::Bitcoin` | `m/84'/2'/0'/{0,1}/*` |
| Litecoin testnet | `Network::Testnet4` | `m/84'/1'/0'/{0,1}/*` |

Descriptors are BIP84 `wpkh` external/internal. Format the coin type from the selected network so testnet paths never land on mainnet wallets.

Addresses: `ltc1…` mainnet, `tltc1…` testnet.

## `wallet-core` API (v0.1)

Keep BDK types private. Public surface:

```rust
pub struct WalletApp { /* Mutex<WalletState> */ }

impl WalletApp {
    pub fn exists(&self, data_dir: &Path) -> bool;
    pub fn create(&self, data_dir: &Path, req: CreateWalletRequest)
        -> Result<CreateWalletResponse, WalletError>;
    pub fn restore(&self, data_dir: &Path, req: RestoreWalletRequest)
        -> Result<WalletSummary, WalletError>;
    pub fn load(&self, data_dir: &Path) -> Result<WalletSummary, WalletError>;
    pub fn sync(&self) -> Result<SyncResult, WalletError>;
    pub fn summary(&self) -> Result<WalletSummary, WalletError>;
    pub fn receive_address(&self) -> Result<String, WalletError>;
    pub fn send(&self, req: SendRequest) -> Result<SendResult, WalletError>;
}
```

### DTOs

- `WalletSummary` — network, balance buckets (litoshis), tip height, receive address
- `SyncResult` — summary + `new_txs`
- `SendRequest` — `address`, `amount_sats`, `fee_rate_sat_vb` (required, no guessing)
- `SendResult` — `txid`, `fee_sats`
- `CreateWalletRequest` / `CreateWalletResponse` — network, optional electrum URL; mnemonic returned **once**
- `RestoreWalletRequest` — mnemonic + network

### Persistence

- Transparent wallet: `PersistedWallet` + `rusqlite` under  
  `~/Library/Application Support/<bundle-id>/wallet.sqlite`
- Mnemonic: `wallet.mnemonic` beside the DB (mode `0600`); Keychain deferred until reliable
- v0.2: `MwebStore` beside the wallet (`mweb.db` + optional `mweb_sync.json`) — never merge confidential coins into `IndexedTxGraph`

### Sync / send internals

```text
create/restore → BIP84 descriptors → PersistedWallet::create
load          → PersistedWallet::load

sync
  → BdkElectrumClient
  → full_scan (first/restore) or sync (revealed SPKs)
  → Wallet::apply_update → persist

send
  → TxBuilder + FeeRate::from_sat_per_vb
  → sign → ElectrumClient::transaction_broadcast
  → persist
  → caller runs sync before trusting UI balance
```

### Concurrency

- Single `WalletApp` in Tauri managed state (`Arc`)
- Lock `Mutex` only for BDK mutations; do not hold across `.await`
- All Electrum I/O inside `spawn_blocking`

## Tauri commands (v0.1)

| Command | Core method |
| --- | --- |
| `wallet_exists` | `exists` |
| `create_wallet` | `create` |
| `restore_wallet` | `restore` |
| `load_wallet` | `load` |
| `sync_wallet` | `sync` |
| `get_summary` | `summary` |
| `get_receive_address` | `receive_address` |
| `send_ltc` | `send` |

Map `WalletError` → `String` at the boundary for simple frontend toasts.

## UI state machine

```text
boot
  ├─ exists? no  → onboarding → create → mnemonic backup → ready
  │                         └→ restore → sync → ready
  └─ exists? yes → load → ready → background sync
```

Phases: `boot` | `onboarding` | `mnemonic` | `ready` | `fatal`.

Flags: `syncing`, `sending`, `error`, `lastTxid`. Disable Send while `syncing || sending`. After send: await broadcast → sync → replace summary (no optimistic balance).

Default testnet fee rate for early builds: `1` sat/vB (matches BDK Electrum E2E).

## v0.2 MWEB (deferred)

- Feature-flag `mweb` on `bdk_wallet`
- Tip seam: Electrum/Esplora tip → `MwebSyncer` / LIP-0006 peer (`LITECOIN_P2P`)
- Peg-in maturity 6 blocks; pure MWEB broadcast prefers node RPC + **wtxid**
- Combined balance via `balance_combined`; bifurcated coin DBs
- Do not index HogAddr / peg-in bridge outs as transparent UTXOs

Ops reference: sibling `bdk/docs/MWEB_PEER_OPS.md`, `LITECOIN_E2E.md` (`mainnet_mweb`).

## Security notes (MVP)

- Never log mnemonics or descriptors with secrets
- Clear create-response mnemonic from frontend memory after backup confirm
- Mnemonic file permissions `0600`; Keychain migration later if needed
- Notarization / hardened runtime: later packaging milestone

## Implementation order

1. `wallet-core` + CLI smoke (create / sync / address / send testnet)
2. Tauri scaffold + commands printing JSON summary
3. Onboarding + mnemonic + Home + Send
4. Packaging (icon, bundle id, notarization)
5. MWEB v0.2
