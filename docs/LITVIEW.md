# Litview / LRK integration

First-party companion to [litview.space](https://litview.space) (sibling checkout
`../lrk`, Litecoin Research Kit). BDK / `wallet-core` remains the wallet; LRK
stays a remote chain index + explorer. The wallet does **not** embed the LRK
indexer or copy the `website_next` SPA.

## Architecture

```text
ui  --invoke-->  src-tauri commands  -->  wallet-core
                                            |-- Electrum / MWEB (BDK)
                                            '-- ureq HTTPS --> litview or self-host
ui  --open_explorer_url-->  system browser  -->  {explorer}/tx/{txid}
```

- All litview HTTP goes through Rust (`ureq`). WebView CSP stays IPC-only
  (`connect-src` does not include litview).
- Deep links open in the OS browser; the WebView never navigates to litview.
- Explorer settings live on `WalletMeta` / `WalletSettings` next to Electrum.

## Privacy matrix

| Action | Trigger | What litview learns | Default |
| --- | --- | --- | --- |
| “View on litview” | User click | IP + txid (browser) | Button visible |
| Tx enrichment `GET /api/tx/{txid}` | User opens tx detail | IP + that txid | On when detail opens; in-session cache |
| Spot price `GET /api/mempool/price` | Auto while unlocked (~60s) | IP only | On; Settings toggle |
| Fee ladder `GET /api/v1/fees/recommended` | Send form / preview | IP only | On; chips are suggestions |
| Tip height cross-check | Automatic | IP only | Deferred (Electrum header check exists) |
| Address / xpub scan | — | IP + address set | **Never** |

Wallet addresses are never uploaded. When enrichment returns vin/vout, the
wallet marks `is_wallet` locally by matching revealed SPKs.

## API surface

Settings (serde defaults for older wallets):

- `explorer_base_url` — default `https://litview.space`
- `show_fiat` — default `true`
- `use_explorer_fee_hints` — default `true`

Helpers / commands:

- `explorer_tx_url(txid)` / `explorer_block_url(hash)` — no network
- `open_explorer_url(url)` — OS browser
- `fetch_tx_detail(txid)` → `TxEnrichment` from `/api/tx/{txid}`
- `fetch_spot_price()` → `/api/mempool/price`
- `fetch_fee_ladder()` → `/api/v1/fees/recommended`

Hand-rolled GETs against the mempool.space-compatible JSON; no `brk_client` dep.

## Phases

1. **Deep links** — Settings base URL, “View on litview”, SECURITY note.
2. **Enrichment** — vin/vout in tx modal, wallet highlighting, soft fail.
3. **Price + fee hints** — USD under balance, fee chips on send, toggles.

## Non-goals

- Embedding LRK / Litecoin Core
- Porting explore cube, charts, Learn, mining pools, or litview Wallets scan
- `BrkClient` in the WebView
- Address history from litview, litview tip cross-check, Tor for explorer traffic
