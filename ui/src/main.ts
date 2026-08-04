import { invoke } from "@tauri-apps/api/core";
import QRCode from "qrcode";

type WalletNetwork = "mainnet" | "testnet";

type WalletSummary = {
  network: WalletNetwork;
  confirmed_sats: number;
  trusted_pending_sats: number;
  untrusted_pending_sats: number;
  immature_sats: number;
  total_sats: number;
  tip_height: number;
  receive_address: string;
};

type CombinedSummary = {
  transparent: WalletSummary;
  mweb_confirmed_sats: number;
  mweb_unconfirmed_sats: number;
  mweb_immature_sats: number;
  mweb_total_sats: number;
  mweb_receive_address: string | null;
  mweb_synced_height: number | null;
  mweb_stale: boolean;
  mweb_status: string;
};

type CreateWalletResponse = {
  mnemonic: string;
  summary: WalletSummary;
};

type SyncResult = {
  summary: WalletSummary;
  new_txs: number;
  electrum_ms: number;
  mweb_ms: number;
  electrum_server: string;
  warnings: string[];
};

type SendResult = {
  txid: string;
  fee_sats: number;
};

type SendPreview = {
  amount_sats: number;
  fee_sats: number;
  fee_rate_sat_vb: number;
};

type PeginPreview = {
  amount_sats: number;
  private_credit_sats: number;
  mweb_fee_sats: number;
  transparent_fee_sats: number;
  total_from_transparent_sats: number;
};

type MwebSendPreview = {
  amount_sats: number;
  fee_sats: number;
};

type PegoutPreview = {
  amount_sats: number;
  fee_sats: number;
  dust_sats: number;
};

type TxKind = "transparent" | "pegin" | "pegout" | "mweb-send" | "mweb-receive";

type ContactKind = "public" | "private";

type ContactRecord = {
  id: string;
  name: string;
  address: string;
  kind: ContactKind;
};

type UtxoRecord = {
  outpoint: string;
  txid: string;
  vout: number;
  amount_sats: number;
  keychain: string;
  confirmations: number;
  locked: boolean;
};

type TxRecord = {
  txid: string;
  net_sats: number;
  sent_sats: number;
  received_sats: number;
  fee_sats: number | null;
  height: number | null;
  confirmations: number;
  timestamp: number | null;
  kind: TxKind;
};

const TX_KIND_LABELS: Record<TxKind, string> = {
  transparent: "",
  pegin: "peg-in",
  pegout: "peg-out",
  "mweb-send": "mweb send",
  "mweb-receive": "mweb receive",
};

type MwebScheme = "litecoin-core" | "lip0004" | "mwebd";

type WalletSettings = {
  electrum_url: string;
  electrum_validate_domain: boolean;
  electrum_use_public_fallback: boolean;
  auto_lock_minutes: number;
  electrum_active_url: string | null;
  litecoin_rpc_url: string | null;
  mweb_peers: string[];
  mweb_scheme: MwebScheme;
  explorer_base_url: string;
  show_fiat: boolean;
  use_explorer_fee_hints: boolean;
};

type TxIo = {
  address: string;
  value_sats: number;
  is_wallet: boolean;
};

type TxEnrichment = {
  txid: string;
  fee_sats: number | null;
  size: number | null;
  weight: number | null;
  status: {
    confirmed: boolean;
    block_height: number | null;
    block_hash: string | null;
    block_time: number | null;
  };
  inputs: TxIo[];
  outputs: TxIo[];
};

type FeeLadder = {
  fastest_sat_vb: number;
  half_hour_sat_vb: number;
  hour_sat_vb: number;
  economy_sat_vb: number | null;
  minimum_sat_vb: number | null;
};

type FeeEstimate = {
  fee_rate_sat_vb: number;
  is_fallback: boolean;
};

type AddressReuseHint = {
  reused: boolean;
};

type DisplayUnit = "ltc" | "litoshis";

type ParsedPaymentUri = {
  address: string;
  amountSats: number | null;
  label: string | null;
};

type MwebSyncProgress = {
  active: boolean;
  fetched: number;
  total: number;
};

type Phase =
  | "boot"
  | "onboarding"
  | "mnemonic"
  | "ready"
  | "fatal"
  | "unlock"
  | "migrate";

const PHASE_LABELS: Record<Phase, string> = {
  boot: "Starting…",
  onboarding: "Set up your wallet",
  mnemonic: "Back up your phrase",
  ready: "Ready",
  fatal: "Wallet data problem",
  unlock: "Locked",
  migrate: "Encryption required",
};

/** Top-level panes. Send/Receive/Private are cards inside the Balance sheet. */
const VIEWS = ["balance", "history", "settings"] as const;
type View = (typeof VIEWS)[number];

const VIEW_TITLES: Record<View, string> = {
  balance: "Balance",
  history: "History",
  settings: "Settings",
};

const CARDS = ["send", "receive", "swap"] as const;
type Card = (typeof CARDS)[number];

const CARD_TITLES: Record<Card, string> = {
  send: "Send",
  receive: "Receive",
  swap: "Swap",
};

type StatusKind = "info" | "success" | "error";
type SyncState = "idle" | "ok" | "error";

const SYNC_TITLES: Record<SyncState, string> = {
  idle: "Not synced yet",
  ok: "Synced",
  error: "Last sync failed",
};

type ThemePref = "auto" | "light" | "dark";

const THEME_KEY = "ltc-theme";
const THEME_ORDER: ThemePref[] = ["auto", "light", "dark"];
const BACKUP_VERIFIED_KEY = "ltc-backup-verified";
const BACKUP_BANNER_DISMISSED_KEY = "ltc-backup-banner-dismissed";
const MWEB_COACH_SEEN_KEY = "ltc-mweb-coach-seen";
const SECURITY_CHECKLIST_DISMISSED_KEY = "ltc-security-checklist-dismissed";
const DISPLAY_UNIT_KEY = "ltc-display-unit";
const HIDE_BALANCES_KEY = "ltc-hide-balances";
const FIRST_RECEIVE_SEEN_KEY = "ltc-first-receive-seen";
const HIDDEN_AMOUNT = "••••";
const MAX_TX_LABEL_CHARS = 140;

const DUST_LITOSHIS = 2940;
const AUTO_SYNC_MS = 60_000;
const QR_CSS_SIZE = 176;
const RECENT_TX_COUNT = 6;
const MIN_PASSPHRASE_LEN = 8;
const QUIZ_WORD_COUNT = 3;
/** Warn when network fee is at least half the amount being sent. */
const HIGH_FEE_RATIO = 0.5;
/** Peg-in coins mature after this many blocks before private spend. */
const MWEB_PEGIN_MATURITY_BLOCKS = 6;
/** Soft threshold for the progressive security checklist (1 LTC). */
const SECURITY_CHECKLIST_SATS = 100_000_000;

const SVG_ARROW_IN =
  '<svg class="icon" viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 5v13"/><path d="m6 13 6 6 6-6"/></svg>';
const SVG_ARROW_OUT =
  '<svg class="icon" viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 19V6"/><path d="m6 11 6-6 6 6"/></svg>';

const el = {
  authShell: document.querySelector<HTMLElement>("#auth-shell")!,
  phase: document.querySelector<HTMLElement>("#phase")!,
  error: document.querySelector<HTMLElement>("#error")!,
  fatal: document.querySelector<HTMLElement>("#fatal")!,
  unlock: document.querySelector<HTMLElement>("#unlock")!,
  migrate: document.querySelector<HTMLElement>("#migrate")!,
  onboarding: document.querySelector<HTMLElement>("#onboarding")!,
  mnemonic: document.querySelector<HTMLElement>("#mnemonic")!,
  mnemonicShow: document.querySelector<HTMLElement>("#mnemonic-show")!,
  mnemonicVerify: document.querySelector<HTMLElement>("#mnemonic-verify")!,
  mnemonicQuiz: document.querySelector<HTMLElement>("#mnemonic-quiz")!,
  mnemonicQuizError: document.querySelector<HTMLElement>("#mnemonic-quiz-error")!,
  ready: document.querySelector<HTMLElement>("#ready")!,
  mnemonicText: document.querySelector<HTMLElement>("#mnemonic-text")!,
  backupBanner: document.querySelector<HTMLElement>("#backup-banner")!,
  btnBackupBannerDismiss: document.querySelector<HTMLButtonElement>("#btn-backup-banner-dismiss")!,
  maturityBanner: document.querySelector<HTMLElement>("#maturity-banner")!,
  maturityBannerText: document.querySelector<HTMLElement>("#maturity-banner-text")!,
  viewTitle: document.querySelector<HTMLElement>("#view-title")!,
  networkBadge: document.querySelector<HTMLElement>("#network-badge")!,
  syncDot: document.querySelector<HTMLElement>("#sync-dot")!,
  syncLabel: document.querySelector<HTMLElement>("#sync-label")!,
  balanceTotal: document.querySelector<HTMLElement>("#balance-total")!,
  balanceFiat: document.querySelector<HTMLElement>("#balance-fiat")!,
  balanceSats: document.querySelector<HTMLElement>("#balance-sats")!,
  balanceConfirmed: document.querySelector<HTMLElement>("#balance-confirmed")!,
  balanceMweb: document.querySelector<HTMLElement>("#balance-mweb")!,
  balanceMwebDetail: document.querySelector<HTMLElement>("#balance-mweb-detail")!,
  balanceTip: document.querySelector<HTMLElement>("#balance-tip")!,
  balancePending: document.querySelector<HTMLElement>("#balance-pending")!,
  statMweb: document.querySelector<HTMLElement>("#stat-mweb")!,
  mwebStatusCard: document.querySelector<HTMLElement>("#mweb-status-card")!,
  mwebStatus: document.querySelector<HTMLElement>("#mweb-status")!,
  mwebProgress: document.querySelector<HTMLElement>("#mweb-progress")!,
  mwebProgressFill: document.querySelector<HTMLElement>("#mweb-progress-fill")!,
  mwebProgressText: document.querySelector<HTMLElement>("#mweb-progress-text")!,
  address: document.querySelector<HTMLElement>("#address")!,
  receiveQr: document.querySelector<HTMLCanvasElement>("#receive-qr")!,
  receiveAmount: document.querySelector<HTMLInputElement>("#receive-amount")!,
  receiveAmountLabel: document.querySelector<HTMLElement>("#receive-amount-label")!,
  receiveLabel: document.querySelector<HTMLInputElement>("#receive-label")!,
  mwebQr: document.querySelector<HTMLCanvasElement>("#mweb-qr")!,
  mwebAddress: document.querySelector<HTMLElement>("#mweb-address")!,
  mwebTools: document.querySelector<HTMLElement>("#mweb-tools")!,
  sendToggle: document.querySelector<HTMLElement>("#send-toggle")!,
  sendSegPublic: document.querySelector<HTMLButtonElement>("#send-seg-public")!,
  sendSegPrivate: document.querySelector<HTMLButtonElement>("#send-seg-private")!,
  sendPublic: document.querySelector<HTMLElement>("#send-public")!,
  sendPrivate: document.querySelector<HTMLElement>("#send-private")!,
  sendBalancePublic: document.querySelector<HTMLElement>("#send-balance-public")!,
  sendBalancePrivate: document.querySelector<HTMLElement>("#send-balance-private")!,
  receiveToggle: document.querySelector<HTMLElement>("#receive-toggle")!,
  receiveSegPublic: document.querySelector<HTMLButtonElement>("#receive-seg-public")!,
  receiveSegPrivate: document.querySelector<HTMLButtonElement>("#receive-seg-private")!,
  receivePublic: document.querySelector<HTMLElement>("#receive-public")!,
  receivePrivate: document.querySelector<HTMLElement>("#receive-private")!,
  receiveBalancePublic: document.querySelector<HTMLElement>("#receive-balance-public")!,
  receiveBalancePrivate: document.querySelector<HTMLElement>("#receive-balance-private")!,
  swapSegIn: document.querySelector<HTMLButtonElement>("#swap-seg-in")!,
  swapSegOut: document.querySelector<HTMLButtonElement>("#swap-seg-out")!,
  swapIn: document.querySelector<HTMLElement>("#swap-in")!,
  swapOut: document.querySelector<HTMLElement>("#swap-out")!,
  swapBalancePublic: document.querySelector<HTMLElement>("#swap-balance-public")!,
  swapBalancePrivate: document.querySelector<HTMLElement>("#swap-balance-private")!,
  views: document.querySelector<HTMLElement>("#views")!,
  sheetBody: document.querySelector<HTMLElement>("#sheet-body")!,
  cardTx: document.querySelector<HTMLElement>("#card-tx")!,
  txListRecent: document.querySelector<HTMLUListElement>("#tx-list-recent")!,
  txEmptyRecent: document.querySelector<HTMLElement>("#tx-empty-recent")!,
  txEmptyRecentTitle: document.querySelector<HTMLElement>("#tx-empty-recent-title")!,
  txEmptyRecentHint: document.querySelector<HTMLElement>("#tx-empty-recent-hint")!,
  btnFundReceive: document.querySelector<HTMLButtonElement>("#btn-fund-receive")!,
  btnSeeAll: document.querySelector<HTMLButtonElement>("#btn-see-all")!,
  modalOverlay: document.querySelector<HTMLElement>("#modal-overlay")!,
  modalPanel: document.querySelector<HTMLElement>("#modal-panel")!,
  modalTitle: document.querySelector<HTMLElement>("#modal-title")!,
  modalBody: document.querySelector<HTMLElement>("#modal-body")!,
  modalActions: document.querySelector<HTMLElement>("#modal-actions")!,
  modalClose: document.querySelector<HTMLButtonElement>("#modal-close")!,
  loadingOverlay: document.querySelector<HTMLElement>("#loading-overlay")!,
  loadingLabel: document.querySelector<HTMLElement>("#loading-label")!,
  toast: document.querySelector<HTMLElement>("#toast")!,
  status: document.querySelector<HTMLElement>("#status")!,
  btnToastClose: document.querySelector<HTMLButtonElement>("#btn-toast-close")!,
  btnTheme: document.querySelector<HTMLButtonElement>("#btn-theme")!,
  lastTxid: document.querySelector<HTMLElement>("#last-txid")!,
  txList: document.querySelector<HTMLUListElement>("#tx-list")!,
  txEmpty: document.querySelector<HTMLElement>("#tx-empty")!,
  txEmptyTitle: document.querySelector<HTMLElement>("#tx-empty-title")!,
  txEmptyHint: document.querySelector<HTMLElement>("#tx-empty-hint")!,
  btnFundReceiveHistory: document.querySelector<HTMLButtonElement>("#btn-fund-receive-history")!,
  historyToolbar: document.querySelector<HTMLElement>("#history-toolbar")!,
  historySearch: document.querySelector<HTMLInputElement>("#history-search")!,
  historyFilterChips: Array.from(
    document.querySelectorAll<HTMLButtonElement>(".filter-chip[data-filter]"),
  ),
  btnExportHistory: document.querySelector<HTMLButtonElement>("#btn-export-history")!,
  securityChecklist: document.querySelector<HTMLElement>("#security-checklist")!,
  securityChecklistList: document.querySelector<HTMLElement>("#security-checklist-list")!,
  btnSecurityChecklistDismiss: document.querySelector<HTMLButtonElement>(
    "#btn-security-checklist-dismiss",
  )!,
  restoreMnemonic: document.querySelector<HTMLTextAreaElement>("#restore-mnemonic")!,
  createRestoreHint: document.querySelector<HTMLElement>("#create-restore-hint")!,
  restoreMwebScheme: document.querySelector<HTMLSelectElement>("#restore-mweb-scheme")!,
  restoreAezeedPass: document.querySelector<HTMLInputElement>("#restore-aezeed-pass")!,
  restorePassphrase: document.querySelector<HTMLInputElement>("#restore-passphrase")!,
  restorePassphrase2: document.querySelector<HTMLInputElement>("#restore-passphrase2")!,
  restorePassMeter: document.querySelector<HTMLElement>("#restore-pass-meter")!,
  restorePassFill: document.querySelector<HTMLElement>("#restore-pass-fill")!,
  restorePassLabel: document.querySelector<HTMLElement>("#restore-pass-label")!,
  onboardPassphrase: document.querySelector<HTMLInputElement>("#onboard-passphrase")!,
  onboardPassphrase2: document.querySelector<HTMLInputElement>("#onboard-passphrase2")!,
  onboardPassMeter: document.querySelector<HTMLElement>("#onboard-pass-meter")!,
  onboardPassFill: document.querySelector<HTMLElement>("#onboard-pass-fill")!,
  onboardPassLabel: document.querySelector<HTMLElement>("#onboard-pass-label")!,
  unlockPassphrase: document.querySelector<HTMLInputElement>("#unlock-passphrase")!,
  migratePassphrase: document.querySelector<HTMLInputElement>("#migrate-passphrase")!,
  migratePassphrase2: document.querySelector<HTMLInputElement>("#migrate-passphrase2")!,
  migratePassMeter: document.querySelector<HTMLElement>("#migrate-pass-meter")!,
  migratePassFill: document.querySelector<HTMLElement>("#migrate-pass-fill")!,
  migratePassLabel: document.querySelector<HTMLElement>("#migrate-pass-label")!,
  sendForm: document.querySelector<HTMLFormElement>("#send-form")!,
  sendAddress: document.querySelector<HTMLInputElement>("#send-address")!,
  btnPickContactPublic: document.querySelector<HTMLButtonElement>("#btn-pick-contact-public")!,
  btnPickContactPrivate: document.querySelector<HTMLButtonElement>("#btn-pick-contact-private")!,
  contactsList: document.querySelector<HTMLUListElement>("#contacts-list")!,
  contactsEmpty: document.querySelector<HTMLElement>("#contacts-empty")!,
  btnContactAdd: document.querySelector<HTMLButtonElement>("#btn-contact-add")!,
  sendAmount: document.querySelector<HTMLInputElement>("#send-amount")!,
  sendDrain: document.querySelector<HTMLInputElement>("#send-drain")!,
  coinControl: document.querySelector<HTMLDetailsElement>("#coin-control")!,
  coinControlSum: document.querySelector<HTMLElement>("#coin-control-sum")!,
  utxoList: document.querySelector<HTMLUListElement>("#utxo-list")!,
  utxoEmpty: document.querySelector<HTMLElement>("#utxo-empty")!,
  btnRefreshUtxos: document.querySelector<HTMLButtonElement>("#btn-refresh-utxos")!,
  peginCoinControl: document.querySelector<HTMLDetailsElement>("#pegin-coin-control")!,
  peginCoinControlSum: document.querySelector<HTMLElement>("#pegin-coin-control-sum")!,
  peginUtxoList: document.querySelector<HTMLUListElement>("#pegin-utxo-list")!,
  peginUtxoEmpty: document.querySelector<HTMLElement>("#pegin-utxo-empty")!,
  btnRefreshPeginUtxos: document.querySelector<HTMLButtonElement>("#btn-refresh-pegin-utxos")!,
  feeChips: document.querySelector<HTMLElement>("#fee-chips")!,
  feeChipRow: document.querySelector<HTMLElement>("#fee-chip-row")!,
  feeCustomField: document.querySelector<HTMLElement>("#fee-custom-field")!,
  feeCustom: document.querySelector<HTMLInputElement>("#fee-custom")!,
  sendFeeHint: document.querySelector<HTMLElement>("#send-fee-hint")!,
  sendAmountLabel: document.querySelector<HTMLElement>("#send-amount-label")!,
  mwebSendAmountLabel: document.querySelector<HTMLElement>("#mweb-send-amount-label")!,
  peginAmountLabel: document.querySelector<HTMLElement>("#pegin-amount-label")!,
  pegoutAmountLabel: document.querySelector<HTMLElement>("#pegout-amount-label")!,
  settingsExplorer: document.querySelector<HTMLInputElement>("#settings-explorer")!,
  settingsShowFiat: document.querySelector<HTMLInputElement>("#settings-show-fiat")!,
  settingsUnitLtc: document.querySelector<HTMLInputElement>("#settings-unit-ltc")!,
  settingsUnitLitoshis: document.querySelector<HTMLInputElement>("#settings-unit-litoshis")!,
  settingsHideBalances: document.querySelector<HTMLInputElement>("#settings-hide-balances")!,
  settingsFeeHints: document.querySelector<HTMLInputElement>("#settings-fee-hints")!,
  settingsElectrum: document.querySelector<HTMLInputElement>("#settings-electrum")!,
  settingsValidateTls: document.querySelector<HTMLInputElement>("#settings-validate-tls")!,
  settingsPublicFallback: document.querySelector<HTMLInputElement>("#settings-public-fallback")!,
  settingsActiveServer: document.querySelector<HTMLElement>("#settings-active-server")!,
  settingsAutoLock: document.querySelector<HTMLInputElement>("#settings-auto-lock")!,
  settingsRpc: document.querySelector<HTMLInputElement>("#settings-rpc")!,
  settingsPeers: document.querySelector<HTMLInputElement>("#settings-peers")!,
  settingsMwebScheme: document.querySelector<HTMLSelectElement>("#settings-mweb-scheme")!,
  peginAmount: document.querySelector<HTMLInputElement>("#pegin-amount")!,
  peginDrain: document.querySelector<HTMLInputElement>("#pegin-drain")!,
  mwebSendAddress: document.querySelector<HTMLInputElement>("#mweb-send-address")!,
  mwebSendAmount: document.querySelector<HTMLInputElement>("#mweb-send-amount")!,
  mwebSendDrain: document.querySelector<HTMLInputElement>("#mweb-send-drain")!,
  pegoutAmount: document.querySelector<HTMLInputElement>("#pegout-amount")!,
  pegoutDrain: document.querySelector<HTMLInputElement>("#pegout-drain")!,
  btnCreate: document.querySelector<HTMLButtonElement>("#btn-create")!,
  btnRestore: document.querySelector<HTMLButtonElement>("#btn-restore")!,
  btnMnemonicToVerify: document.querySelector<HTMLButtonElement>("#btn-mnemonic-to-verify")!,
  btnMnemonicShowAgain: document.querySelector<HTMLButtonElement>("#btn-mnemonic-show-again")!,
  btnMnemonicDone: document.querySelector<HTMLButtonElement>("#btn-mnemonic-done")!,
  btnSync: document.querySelector<HTMLButtonElement>("#btn-sync")!,
  btnAddress: document.querySelector<HTMLButtonElement>("#btn-address")!,
  btnCopy: document.querySelector<HTMLButtonElement>("#btn-copy")!,
  btnCopyPayment: document.querySelector<HTMLButtonElement>("#btn-copy-payment")!,
  btnCopyMweb: document.querySelector<HTMLButtonElement>("#btn-copy-mweb")!,
  btnResyncMweb: document.querySelector<HTMLButtonElement>("#btn-resync-mweb")!,
  btnApplyMwebScheme: document.querySelector<HTMLButtonElement>("#btn-apply-mweb-scheme")!,
  btnSend: document.querySelector<HTMLButtonElement>("#btn-send")!,
  btnWipe: document.querySelector<HTMLButtonElement>("#btn-wipe")!,
  btnWipeUnlock: document.querySelector<HTMLButtonElement>("#btn-wipe-unlock")!,
  btnUnlock: document.querySelector<HTMLButtonElement>("#btn-unlock")!,
  btnMigrate: document.querySelector<HTMLButtonElement>("#btn-migrate")!,
  btnSaveSettings: document.querySelector<HTMLButtonElement>("#btn-save-settings")!,
  btnLock: document.querySelector<HTMLButtonElement>("#btn-lock")!,
  btnPegin: document.querySelector<HTMLButtonElement>("#btn-pegin")!,
  btnMwebSend: document.querySelector<HTMLButtonElement>("#btn-mweb-send")!,
  btnPegout: document.querySelector<HTMLButtonElement>("#btn-pegout")!,
};

const views = Object.fromEntries(
  VIEWS.map((view) => [
    view,
    {
      nav: document.querySelector<HTMLButtonElement>(`#nav-${view}`)!,
      pane: document.querySelector<HTMLElement>(`#view-${view}`)!,
    },
  ]),
) as Record<View, { nav: HTMLButtonElement; pane: HTMLElement }>;

const cards = Object.fromEntries(
  CARDS.map((card) => [
    card,
    {
      nav: document.querySelector<HTMLButtonElement>(`#nav-${card}`)!,
      pane: document.querySelector<HTMLElement>(`#card-${card}`)!,
    },
  ]),
) as Record<Card, { nav: HTMLButtonElement; pane: HTMLElement }>;

let syncing = false;
let sending = false;
let currentPhase: Phase = "boot";
let currentView: View = "balance";
let activeCard: Card | null = null;
let syncState: SyncState = "idle";
let lastTxid: string | null = null;
let txRecords: TxRecord[] = [];
type HistoryFilter = "all" | "public" | "private" | "pending";
let historyFilter: HistoryFilter = "all";
let historySearchQuery = "";
let contactsCache: ContactRecord[] = [];
let utxoCache: UtxoRecord[] = [];
const sendSelectedOutpoints = new Set<string>();
const peginSelectedOutpoints = new Set<string>();
/** Local notes keyed by txid/wtxid (never sent to litview). */
let txLabels: Record<string, string> = {};
let autoSyncTimer: number | null = null;
let mwebProgressTimer: number | null = null;
let statusTimer: number | null = null;
let showFiat = true;
let useExplorerFeeHints = true;
let spotPriceUsd: number | null = null;
let lastTotalSats = 0;
let lastPendingSats = 0;
let lastCombined: CombinedSummary | null = null;
let lastSummary: WalletSummary | null = null;
let displayUnit: DisplayUnit = "ltc";
let hideBalances = false;
let selectedFeeRateSatVb: number | null = null;
let customFeeActive = false;
/** True once we've observed a non-zero balance this session (legacy first-receive skip). */
let sawNonZeroBalance = false;
/** In-session mnemonic for create → verify; cleared after quiz success. */
let pendingMnemonic: string | null = null;
/** Mnemonic indexes shown as numbered slots (phrase order). */
let quizPositions: number[] = [];
/** Chosen bank index per quiz slot (`quizPositions` parallel), or null if empty. */
let quizAnswers: Array<number | null> = [];
/** Which slot receives the next bank tap. */
let quizActiveSlot = 0;
const txEnrichmentCache = new Map<string, TxEnrichment>();

function isChainTxid(id: string): boolean {
  return /^[0-9a-fA-F]{64}$/.test(id.trim());
}

/** litview indexes transparent chain txs; peg-ins have a public txid. Pure MWEB ids do not. */
function txKindExplorable(kind: TxKind): boolean {
  return kind === "transparent" || kind === "pegin";
}

async function openExplorerForTxid(txid: string) {
  if (!isChainTxid(txid)) {
    setStatus(
      "Private transfers aren't on public explorers — keep the Kernel ID as your reference.",
      "info",
    );
    return;
  }
  try {
    const url = await invoke<string>("explorer_tx_url", { txid });
    await invoke("open_explorer_url", { url });
  } catch (e) {
    setError(String(e));
  }
}

function formatUsd(amount: number): string {
  return amount.toLocaleString("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

function renderFiat() {
  if (hideBalances || !showFiat || spotPriceUsd == null || !Number.isFinite(spotPriceUsd)) {
    el.balanceFiat.hidden = true;
    el.balanceFiat.textContent = "";
    return;
  }
  const usd = (lastTotalSats / 100_000_000) * spotPriceUsd;
  el.balanceFiat.hidden = false;
  el.balanceFiat.textContent = `≈ ${formatUsd(usd)}`;
}

async function refreshSpotPrice() {
  if (!showFiat || currentPhase !== "ready") return;
  try {
    spotPriceUsd = await invoke<number>("fetch_spot_price");
    renderFiat();
  } catch {
    /* soft-fail: keep last price or hide */
  }
}

function buildIoSection(title: string, items: TxIo[]): HTMLElement {
  const section = document.createElement("div");
  section.className = "tx-io";
  const heading = document.createElement("h4");
  heading.textContent = title;
  const list = document.createElement("ul");
  list.className = "tx-io-list";
  if (items.length === 0) {
    const li = document.createElement("li");
    li.textContent = "None";
    list.appendChild(li);
  } else {
    for (const io of items) {
      const li = document.createElement("li");
      if (io.is_wallet) li.classList.add("wallet");
      const addr = document.createElement("span");
      addr.className = "addr";
      addr.textContent = io.address
        ? io.is_wallet
          ? `${io.address} (yours)`
          : io.address
        : "(no address)";
      const amt = document.createElement("span");
      amt.className = "amt";
      amt.textContent = hideBalances ? HIDDEN_AMOUNT : formatAmountPlain(io.value_sats);
      li.append(addr, amt);
      list.appendChild(li);
    }
  }
  section.append(heading, list);
  return section;
}

function renderFeeChips(ladder: FeeLadder | null) {
  el.feeChipRow.textContent = "";
  if (!useExplorerFeeHints) {
    el.feeChips.hidden = true;
    el.feeCustomField.hidden = true;
    return;
  }
  el.feeChips.hidden = false;
  el.feeCustomField.hidden = false;
  if (!ladder) return;
  const chips: [string, number][] = [
    ["Fast", ladder.fastest_sat_vb],
    ["~30 min", ladder.half_hour_sat_vb],
    ["~1 hour", ladder.hour_sat_vb],
  ];
  if (ladder.economy_sat_vb != null) {
    chips.push(["Economy", ladder.economy_sat_vb]);
  }
  for (const [label, rate] of chips) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "fee-chip";
    btn.textContent = `${label} · ${rate} sat/vB`;
    const pressed = !customFeeActive && selectedFeeRateSatVb === rate;
    btn.setAttribute("aria-pressed", pressed ? "true" : "false");
    btn.addEventListener("click", () => {
      customFeeActive = false;
      el.feeCustom.value = "";
      selectedFeeRateSatVb = selectedFeeRateSatVb === rate ? null : rate;
      renderFeeChips(ladder);
      el.sendFeeHint.textContent =
        selectedFeeRateSatVb != null
          ? `Using ${selectedFeeRateSatVb} sat/vB from explorer suggestions.`
          : "Network fee is calculated automatically.";
    });
    el.feeChipRow.appendChild(btn);
  }
}

async function refreshFeeEstimate() {
  if (useExplorerFeeHints || currentPhase !== "ready") return;
  el.feeChips.hidden = false;
  el.feeChipRow.textContent = "";
  el.feeCustomField.hidden = false;
  try {
    const estimate = await invoke<FeeEstimate>("estimate_fee");
    const suffix = estimate.is_fallback ? " (floor fallback)" : "";
    el.sendFeeHint.textContent =
      selectedFeeRateSatVb != null && customFeeActive
        ? `Using custom ${selectedFeeRateSatVb} sat/vB.`
        : `Suggested ~${estimate.fee_rate_sat_vb} sat/vB from Electrum${suffix}.`;
  } catch {
    el.sendFeeHint.textContent =
      selectedFeeRateSatVb != null && customFeeActive
        ? `Using custom ${selectedFeeRateSatVb} sat/vB.`
        : "Network fee is calculated automatically.";
  }
}

async function refreshFeeLadder() {
  if (currentPhase !== "ready") {
    renderFeeChips(null);
    return;
  }
  if (!useExplorerFeeHints) {
    renderFeeChips(null);
    await refreshFeeEstimate();
    return;
  }
  try {
    const ladder = await invoke<FeeLadder>("fetch_fee_ladder");
    renderFeeChips(ladder);
  } catch {
    renderFeeChips(null);
  }
}

function formatLtc(sats: number): string {
  const whole = Math.trunc(sats / 100_000_000);
  const frac = Math.abs(sats % 100_000_000)
    .toString()
    .padStart(8, "0");
  const sign = sats < 0 ? "-" : "";
  return `${sign}${whole}.${frac} LTC`;
}

function formatLitoshisPlain(sats: number): string {
  return `${sats.toLocaleString("en-US")} litoshis`;
}

function formatLitoshis(sats: number): string {
  return `(${formatLitoshisPlain(sats)})`;
}

/** Primary amount display respecting unit preference (and hide-balances). */
function formatAmount(sats: number): string {
  if (hideBalances) return HIDDEN_AMOUNT;
  return formatAmountPlain(sats);
}

/** Amount display that never hides — for send/swap confirm and result dialogs. */
function formatAmountPlain(sats: number): string {
  return displayUnit === "litoshis" ? formatLitoshisPlain(sats) : formatLtc(sats);
}

function formatAmountSubtitle(sats: number): string {
  if (hideBalances) return HIDDEN_AMOUNT;
  return displayUnit === "litoshis" ? `(${formatLtc(sats)})` : formatLitoshis(sats);
}

function formatMs(ms: number): string {
  return ms >= 1_000 ? `${(ms / 1_000).toFixed(1)}s` : `${ms}ms`;
}

function unitLabel(): string {
  return displayUnit === "litoshis" ? "litoshis" : "LTC";
}

/** Parse LTC decimal string to litoshis. Rejects commas, negatives, >8 decimals. */
function parseLtcToSats(input: string): number | null {
  // Strip all whitespace (incl. non-breaking/narrow spaces from pasted text).
  const raw = input.replace(/[\s\u00a0\u202f]+/g, "");
  if (!raw || raw === "." || raw.includes(",") || raw.startsWith("-")) return null;
  // Allow ".009" and "5." in addition to "0.009".
  if (!/^(\d+(\.\d*)?|\.\d+)$/.test(raw)) return null;
  const [wholePart = "", fracPart = ""] = raw.split(".");
  if (fracPart.length > 8) return null;
  const whole = wholePart ? Number(wholePart) : 0;
  if (!Number.isSafeInteger(whole)) return null;
  const frac = fracPart.padEnd(8, "0");
  return whole * 100_000_000 + Number(frac);
}

/** Parse integer litoshis (no decimals). */
function parseLitoshisToSats(input: string): number | null {
  const raw = input.replace(/[\s\u00a0\u202f,]+/g, "");
  if (!raw || raw.startsWith("-") || !/^\d+$/.test(raw)) return null;
  const n = Number(raw);
  if (!Number.isSafeInteger(n)) return null;
  return n;
}

function parseAmountToSats(input: string): number | null {
  return displayUnit === "litoshis" ? parseLitoshisToSats(input) : parseLtcToSats(input);
}

/** Format sats into the active unit for amount input fields. */
function formatAmountInput(sats: number): string {
  if (displayUnit === "litoshis") return String(sats);
  const whole = Math.trunc(sats / 100_000_000);
  const frac = Math.abs(sats % 100_000_000)
    .toString()
    .padStart(8, "0")
    .replace(/0+$/, "");
  return frac ? `${whole}.${frac}` : String(whole);
}

function amountError(field: string, rawValue: string): string {
  const shown = rawValue.trim();
  const unit = unitLabel();
  if (!shown) return `Enter a ${field} amount in ${unit}.`;
  if (displayUnit === "litoshis") {
    return `Invalid ${field} amount "${shown}" — enter whole litoshis (e.g. ${DUST_LITOSHIS}).`;
  }
  if (shown.includes(",")) {
    return `Invalid ${field} amount "${shown}" — use a dot as the decimal separator (e.g. 0.009), no commas.`;
  }
  return `Invalid ${field} amount "${shown}" — enter LTC like 0.009 (max 8 decimal places).`;
}

function readDisplayUnit(): DisplayUnit {
  try {
    return localStorage.getItem(DISPLAY_UNIT_KEY) === "litoshis" ? "litoshis" : "ltc";
  } catch {
    return "ltc";
  }
}

function persistDisplayUnit(unit: DisplayUnit) {
  try {
    localStorage.setItem(DISPLAY_UNIT_KEY, unit);
  } catch {
    /* localStorage unavailable */
  }
}

function readHideBalances(): boolean {
  try {
    return localStorage.getItem(HIDE_BALANCES_KEY) === "1";
  } catch {
    return false;
  }
}

function persistHideBalances(hidden: boolean) {
  try {
    if (hidden) localStorage.setItem(HIDE_BALANCES_KEY, "1");
    else localStorage.removeItem(HIDE_BALANCES_KEY);
  } catch {
    /* localStorage unavailable */
  }
}

function syncHideBalancesUi() {
  el.settingsHideBalances.checked = hideBalances;
  el.balanceTotal.classList.toggle("is-hidden-balance", hideBalances);
  el.balanceTotal.title = hideBalances
    ? "Balances hidden — tap to show LTC"
    : "Tap to cycle LTC / litoshis / hide balances";
}

function setHideBalances(hidden: boolean) {
  hideBalances = hidden;
  persistHideBalances(hidden);
  syncHideBalancesUi();
  if (lastCombined) renderCombined(lastCombined);
  else if (lastSummary) renderSummary(lastSummary);
  if (txRecords.length) renderHistory(txRecords);
}

function refreshBalanceDisplays() {
  if (lastCombined) renderCombined(lastCombined);
  else if (lastSummary) renderSummary(lastSummary);
  if (txRecords.length) renderHistory(txRecords);
}

function amountInputs(): HTMLInputElement[] {
  return [el.sendAmount, el.mwebSendAmount, el.peginAmount, el.pegoutAmount, el.receiveAmount];
}

function syncAmountFieldLabels() {
  const label = `Amount (${unitLabel()})`;
  el.sendAmountLabel.textContent = label;
  el.mwebSendAmountLabel.textContent = label;
  el.peginAmountLabel.textContent = label;
  el.pegoutAmountLabel.textContent = label;
  el.receiveAmountLabel.textContent = `Request amount (optional, ${unitLabel()})`;
  const placeholder = displayUnit === "litoshis" ? String(DUST_LITOSHIS) : "0.001";
  for (const input of [el.sendAmount, el.mwebSendAmount, el.peginAmount, el.pegoutAmount]) {
    input.placeholder = placeholder;
    input.inputMode = displayUnit === "litoshis" ? "numeric" : "decimal";
  }
  el.receiveAmount.placeholder = displayUnit === "litoshis" ? "1000000" : "0.01";
  el.receiveAmount.inputMode = displayUnit === "litoshis" ? "numeric" : "decimal";
  el.settingsUnitLtc.checked = displayUnit === "ltc";
  el.settingsUnitLitoshis.checked = displayUnit === "litoshis";
  syncHideBalancesUi();
}

function setDisplayUnit(unit: DisplayUnit, opts: { clearAmbiguous?: boolean; keepHidden?: boolean } = {}) {
  const prev = displayUnit;
  displayUnit = unit;
  persistDisplayUnit(unit);
  if (!opts.keepHidden && hideBalances) {
    hideBalances = false;
    persistHideBalances(false);
  }
  syncAmountFieldLabels();
  if (opts.clearAmbiguous !== false && prev !== unit) {
    for (const input of amountInputs()) {
      if (!input.value.trim()) continue;
      // Value typed for the other unit is ambiguous — clear rather than mis-parse.
      input.value = "";
    }
  }
  refreshBalanceDisplays();
  void refreshPublicReceiveQr();
}

function setPhase(next: Phase) {
  currentPhase = next;
  el.phase.textContent = PHASE_LABELS[next];
  el.authShell.hidden = next === "ready";
  el.fatal.hidden = next !== "fatal";
  el.unlock.hidden = next !== "unlock";
  el.migrate.hidden = next !== "migrate";
  el.onboarding.hidden = next !== "onboarding";
  el.mnemonic.hidden = next !== "mnemonic";
  el.ready.hidden = next !== "ready";
  if (next === "ready") startAutoSync();
  else stopAutoSync();
  if (next !== "mnemonic") showMnemonicStep("show");
  updateBackupBanner();
  updateMaturityBanner(0);
  updateEmptyFundingState();
  updateSecurityChecklist();
}

function updateTitle() {
  el.viewTitle.textContent =
    currentView === "balance" && activeCard ? CARD_TITLES[activeCard] : VIEW_TITLES[currentView];
}

/** Reflect activeCard on the sheet and the sidebar in one place. */
function applyCardState() {
  views.balance.pane.dataset.sheet = activeCard ? "expanded" : "collapsed";
  el.cardTx.hidden = activeCard != null;
  for (const card of CARDS) {
    const { nav, pane } = cards[card];
    const active = activeCard === card;
    pane.hidden = !active;
    nav.setAttribute("aria-selected", String(active));
  }
  views.balance.nav.setAttribute(
    "aria-selected",
    String(currentView === "balance" && activeCard == null),
  );
  updateTitle();
}

function setView(next: View) {
  currentView = next;
  // Switching views folds the sheet — Balance in the sidebar always means the
  // overview, and the sidebar never shows two selections.
  activeCard = null;
  for (const view of VIEWS) {
    const { nav, pane } = views[view];
    pane.hidden = view !== next;
    nav.setAttribute("aria-selected", String(view === next));
  }
  el.views.classList.toggle("views-balance", next === "balance");
  applyCardState();
}

function setCard(next: Card | null) {
  if (next && cards[next].nav.hidden) return;
  activeCard = next;
  if (next && currentView !== "balance") {
    setView("balance");
    // setView clears the card, so re-apply the requested one.
    activeCard = next;
  }
  applyCardState();
  el.sheetBody.scrollTop = 0;
  if (next === "send") void refreshFeeLadder();
}

for (const view of VIEWS) {
  views[view].nav.addEventListener("click", () => setView(view));
}

for (const card of CARDS) {
  cards[card].nav.addEventListener("click", () => setCard(card));
}

el.btnSeeAll.addEventListener("click", () => setView("history"));

el.historySearch.addEventListener("input", () => {
  historySearchQuery = el.historySearch.value;
  renderHistoryFiltered();
});

for (const chip of el.historyFilterChips) {
  chip.addEventListener("click", () => {
    const next = chip.dataset.filter as HistoryFilter | undefined;
    if (!next) return;
    historyFilter = next;
    for (const other of el.historyFilterChips) {
      other.setAttribute("aria-pressed", String(other.dataset.filter === historyFilter));
    }
    renderHistoryFiltered();
  });
}

el.btnExportHistory.addEventListener("click", () => void exportHistoryFlow());
el.btnContactAdd.addEventListener("click", () => void contactEditorFlow(null));
el.btnPickContactPublic.addEventListener("click", () => void pickContactFlow("public"));
el.btnPickContactPrivate.addEventListener("click", () => void pickContactFlow("private"));

async function exportHistoryFlow() {
  const result = await openModal({
    title: "Export history",
    build: (body) => {
      const p = document.createElement("p");
      p.className = "lede";
      p.textContent =
        "Saves a local file of your transaction history, including amounts and notes. " +
        "The file never includes your recovery phrase or wallet passphrase.";
      const hint = document.createElement("p");
      hint.className = "hint";
      hint.textContent =
        "Export always includes full amounts even when balances are hidden on screen.";
      body.append(p, hint);
    },
    actions: [
      { id: "cancel", label: "Cancel", kind: "ghost" },
      { id: "json", label: "Export JSON", kind: "secondary" },
      { id: "csv", label: "Export CSV", kind: "primary" },
    ],
  });
  if (result !== "csv" && result !== "json") return;
  try {
    showLoading("Exporting…");
    const path = await invoke<string | null>("export_history", { format: result });
    if (path) setStatus(`Exported to ${path}`, "success");
  } catch (e) {
    setStatus(String(e), "error");
  } finally {
    hideLoading();
  }
}

/* ---------------------------------------------------------------------------
   Public/Private segmented toggles inside the Send, Receive and Swap cards
   --------------------------------------------------------------------------- */

type SegMode = "public" | "private";
type SwapDirection = "in" | "out";

let sendMode: SegMode = "public";
let receiveMode: SegMode = "public";
let swapDirection: SwapDirection = "in";

function applySeg(
  firstSeg: HTMLButtonElement,
  firstPanel: HTMLElement,
  secondSeg: HTMLButtonElement,
  secondPanel: HTMLElement,
  firstActive: boolean,
) {
  firstSeg.setAttribute("aria-selected", String(firstActive));
  secondSeg.setAttribute("aria-selected", String(!firstActive));
  firstPanel.hidden = !firstActive;
  secondPanel.hidden = firstActive;
}

function applySegModes() {
  applySeg(el.sendSegPublic, el.sendPublic, el.sendSegPrivate, el.sendPrivate, sendMode === "public");
  applySeg(
    el.receiveSegPublic,
    el.receivePublic,
    el.receiveSegPrivate,
    el.receivePrivate,
    receiveMode === "public",
  );
  applySeg(el.swapSegIn, el.swapIn, el.swapSegOut, el.swapOut, swapDirection === "in");
}

el.sendSegPublic.addEventListener("click", () => {
  sendMode = "public";
  applySegModes();
});
el.sendSegPrivate.addEventListener("click", () => {
  sendMode = "private";
  applySegModes();
});
el.receiveSegPublic.addEventListener("click", () => {
  receiveMode = "public";
  applySegModes();
});
el.receiveSegPrivate.addEventListener("click", () => {
  receiveMode = "private";
  applySegModes();
});
el.swapSegIn.addEventListener("click", () => {
  swapDirection = "in";
  applySegModes();
});
el.swapSegOut.addEventListener("click", () => {
  swapDirection = "out";
  applySegModes();
});

function setStatus(message: string | null, kind: StatusKind = "info") {
  if (statusTimer != null) {
    clearTimeout(statusTimer);
    statusTimer = null;
  }
  if (!message) {
    el.toast.hidden = true;
    el.status.textContent = "";
    return;
  }
  el.status.textContent = message;
  el.status.title = message;
  el.toast.dataset.kind = kind;
  el.toast.hidden = false;
  statusTimer = window.setTimeout(
    () => {
      el.toast.hidden = true;
      statusTimer = null;
    },
    kind === "error" ? 9_000 : 4_000,
  );
}

el.btnToastClose.addEventListener("click", () => setStatus(null));

function setError(message: string | null) {
  if (!message) {
    el.error.hidden = true;
    el.error.textContent = "";
    return;
  }
  // Inside the app shell there is no room for a persistent banner — use the toast.
  if (currentPhase === "ready") {
    el.error.hidden = true;
    el.error.textContent = "";
    setStatus(message, "error");
    return;
  }
  el.error.hidden = false;
  el.error.textContent = message;
}

/* ---------------------------------------------------------------------------
   Glass modal shell
   --------------------------------------------------------------------------- */

type ModalActionKind = "primary" | "secondary" | "ghost" | "danger";

type ModalAction = {
  id: string;
  label: string;
  kind?: ModalActionKind;
  /** Nav actions sit on the left of the action row (prev/next). */
  nav?: boolean;
};

type ModalOptions = {
  title: string;
  build: (body: HTMLElement) => void;
  actions: ModalAction[];
  wide?: boolean;
  dismissable?: boolean;
  focus?: () => HTMLElement | null;
  onKey?: (event: KeyboardEvent, close: (id: string) => void) => void;
};

const FOCUSABLE =
  'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])';

let modalResolve: ((id: string | null) => void) | null = null;
let modalRestoreFocus: HTMLElement | null = null;
let modalKeyHandler: ((event: KeyboardEvent) => void) | null = null;
let modalDismissable = true;

function modalFocusables(): HTMLElement[] {
  return Array.from(el.modalPanel.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (node) => !node.hidden && node.offsetParent !== null,
  );
}

function closeModal(result: string | null) {
  const resolve = modalResolve;
  if (!resolve) return;
  modalResolve = null;
  if (modalKeyHandler) {
    document.removeEventListener("keydown", modalKeyHandler, true);
    modalKeyHandler = null;
  }
  el.modalOverlay.hidden = true;
  el.modalBody.textContent = "";
  el.modalActions.textContent = "";
  el.modalPanel.classList.remove("modal-panel-wide");
  modalRestoreFocus?.focus();
  modalRestoreFocus = null;
  resolve(result);
}

function openModal(opts: ModalOptions): Promise<string | null> {
  // One dialog at a time: an already-open one resolves as dismissed.
  closeModal(null);
  modalDismissable = opts.dismissable !== false;
  modalRestoreFocus = document.activeElement as HTMLElement | null;
  el.modalTitle.textContent = opts.title;
  el.modalClose.hidden = !modalDismissable;
  el.modalPanel.classList.toggle("modal-panel-wide", opts.wide === true);
  opts.build(el.modalBody);

  let navGroup: HTMLElement | null = null;
  for (const action of opts.actions) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = `btn btn-${action.kind ?? "secondary"}`;
    btn.dataset.action = action.id;
    btn.textContent = action.label;
    btn.addEventListener("click", () => closeModal(action.id));
    if (action.nav) {
      if (!navGroup) {
        navGroup = document.createElement("div");
        navGroup.className = "modal-nav";
        el.modalActions.appendChild(navGroup);
      }
      navGroup.appendChild(btn);
    } else {
      el.modalActions.appendChild(btn);
    }
  }

  el.modalOverlay.hidden = false;
  (opts.focus?.() ?? modalFocusables()[0] ?? el.modalPanel).focus();

  modalKeyHandler = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      if (modalDismissable) {
        event.preventDefault();
        closeModal(null);
      }
      return;
    }
    if (event.key === "Tab") {
      const nodes = modalFocusables();
      if (nodes.length === 0) return;
      const first = nodes[0];
      const last = nodes[nodes.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (event.shiftKey && (active === first || !el.modalPanel.contains(active))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
      }
      return;
    }
    opts.onKey?.(event, closeModal);
  };
  document.addEventListener("keydown", modalKeyHandler, true);

  return new Promise((resolve) => {
    modalResolve = resolve;
  });
}

el.modalClose.addEventListener("click", () => {
  if (modalDismissable) closeModal(null);
});

el.modalOverlay.addEventListener("mousedown", (event) => {
  if (event.target === el.modalOverlay && modalDismissable) closeModal(null);
});

type DetailRow = [label: string, value: string, mono?: boolean];

function buildDetailList(rows: DetailRow[]): HTMLElement {
  const list = document.createElement("div");
  list.className = "detail-list";
  for (const [label, value, mono] of rows) {
    const row = document.createElement("div");
    row.className = "detail-row";
    const labelEl = document.createElement("span");
    labelEl.className = "detail-label";
    labelEl.textContent = label;
    const valueEl = document.createElement("span");
    valueEl.className = mono ? "detail-value mono" : "detail-value";
    valueEl.textContent = value;
    row.append(labelEl, valueEl);
    list.appendChild(row);
  }
  return list;
}

function appendParagraph(host: HTMLElement, text: string, className: string) {
  const p = document.createElement("p");
  p.className = className;
  p.textContent = text;
  host.appendChild(p);
}

function addressNetworkBadge(address: string): string {
  const a = address.trim().toLowerCase();
  if (a.startsWith("ltcmweb1") || a.startsWith("tmweb1")) return "Private · ltcmweb1";
  if (a.startsWith("ltc1") || a.startsWith("tltc1")) return "Public · ltc1";
  return "Address";
}

function isHighFee(feeSats: number, amountSats: number): boolean {
  return amountSats > 0 && feeSats >= amountSats * HIGH_FEE_RATIO;
}

function appendConfirmDestination(host: HTMLElement, address: string, badge: string) {
  const box = document.createElement("div");
  box.className = "confirm-destination";
  const head = document.createElement("div");
  head.className = "confirm-destination-head";
  const badgeEl = document.createElement("span");
  badgeEl.className = "addr-badge";
  badgeEl.textContent = badge;
  const copyBtn = document.createElement("button");
  copyBtn.type = "button";
  copyBtn.className = "btn btn-ghost btn-sm";
  copyBtn.textContent = "Copy";
  copyBtn.addEventListener("click", () => {
    void copyText(address, "Address copied.");
  });
  head.append(badgeEl, copyBtn);
  const addr = document.createElement("div");
  addr.className = "confirm-destination-address";
  addr.textContent = address;
  box.append(head, addr);
  host.appendChild(box);
}

/** Append optional note field; returns a getter for the trimmed label. */
function appendTxLabelField(body: HTMLElement, initial = ""): () => string {
  const field = document.createElement("label");
  field.className = "field tx-label-field";
  const caption = document.createElement("span");
  caption.className = "field-label";
  caption.textContent = "Note (optional)";
  const input = document.createElement("input");
  input.type = "text";
  input.maxLength = MAX_TX_LABEL_CHARS;
  input.placeholder = "e.g. rent, exchange withdrawal";
  input.value = initial;
  input.spellcheck = true;
  field.append(caption, input);
  body.appendChild(field);
  return () => input.value.trim().slice(0, MAX_TX_LABEL_CHARS);
}

async function persistTxLabel(txid: string, label: string) {
  const note = label.trim().slice(0, MAX_TX_LABEL_CHARS);
  try {
    await invoke("set_tx_label", { req: { txid, label: note } });
    if (note) txLabels[txid] = note;
    else delete txLabels[txid];
  } catch {
    /* soft-fail: broadcast already succeeded */
  }
}

async function refreshTxLabels() {
  try {
    txLabels = (await invoke<Record<string, string>>("get_tx_labels")) ?? {};
  } catch {
    txLabels = {};
  }
}

async function openConfirm(opts: {
  title: string;
  message: string;
  rows?: DetailRow[];
  detail?: string;
  warning?: string | string[];
  destination?: string;
  confirmLabel?: string;
  danger?: boolean;
  afterDetail?: (body: HTMLElement) => void;
}): Promise<boolean> {
  const warnings = opts.warning == null ? [] : Array.isArray(opts.warning) ? opts.warning : [opts.warning];
  const result = await openModal({
    title: opts.title,
    build: (body) => {
      appendParagraph(body, opts.message, "lede");
      if (opts.destination) {
        appendConfirmDestination(
          body,
          opts.destination,
          addressNetworkBadge(opts.destination),
        );
      }
      for (const warning of warnings) {
        if (warning) appendParagraph(body, warning, "confirm-warning");
      }
      if (opts.rows?.length) body.appendChild(buildDetailList(opts.rows));
      if (opts.detail) appendParagraph(body, opts.detail, "hint");
      opts.afterDetail?.(body);
    },
    actions: [
      { id: "cancel", label: "Cancel", kind: "ghost" },
      {
        id: "confirm",
        label: opts.confirmLabel ?? "Confirm",
        kind: opts.danger ? "danger" : "primary",
      },
    ],
    focus: () => el.modalActions.querySelector<HTMLElement>('[data-action="confirm"]'),
  });
  return result === "confirm";
}

async function showResult(opts: {
  title: string;
  message: string;
  rows: DetailRow[];
  copy?: { value: string; label: string; toast: string };
  explorerTxid?: string;
  extraActions?: ModalAction[];
}): Promise<string | null> {
  for (;;) {
    const actions: ModalAction[] = [];
    if (opts.explorerTxid && isChainTxid(opts.explorerTxid)) {
      actions.push({ id: "explore", label: "View on litview", kind: "secondary" });
    }
    if (opts.extraActions?.length) actions.push(...opts.extraActions);
    if (opts.copy) actions.push({ id: "copy", label: opts.copy.label, kind: "secondary" });
    actions.push({ id: "done", label: "Done", kind: "primary" });
    const result = await openModal({
      title: opts.title,
      wide: true,
      build: (body) => {
        appendParagraph(body, opts.message, "lede");
        body.appendChild(buildDetailList(opts.rows));
      },
      actions,
      focus: () => el.modalActions.querySelector<HTMLElement>('[data-action="done"]'),
    });
    if (result === "explore" && opts.explorerTxid) {
      await openExplorerForTxid(opts.explorerTxid);
      continue;
    }
    if (result === "copy" && opts.copy) {
      await copyText(opts.copy.value, opts.copy.toast);
      continue;
    }
    return result;
  }
}

/**
 * Re-authenticate before a destructive action. `unlock_wallet` re-decrypts the
 * stored blob, so a wrong passphrase fails before any lock is taken and the
 * unlocked session is left untouched.
 */
async function requirePassphrase(reason: string): Promise<boolean> {
  try {
    // A wallet still stored in plaintext has nothing to verify against.
    if (await invoke<boolean>("wallet_needs_migration")) return true;
  } catch {
    /* fall through and ask anyway */
  }

  let errorText: string | null = null;
  for (;;) {
    let value = "";
    let input: HTMLInputElement | null = null;
    const result = await openModal({
      title: "Confirm your passphrase",
      build: (body) => {
        appendParagraph(body, reason, "lede");
        const label = document.createElement("label");
        label.className = "field";
        const caption = document.createElement("span");
        caption.className = "field-label";
        caption.textContent = "Passphrase";
        input = document.createElement("input");
        input.type = "password";
        input.autocomplete = "current-password";
        input.addEventListener("input", () => {
          value = input!.value;
        });
        input.addEventListener("keydown", (event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            closeModal("submit");
          }
        });
        label.append(caption, input);
        body.appendChild(label);
        if (errorText) appendParagraph(body, errorText, "modal-error");
      },
      actions: [
        { id: "cancel", label: "Cancel", kind: "ghost" },
        { id: "submit", label: "Continue", kind: "primary" },
      ],
      focus: () => input,
    });

    if (result !== "submit") return false;
    if (!value) {
      errorText = "Enter your passphrase.";
      continue;
    }
    showLoading("Verifying passphrase…");
    try {
      await invoke("unlock_wallet", { req: { passphrase: value } });
      return true;
    } catch {
      errorText = "That passphrase is not correct.";
    } finally {
      hideLoading();
    }
  }
}

/* ---------------------------------------------------------------------------
   Loading overlay
   --------------------------------------------------------------------------- */

let loadingDepth = 0;

function showLoading(label: string) {
  loadingDepth += 1;
  el.loadingLabel.textContent = label;
  el.loadingOverlay.hidden = false;
}

function setLoadingLabel(label: string) {
  if (loadingDepth > 0) el.loadingLabel.textContent = label;
}

function hideLoading() {
  loadingDepth = Math.max(0, loadingDepth - 1);
  if (loadingDepth === 0) el.loadingOverlay.hidden = true;
}

async function copyText(text: string, okMessage: string) {
  try {
    await navigator.clipboard.writeText(text);
    setStatus(okMessage, "success");
  } catch {
    setStatus("Copy failed — select the text and copy manually.", "error");
  }
}

const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

function readThemePref(): ThemePref {
  const raw = document.documentElement.dataset.themePref;
  return raw === "light" || raw === "dark" ? raw : "auto";
}

function applyTheme(pref: ThemePref) {
  const dark = pref === "dark" || (pref === "auto" && darkQuery.matches);
  const root = document.documentElement;
  root.dataset.theme = dark ? "dark" : "light";
  root.dataset.themePref = pref;
  try {
    localStorage.setItem(THEME_KEY, pref);
  } catch {
    /* localStorage unavailable */
  }
  el.btnTheme.title = `Theme: ${pref}`;
  el.btnTheme.setAttribute("aria-label", `Theme: ${pref}. Click to change.`);
}

darkQuery.addEventListener("change", () => {
  if (readThemePref() === "auto") applyTheme("auto");
});

el.btnTheme.addEventListener("click", () => {
  const next = THEME_ORDER[(THEME_ORDER.indexOf(readThemePref()) + 1) % THEME_ORDER.length];
  applyTheme(next);
});

function flashLabel(btn: HTMLButtonElement, text: string) {
  const label = btn.querySelector<HTMLElement>(".btn-label");
  if (!label) return;
  const original = label.dataset.original ?? label.textContent ?? "";
  label.dataset.original = original;
  label.textContent = text;
  window.setTimeout(() => {
    label.textContent = label.dataset.original ?? original;
  }, 1_400);
}

function updateBusyUi() {
  const busy = syncing || sending;
  const drain = el.sendDrain.checked;
  el.btnSync.disabled = busy;
  el.btnAddress.disabled = busy;
  el.btnCopy.disabled = busy;
  el.btnCopyPayment.disabled = busy;
  el.btnSend.disabled = busy;
  // A filled restore field means the user intends to restore; block Create so
  // the primary button can't silently generate a fresh wallet instead.
  const restorePending = el.restoreMnemonic.value.trim().length > 0;
  const createPassOk =
    requireWalletPassphrase(el.onboardPassphrase.value, el.onboardPassphrase2.value) == null;
  const restorePassOk =
    requireWalletPassphrase(el.restorePassphrase.value, el.restorePassphrase2.value) == null;
  const migratePassOk =
    requireWalletPassphrase(el.migratePassphrase.value, el.migratePassphrase2.value) == null;
  el.btnCreate.disabled = busy || restorePending || !createPassOk;
  el.createRestoreHint.hidden = !restorePending;
  el.btnRestore.disabled = busy || !restorePassOk || !el.restoreMnemonic.value.trim();
  el.btnMigrate.disabled = busy || !migratePassOk;
  el.sendAddress.disabled = busy;
  el.sendAmount.disabled = busy || drain;
  el.peginAmount.disabled = busy || el.peginDrain.checked;
  el.mwebSendAmount.disabled = busy || el.mwebSendDrain.checked;
  el.pegoutAmount.disabled = busy || el.pegoutDrain.checked;
  el.btnPegin.disabled = busy;
  el.btnMwebSend.disabled = busy;
  el.btnPegout.disabled = busy;
  el.btnResyncMweb.disabled = busy;

  el.syncLabel.textContent = sending ? "Sending…" : syncing ? "Syncing…" : "Sync";
  el.syncDot.dataset.state = busy ? "busy" : syncState;
  el.syncDot.title = busy
    ? sending
      ? "Sending"
      : "Syncing"
    : SYNC_TITLES[syncState];
}

function setMwebVisible(visible: boolean) {
  el.statMweb.hidden = !visible;
  el.mwebStatusCard.hidden = !visible;
  el.mwebTools.hidden = !visible;
  // Without MWEB there is only one side: hide the toggles and force public.
  el.sendToggle.hidden = !visible;
  el.receiveToggle.hidden = !visible;
  cards.swap.nav.hidden = !visible;
  if (!visible) {
    sendMode = "public";
    receiveMode = "public";
    applySegModes();
    if (activeCard === "swap") setCard(null);
  }
}

function isMwebAddress(address: string): boolean {
  return address.startsWith("ltcmweb") || address.startsWith("tmweb");
}

/** BIP21 amount is always LTC decimal (not litoshis). */
function satsToBip21Amount(sats: number): string {
  const whole = Math.trunc(sats / 100_000_000);
  const frac = Math.abs(sats % 100_000_000)
    .toString()
    .padStart(8, "0")
    .replace(/0+$/, "");
  return frac ? `${whole}.${frac}` : String(whole);
}

function buildPaymentUri(
  address: string,
  opts: { amountSats?: number | null; label?: string | null } = {},
): string {
  if (!address || isMwebAddress(address)) return address;
  let uri = `litecoin:${address}`;
  const params: string[] = [];
  if (opts.amountSats != null && opts.amountSats > 0) {
    params.push(`amount=${satsToBip21Amount(opts.amountSats)}`);
  }
  const label = opts.label?.trim();
  if (label) params.push(`label=${encodeURIComponent(label)}`);
  if (params.length) uri += `?${params.join("&")}`;
  return uri;
}

function parsePaymentUri(text: string): ParsedPaymentUri | null {
  const raw = text.trim();
  if (!raw) return null;
  const m = raw.match(/^litecoin:([^?/#]+)(?:\?(.*))?$/i);
  if (!m) return null;
  const address = decodeURIComponent(m[1] ?? "").trim();
  if (!address) return null;
  let amountSats: number | null = null;
  let label: string | null = null;
  const query = m[2] ?? "";
  if (query) {
    for (const part of query.split("&")) {
      if (!part) continue;
      const eq = part.indexOf("=");
      const key = (eq >= 0 ? part.slice(0, eq) : part).toLowerCase();
      const value = eq >= 0 ? part.slice(eq + 1) : "";
      if (key === "amount") {
        const parsed = parseLtcToSats(decodeURIComponent(value));
        if (parsed == null) return null;
        amountSats = parsed;
      } else if (key === "label") {
        try {
          label = decodeURIComponent(value.replace(/\+/g, " "));
        } catch {
          label = value;
        }
      }
    }
  }
  return { address, amountSats, label };
}

function publicReceiveRequest(): { amountSats: number | null; label: string | null } {
  const amountRaw = el.receiveAmount.value.trim();
  let amountSats: number | null = null;
  if (amountRaw) {
    amountSats = parseAmountToSats(amountRaw);
  }
  const label = el.receiveLabel.value.trim() || null;
  return { amountSats, label };
}

async function refreshPublicReceiveQr() {
  const address = el.address.textContent?.trim() ?? "";
  if (!address) {
    await renderQr(el.receiveQr, "");
    return;
  }
  const { amountSats, label } = publicReceiveRequest();
  // Invalid optional amount: still show address-only QR rather than a broken URI.
  const uri = buildPaymentUri(address, {
    amountSats: amountSats != null && amountSats > 0 ? amountSats : null,
    label,
  });
  await renderQr(el.receiveQr, uri);
}

async function renderQr(canvas: HTMLCanvasElement, payload: string) {
  const ctx = canvas.getContext("2d");
  if (!payload) {
    ctx?.clearRect(0, 0, canvas.width, canvas.height);
    return;
  }
  // Render at device resolution, then pin the CSS box so it stays crisp on Retina.
  const dpr = Math.min(3, Math.max(1, Math.round(window.devicePixelRatio || 1)));
  try {
    await QRCode.toCanvas(canvas, payload, {
      errorCorrectionLevel: "M",
      margin: 2,
      width: QR_CSS_SIZE * dpr,
      color: { dark: "#000000", light: "#ffffff" },
    });
    canvas.style.width = `${QR_CSS_SIZE}px`;
    canvas.style.height = `${QR_CSS_SIZE}px`;
  } catch (e) {
    ctx?.clearRect(0, 0, canvas.width, canvas.height);
    setError(`QR render failed: ${e}`);
  }
}

function splitMnemonicWords(mnemonic: string): string[] {
  return mnemonic.trim().split(/\s+/).filter(Boolean);
}

function renderMnemonic(mnemonic: string) {
  el.mnemonicText.textContent = "";
  const words = splitMnemonicWords(mnemonic);
  words.forEach((word, i) => {
    const chip = document.createElement("div");
    chip.className = "mnemonic-word";
    const index = document.createElement("span");
    index.className = "mnemonic-index";
    index.textContent = String(i + 1);
    const text = document.createElement("span");
    text.textContent = word;
    chip.append(index, text);
    el.mnemonicText.appendChild(chip);
  });
}

function showMnemonicStep(step: "show" | "verify") {
  el.mnemonicShow.hidden = step !== "show";
  el.mnemonicVerify.hidden = step !== "verify";
  el.mnemonicQuizError.hidden = true;
  el.mnemonicQuizError.textContent = "";
  if (currentPhase === "mnemonic") {
    el.phase.textContent =
      step === "verify" ? "Confirm your backup" : PHASE_LABELS.mnemonic;
  }
}

function shuffleInPlace<T>(items: T[]): T[] {
  for (let i = items.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [items[i], items[j]] = [items[j], items[i]];
  }
  return items;
}

function pickQuizPositions(wordCount: number): number[] {
  const count = Math.min(QUIZ_WORD_COUNT, wordCount);
  const pool = Array.from({ length: wordCount }, (_, i) => i);
  shuffleInPlace(pool);
  return pool.slice(0, count).sort((a, b) => a - b);
}

function quizIsComplete(): boolean {
  return (
    quizPositions.length > 0 &&
    quizAnswers.length === quizPositions.length &&
    quizAnswers.every((answer, i) => answer === quizPositions[i])
  );
}

function quizAllSlotsFilled(): boolean {
  return quizAnswers.length > 0 && quizAnswers.every((answer) => answer != null);
}

function firstEmptyQuizSlot(): number {
  const empty = quizAnswers.findIndex((answer) => answer == null);
  return empty >= 0 ? empty : 0;
}

function updateMnemonicQuizUi() {
  const slotsHost = el.mnemonicQuiz.querySelector<HTMLElement>(".mnemonic-quiz-slots");
  const bankHost = el.mnemonicQuiz.querySelector<HTMLElement>(".mnemonic-quiz-bank");
  if (!slotsHost || !bankHost || !pendingMnemonic) return;

  const words = splitMnemonicWords(pendingMnemonic);
  for (const slot of slotsHost.querySelectorAll<HTMLButtonElement>(".mnemonic-quiz-slot")) {
    const slotIndex = Number(slot.dataset.slotIndex);
    const answerPos = quizAnswers[slotIndex];
    const value = slot.querySelector<HTMLElement>(".mnemonic-quiz-slot-value");
    if (!value) continue;
    if (answerPos == null) {
      value.textContent = "Tap a word below";
      value.classList.add("is-empty");
    } else {
      value.textContent = words[answerPos] ?? "";
      value.classList.remove("is-empty");
    }
    slot.classList.toggle("is-active", slotIndex === quizActiveSlot);
  }

  const used = new Set(quizAnswers.filter((pos): pos is number => pos != null));
  for (const chip of bankHost.querySelectorAll<HTMLButtonElement>(".mnemonic-quiz-chip")) {
    const pos = Number(chip.dataset.quizPos);
    chip.disabled = used.has(pos);
  }

  el.mnemonicQuizError.hidden = true;
  el.mnemonicQuizError.textContent = "";
  if (quizAllSlotsFilled() && !quizIsComplete()) {
    el.mnemonicQuizError.hidden = false;
    el.mnemonicQuizError.textContent =
      "That does not match. Clear a slot and try again, or show the phrase again.";
    el.btnMnemonicDone.disabled = true;
  } else {
    el.btnMnemonicDone.disabled = !quizIsComplete();
  }
}

function placeQuizWord(wordPos: number) {
  if (quizAnswers.includes(wordPos)) return;
  const slotIndex =
    quizAnswers[quizActiveSlot] == null ? quizActiveSlot : firstEmptyQuizSlot();
  if (quizAnswers[slotIndex] != null) return;
  quizAnswers[slotIndex] = wordPos;
  quizActiveSlot = firstEmptyQuizSlot();
  updateMnemonicQuizUi();
}

function clearQuizSlot(slotIndex: number) {
  if (quizAnswers[slotIndex] == null) {
    quizActiveSlot = slotIndex;
    updateMnemonicQuizUi();
    return;
  }
  quizAnswers[slotIndex] = null;
  quizActiveSlot = slotIndex;
  updateMnemonicQuizUi();
}

function buildMnemonicQuiz(positions: number[]) {
  el.mnemonicQuiz.textContent = "";
  quizAnswers = positions.map(() => null);
  quizActiveSlot = 0;
  if (!pendingMnemonic) return;

  const words = splitMnemonicWords(pendingMnemonic);
  const slotsHost = document.createElement("div");
  slotsHost.className = "mnemonic-quiz-slots";
  slotsHost.setAttribute("aria-label", "Word slots to fill");

  positions.forEach((pos, slotIndex) => {
    const slot = document.createElement("button");
    slot.type = "button";
    slot.className = "mnemonic-quiz-slot";
    slot.dataset.slotIndex = String(slotIndex);
    const index = document.createElement("span");
    index.className = "mnemonic-quiz-slot-index";
    index.textContent = `Word ${pos + 1}`;
    const value = document.createElement("span");
    value.className = "mnemonic-quiz-slot-value is-empty";
    value.textContent = "Tap a word below";
    slot.append(index, value);
    slot.addEventListener("click", () => clearQuizSlot(slotIndex));
    slotsHost.appendChild(slot);
  });

  const bankHost = document.createElement("div");
  bankHost.className = "mnemonic-quiz-bank";
  bankHost.setAttribute("aria-label", "Words to select");

  for (const pos of shuffleInPlace([...positions])) {
    const chip = document.createElement("button");
    chip.type = "button";
    chip.className = "mnemonic-quiz-chip";
    chip.dataset.quizPos = String(pos);
    chip.textContent = words[pos] ?? "";
    chip.addEventListener("click", () => placeQuizWord(pos));
    bankHost.appendChild(chip);
  }

  el.mnemonicQuiz.append(slotsHost, bankHost);
  updateMnemonicQuizUi();
}

function clearPendingMnemonic() {
  pendingMnemonic = null;
  quizPositions = [];
  quizAnswers = [];
  quizActiveSlot = 0;
  el.mnemonicText.textContent = "";
  el.mnemonicQuiz.textContent = "";
  el.btnMnemonicDone.disabled = true;
  showMnemonicStep("show");
}

function setBackupVerified(verified: boolean) {
  try {
    if (verified) localStorage.setItem(BACKUP_VERIFIED_KEY, "1");
    else localStorage.removeItem(BACKUP_VERIFIED_KEY);
  } catch {
    /* localStorage unavailable */
  }
}

function isBackupVerified(): boolean {
  try {
    return localStorage.getItem(BACKUP_VERIFIED_KEY) === "1";
  } catch {
    return false;
  }
}

function isBackupBannerDismissed(): boolean {
  try {
    return localStorage.getItem(BACKUP_BANNER_DISMISSED_KEY) === "1";
  } catch {
    return false;
  }
}

function setBackupBannerDismissed(dismissed: boolean) {
  try {
    if (dismissed) localStorage.setItem(BACKUP_BANNER_DISMISSED_KEY, "1");
    else localStorage.removeItem(BACKUP_BANNER_DISMISSED_KEY);
  } catch {
    /* localStorage unavailable */
  }
}

function isMwebCoachSeen(): boolean {
  try {
    return localStorage.getItem(MWEB_COACH_SEEN_KEY) === "1";
  } catch {
    return false;
  }
}

function setMwebCoachSeen(seen: boolean) {
  try {
    if (seen) localStorage.setItem(MWEB_COACH_SEEN_KEY, "1");
    else localStorage.removeItem(MWEB_COACH_SEEN_KEY);
  } catch {
    /* localStorage unavailable */
  }
}

function isSecurityChecklistDismissed(): boolean {
  try {
    return localStorage.getItem(SECURITY_CHECKLIST_DISMISSED_KEY) === "1";
  } catch {
    return false;
  }
}

function setSecurityChecklistDismissed(dismissed: boolean) {
  try {
    if (dismissed) localStorage.setItem(SECURITY_CHECKLIST_DISMISSED_KEY, "1");
    else localStorage.removeItem(SECURITY_CHECKLIST_DISMISSED_KEY);
  } catch {
    /* localStorage unavailable */
  }
}

function isFirstReceiveSeen(): boolean {
  try {
    return localStorage.getItem(FIRST_RECEIVE_SEEN_KEY) === "1";
  } catch {
    return false;
  }
}

function setFirstReceiveSeen(seen: boolean) {
  try {
    if (seen) localStorage.setItem(FIRST_RECEIVE_SEEN_KEY, "1");
    else localStorage.removeItem(FIRST_RECEIVE_SEEN_KEY);
  } catch {
    /* localStorage unavailable */
  }
}

function clearBackupLocalFlags() {
  setBackupVerified(false);
  setBackupBannerDismissed(false);
  setMwebCoachSeen(false);
  setSecurityChecklistDismissed(false);
  setFirstReceiveSeen(false);
  sawNonZeroBalance = false;
  lastPendingSats = 0;
  lastCombined = null;
  lastSummary = null;
}

function pulseRecentHistoryRows() {
  const recent = el.txListRecent.querySelector<HTMLElement>(".tx-row");
  if (!recent) return;
  const txid = recent.dataset.txid;
  const rows = [
    recent,
    ...el.txList.querySelectorAll<HTMLElement>(".tx-row"),
  ].filter((row, index, all) => all.indexOf(row) === index);
  for (const row of rows) {
    if (txid && row.dataset.txid !== txid) continue;
    row.classList.remove("tx-row-pulse");
    // Restart animation if the class was already present.
    void row.offsetWidth;
    row.classList.add("tx-row-pulse");
    window.setTimeout(() => row.classList.remove("tx-row-pulse"), 1_500);
  }
}

async function showFirstReceiveModal() {
  await openModal({
    title: "Funds arrived",
    build: (body) => {
      appendParagraph(
        body,
        "Funds arrived on Public. Use Swap to move to Private after they confirm, if you want confidentiality.",
        "lede",
      );
      appendParagraph(
        body,
        "Most exchanges pay to a public ltc1 address first. Private (MWEB) is optional.",
        "hint",
      );
    },
    actions: [{ id: "done", label: "Got it", kind: "primary" }],
    focus: () => el.modalActions.querySelector<HTMLElement>('[data-action="done"]'),
  });
  setFirstReceiveSeen(true);
}

function transparentPendingSats(s: WalletSummary): number {
  return s.trusted_pending_sats + s.untrusted_pending_sats + s.immature_sats;
}

function updateBackupBanner() {
  const show =
    currentPhase === "ready" &&
    lastTotalSats > 0 &&
    !isBackupVerified() &&
    !isBackupBannerDismissed();
  el.backupBanner.hidden = !show;
}

function updateMaturityBanner(immatureSats: number) {
  const show = currentPhase === "ready" && immatureSats > 0;
  el.maturityBanner.hidden = !show;
  if (show) {
    el.maturityBannerText.textContent = `${formatAmount(immatureSats)} maturing — not spendable privately yet.`;
  }
}

function updateEmptyFundingState() {
  const funded = lastTotalSats > 0;
  const showFund = currentPhase === "ready" && !funded;

  el.txEmptyRecentTitle.textContent = showFund
    ? "Fund your wallet"
    : "No transactions yet.";
  el.txEmptyRecentHint.hidden = !showFund;
  el.btnFundReceive.hidden = !showFund;

  el.txEmptyTitle.textContent = showFund ? "Fund your wallet" : "No transactions yet.";
  el.txEmptyHint.hidden = !showFund;
  el.btnFundReceiveHistory.hidden = !showFund;
}

function openPublicReceive() {
  receiveMode = "public";
  applySegModes();
  setCard("receive");
}

const MWEB_COACH_PANELS: Array<{ kicker: string; title: string; body: string }> = [
  {
    kicker: "Public",
    title: "Public balance",
    body: "Public addresses start with ltc1. They work with exchanges and most Litecoin wallets. Transactions are visible on public explorers.",
  },
  {
    kicker: "Private",
    title: "Private balance (MWEB)",
    body: "Private stealth addresses start with ltcmweb1. Amounts and payment partners stay confidential among MWEB wallets.",
  },
  {
    kicker: "Swap",
    title: "Moving between Public and Private",
    body: "Swap moves your own funds. Public → Private is a peg-in that matures after about 6 blocks before you can spend privately.",
  },
];

async function showMwebCoach() {
  let step = 0;
  for (;;) {
    const panel = MWEB_COACH_PANELS[step];
    const isLast = step === MWEB_COACH_PANELS.length - 1;
    const actions: ModalAction[] = [];
    if (step > 0) actions.push({ id: "prev", label: "Back", kind: "ghost", nav: true });
    actions.push({ id: "skip", label: "Skip", kind: "ghost" });
    actions.push({
      id: isLast ? "done" : "next",
      label: isLast ? "Finish" : "Next",
      kind: "primary",
    });
    const result = await openModal({
      title: panel.title,
      wide: true,
      build: (body) => {
        const kicker = document.createElement("p");
        kicker.className = "coach-kicker";
        kicker.textContent = `${panel.kicker} · ${step + 1} of ${MWEB_COACH_PANELS.length}`;
        const text = document.createElement("p");
        text.className = "coach-body";
        text.textContent = panel.body;
        body.append(kicker, text);
      },
      actions,
      focus: () =>
        el.modalActions.querySelector<HTMLElement>(
          `[data-action="${isLast ? "done" : "next"}"]`,
        ),
    });
    if (result === "prev") {
      step = Math.max(0, step - 1);
      continue;
    }
    if (result === "next") {
      step = Math.min(MWEB_COACH_PANELS.length - 1, step + 1);
      continue;
    }
    // Skip, Finish, Escape, or overlay dismiss — mark seen so it does not nag.
    setMwebCoachSeen(true);
    return;
  }
}

function maybeShowMwebCoach() {
  if (currentPhase !== "ready" || isMwebCoachSeen()) return;
  void showMwebCoach();
}

function updateSecurityChecklist() {
  const show =
    currentPhase === "ready" &&
    lastTotalSats >= SECURITY_CHECKLIST_SATS &&
    !isSecurityChecklistDismissed();
  el.securityChecklist.hidden = !show;
  if (!show) return;

  const checks: Record<string, boolean> = {
    backup: isBackupVerified(),
    autolock: autoLockMinutes > 0,
    tls: el.settingsValidateTls.checked,
    wipe: true, // informational — always “acknowledged” via static copy
  };
  for (const item of el.securityChecklistList.querySelectorAll<HTMLElement>(".checklist-item")) {
    const key = item.dataset.check ?? "";
    const ok = checks[key] === true;
    item.classList.toggle("is-ok", ok);
    item.classList.toggle("is-miss", !ok);
  }
}

function renderSummary(s: WalletSummary) {
  lastSummary = s;
  el.networkBadge.textContent = s.network;
  el.balanceTotal.classList.remove("skeleton");
  el.balanceTotal.textContent = formatAmount(s.total_sats);
  lastTotalSats = s.total_sats;
  if (s.total_sats > 0) sawNonZeroBalance = true;
  el.balanceSats.textContent = formatAmountSubtitle(s.total_sats);
  renderFiat();
  el.balanceConfirmed.textContent = formatAmount(s.confirmed_sats);
  el.balanceTip.textContent = s.tip_height.toLocaleString("en-US");
  el.address.textContent = s.receive_address;
  void refreshPublicReceiveQr();

  // Spendable public balance shown on the Public toggle segments.
  const publicBalance = formatAmount(s.confirmed_sats);
  el.sendBalancePublic.textContent = publicBalance;
  el.receiveBalancePublic.textContent = publicBalance;
  el.swapBalancePublic.textContent = publicBalance;

  const pendingParts: string[] = [];
  if (s.trusted_pending_sats > 0) {
    pendingParts.push(`trusted pending ${formatAmount(s.trusted_pending_sats)}`);
  }
  if (s.untrusted_pending_sats > 0) {
    pendingParts.push(`untrusted pending ${formatAmount(s.untrusted_pending_sats)}`);
  }
  if (s.immature_sats > 0) {
    pendingParts.push(`immature ${formatAmount(s.immature_sats)}`);
  }
  if (pendingParts.length > 0) {
    el.balancePending.hidden = false;
    el.balancePending.textContent = pendingParts.join(" · ");
  } else {
    el.balancePending.hidden = true;
    el.balancePending.textContent = "";
  }
  // Cleared here; renderCombined repopulates when MWEB is available.
  el.balanceMwebDetail.hidden = true;
  el.balanceMwebDetail.textContent = "";
  updateMaturityBanner(0);
  updateBackupBanner();
  updateEmptyFundingState();
  updateSecurityChecklist();
}

function renderCombined(c: CombinedSummary) {
  lastCombined = c;
  renderSummary(c.transparent);
  // Hero "Total balance" is wallet-wide: transparent + MWEB.
  const grandTotal = c.transparent.total_sats + c.mweb_total_sats;
  el.balanceTotal.textContent = formatAmount(grandTotal);
  el.balanceSats.textContent = formatAmountSubtitle(grandTotal);
  lastTotalSats = grandTotal;
  if (grandTotal > 0) sawNonZeroBalance = true;
  renderFiat();
  setMwebVisible(true);

  let mwebText = formatAmount(c.mweb_total_sats);
  if (c.mweb_stale) {
    mwebText += c.mweb_synced_height != null
      ? ` · stale as of height ${c.mweb_synced_height}`
      : " · stale";
  }
  el.balanceMweb.textContent = mwebText;

  const detailParts: string[] = [];
  detailParts.push(`Spendable ${formatAmount(c.mweb_confirmed_sats)}`);
  if (c.mweb_immature_sats > 0) {
    detailParts.push(
      `Maturing ${formatAmount(c.mweb_immature_sats)} (available after ~${MWEB_PEGIN_MATURITY_BLOCKS} confirmations)`,
    );
  }
  if (c.mweb_unconfirmed_sats > 0) {
    detailParts.push(`Unconfirmed private ${formatAmount(c.mweb_unconfirmed_sats)}`);
  }
  const showDetail =
    c.mweb_immature_sats > 0 ||
    c.mweb_unconfirmed_sats > 0 ||
    c.mweb_total_sats > 0;
  el.balanceMwebDetail.hidden = !showDetail;
  el.balanceMwebDetail.textContent = showDetail ? detailParts.join(" · ") : "";

  el.mwebStatus.hidden = false;
  el.mwebStatus.textContent = c.mweb_status;
  if (c.mweb_receive_address) {
    el.mwebAddress.textContent = c.mweb_receive_address;
    void renderQr(el.mwebQr, c.mweb_receive_address);
  }

  // Private Send shows only spendable — maturing must never look sendable.
  el.sendBalancePrivate.textContent = formatAmount(c.mweb_confirmed_sats);
  let privateChip = formatAmount(c.mweb_confirmed_sats);
  if (c.mweb_immature_sats > 0) {
    privateChip += ` · maturing ${formatAmount(c.mweb_immature_sats)}`;
  }
  el.receiveBalancePrivate.textContent = privateChip;
  el.swapBalancePrivate.textContent = privateChip;
  updateBackupBanner();
  updateMaturityBanner(c.mweb_immature_sats);
  updateEmptyFundingState();
  updateSecurityChecklist();
}

function renderLastTxid() {
  if (!lastTxid) {
    el.lastTxid.hidden = true;
    el.lastTxid.textContent = "";
    return;
  }
  el.lastTxid.hidden = false;
  el.lastTxid.textContent = `Last txid: ${lastTxid}`;
}

function formatTxTime(timestamp: number | null): string {
  if (timestamp == null) return "";
  // Backend reports seconds; tolerate millisecond timestamps too.
  const date = new Date(timestamp > 1e12 ? timestamp : timestamp * 1_000);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatTxTimeLong(timestamp: number | null): string {
  if (timestamp == null) return "unknown";
  const date = new Date(timestamp > 1e12 ? timestamp : timestamp * 1_000);
  return Number.isNaN(date.getTime()) ? "unknown" : date.toLocaleString();
}

function txDirection(tx: TxRecord): "in" | "out" {
  return tx.net_sats >= 0 ? "in" : "out";
}

function formatSignedAmount(tx: TxRecord): string {
  if (hideBalances) return HIDDEN_AMOUNT;
  return `${txDirection(tx) === "in" ? "+" : "−"}${formatAmountPlain(Math.abs(tx.net_sats))}`;
}

function isPrivateTxKind(kind: TxKind): boolean {
  return kind === "pegin" || kind === "pegout" || kind === "mweb-send" || kind === "mweb-receive";
}

function filterHistoryRecords(txs: TxRecord[]): TxRecord[] {
  const q = historySearchQuery.trim().toLowerCase();
  return txs.filter((tx) => {
    if (historyFilter === "public" && tx.kind !== "transparent") return false;
    if (historyFilter === "private" && !isPrivateTxKind(tx.kind)) return false;
    if (historyFilter === "pending" && tx.confirmations !== 0) return false;
    if (!q) return true;
    const note = (txLabels[tx.txid] ?? "").toLowerCase();
    return tx.txid.toLowerCase().includes(q) || note.includes(q);
  });
}

function buildTxRow(tx: TxRecord, index: number): HTMLLIElement {
  const dir = txDirection(tx);

  const icon = document.createElement("span");
  icon.className = `tx-icon ${dir}`;
  icon.innerHTML = dir === "in" ? SVG_ARROW_IN : SVG_ARROW_OUT;

  const amount = document.createElement("span");
  amount.className = dir === "in" ? "tx-amt in" : "tx-amt";
  amount.textContent = formatSignedAmount(tx);

  const meta = document.createElement("span");
  meta.className = "tx-meta";
  const note = txLabels[tx.txid]?.trim();
  meta.textContent = [note, TX_KIND_LABELS[tx.kind], formatTxTime(tx.timestamp)]
    .filter(Boolean)
    .join(" · ");
  meta.hidden = meta.textContent === "";

  const main = document.createElement("div");
  main.className = "tx-main";
  main.append(amount, meta);

  const pill = document.createElement("span");
  const peginMaturing =
    tx.kind === "pegin" && tx.confirmations < MWEB_PEGIN_MATURITY_BLOCKS;
  if (peginMaturing) {
    const left = Math.max(0, MWEB_PEGIN_MATURITY_BLOCKS - tx.confirmations);
    pill.className = "pill pending";
    pill.textContent =
      tx.confirmations === 0 ? "maturing · pending" : `maturing · ${left} left`;
  } else {
    pill.className = tx.confirmations === 0 ? "pill pending" : "pill";
    pill.textContent =
      tx.confirmations === 0 ? "pending" : `${tx.confirmations.toLocaleString("en-US")} conf`;
  }

  const txid = document.createElement("span");
  txid.className = "tx-id";
  txid.textContent = `${tx.txid.slice(0, 8)}…${tx.txid.slice(-8)}`;
  txid.title = tx.txid;

  const side = document.createElement("div");
  side.className = "tx-side";
  side.append(pill, txid);

  const li = document.createElement("li");
  li.className = "tx-row";
  li.tabIndex = 0;
  li.dataset.txid = tx.txid;
  li.setAttribute("role", "button");
  li.setAttribute("aria-label", `Transaction ${formatSignedAmount(tx)} — show details`);
  li.append(icon, main, side);
  li.addEventListener("click", () => void openTxDetail(index));
  li.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      void openTxDetail(index);
    }
  });
  return li;
}

function renderHistory(txs: TxRecord[]) {
  txRecords = txs;
  el.txListRecent.textContent = "";
  el.txEmptyRecent.hidden = txs.length > 0;
  el.btnSeeAll.hidden = txs.length <= RECENT_TX_COUNT;
  el.historyToolbar.hidden = txs.length === 0;
  el.btnExportHistory.hidden = txs.length === 0;
  updateEmptyFundingState();
  txs
    .slice(0, RECENT_TX_COUNT)
    .forEach((tx, index) => el.txListRecent.appendChild(buildTxRow(tx, index)));
  renderHistoryFiltered();
}

function renderHistoryFiltered() {
  const filtered = filterHistoryRecords(txRecords);
  el.txList.textContent = "";
  const hasAny = txRecords.length > 0;
  const hasMatches = filtered.length > 0;
  el.txEmpty.hidden = hasMatches;
  if (!hasAny) {
    updateEmptyFundingState();
  } else if (!hasMatches) {
    el.txEmptyTitle.textContent = "No matching transactions.";
    el.txEmptyHint.hidden = true;
    el.btnFundReceiveHistory.hidden = true;
  }
  filtered.forEach((tx) => {
    const index = txRecords.findIndex((row) => row.txid === tx.txid);
    el.txList.appendChild(buildTxRow(tx, index >= 0 ? index : 0));
  });
}

/** Detail panel for one transaction; prev/next walk the cached list in place. */
async function openTxDetail(index: number) {
  let at = index;
  for (;;) {
    const tx = txRecords[at];
    if (!tx) return;

    const peginLeft =
      tx.kind === "pegin" && tx.confirmations < MWEB_PEGIN_MATURITY_BLOCKS
        ? Math.max(0, MWEB_PEGIN_MATURITY_BLOCKS - tx.confirmations)
        : null;
    const statusText =
      peginLeft != null
        ? tx.confirmations === 0
          ? "Maturing — waiting for first confirmation"
          : `Maturing — ~${peginLeft} block${peginLeft === 1 ? "" : "s"} remaining`
        : tx.confirmations === 0
          ? "Pending — not in a block yet"
          : `${tx.confirmations.toLocaleString("en-US")} confirmations`;
    const idLabel = txKindExplorable(tx.kind) ? "Transaction ID" : "Kernel ID";
    const rows: DetailRow[] = [
      ["Status", statusText],
      ["Type", TX_KIND_LABELS[tx.kind] || (txDirection(tx) === "in" ? "received" : "sent")],
      ["Time", formatTxTimeLong(tx.timestamp)],
    ];
    if (tx.height != null) rows.push(["Block height", tx.height.toLocaleString("en-US")]);
    if (tx.received_sats > 0) rows.push(["Received", formatAmount(tx.received_sats)]);
    if (tx.sent_sats > 0) rows.push(["Sent", formatAmount(tx.sent_sats)]);
    rows.push([
      "Fee",
      hideBalances
        ? HIDDEN_AMOUNT
        : tx.fee_sats != null
          ? `${tx.fee_sats.toLocaleString("en-US")} litoshis`
          : "unknown",
    ]);
    rows.push([idLabel, tx.txid, true]);

    const canExplore = txKindExplorable(tx.kind) && isChainTxid(tx.txid);
    let enrichment: TxEnrichment | null = null;
    if (canExplore) {
      enrichment = txEnrichmentCache.get(tx.txid) ?? null;
      if (!enrichment) {
        try {
          enrichment = await invoke<TxEnrichment>("fetch_tx_detail", { txid: tx.txid });
          txEnrichmentCache.set(tx.txid, enrichment);
        } catch {
          enrichment = null;
        }
      }
      if (enrichment?.fee_sats != null && tx.fee_sats == null && !hideBalances) {
        rows.splice(
          rows.findIndex((r) => r[0] === "Fee"),
          1,
          ["Fee", `${enrichment.fee_sats.toLocaleString("en-US")} litoshis`],
        );
      }
      if (enrichment?.status.block_hash) {
        rows.push(["Block hash", enrichment.status.block_hash, true]);
      }
    }

    const hasPrev = at > 0;
    const hasNext = at < txRecords.length - 1;
    const actions: ModalAction[] = [];
    if (hasPrev) actions.push({ id: "prev", label: "‹ Prev", kind: "ghost", nav: true });
    if (hasNext) actions.push({ id: "next", label: "Next ›", kind: "ghost", nav: true });
    if (canExplore) actions.push({ id: "explore", label: "View on litview", kind: "secondary" });
    actions.push({ id: "copy", label: "Copy ID", kind: "secondary" });
    actions.push({ id: "save-note", label: "Save note", kind: "secondary" });
    actions.push({ id: "close", label: "Close", kind: "primary" });

    const dir = txDirection(tx);
    let readLabel = () => txLabels[tx.txid] ?? "";
    const result = await openModal({
      title: `Transaction ${at + 1} of ${txRecords.length}`,
      wide: true,
      build: (body) => {
        const amount = document.createElement("p");
        amount.className = dir === "in" ? "detail-amount in" : "detail-amount";
        amount.textContent = formatSignedAmount(tx);
        body.append(amount, buildDetailList(rows));
        readLabel = appendTxLabelField(body, txLabels[tx.txid] ?? "");
        if (!canExplore) {
          appendParagraph(
            body,
            "Private transfers are not listed on public explorers — that is expected. Keep the Kernel ID as your reference.",
            "hint",
          );
        }
        if (enrichment) {
          body.append(
            buildIoSection("Inputs", enrichment.inputs),
            buildIoSection("Outputs", enrichment.outputs),
          );
        } else if (canExplore) {
          appendParagraph(
            body,
            "Could not load vin/vout from the explorer. Wallet-local details above are still valid.",
            "hint",
          );
        }
      },
      actions,
      focus: () => el.modalActions.querySelector<HTMLElement>('[data-action="close"]'),
      onKey: (event, close) => {
        if (event.key === "ArrowLeft" && hasPrev) {
          event.preventDefault();
          close("prev");
        } else if (event.key === "ArrowRight" && hasNext) {
          event.preventDefault();
          close("next");
        }
      },
    });

    const latestNote = readLabel();
    const priorNote = txLabels[tx.txid] ?? "";
    if (latestNote !== priorNote) {
      await persistTxLabel(tx.txid, latestNote);
      renderHistory(txRecords);
    }

    if (result === "prev") {
      at -= 1;
      continue;
    }
    if (result === "next") {
      at += 1;
      continue;
    }
    if (result === "explore") {
      await openExplorerForTxid(tx.txid);
      continue;
    }
    if (result === "copy") {
      await copyText(tx.txid, "Transaction ID copied.");
      continue;
    }
    if (result === "save-note") {
      setStatus(latestNote ? "Note saved." : "Note cleared.", "success");
      continue;
    }
    return;
  }
}

async function refreshHistory() {
  try {
    await refreshTxLabels();
    const txs = await invoke<TxRecord[]>("list_transactions");
    renderHistory(txs);
  } catch {
    // ignore when locked / not loaded
  }
}

async function refreshCombined() {
  try {
    const c = await invoke<CombinedSummary>("get_combined_summary");
    renderCombined(c);
  } catch {
    try {
      const s = await invoke<WalletSummary>("get_summary");
      renderSummary(s);
      setMwebVisible(false);
    } catch {
      /* ignore */
    }
  }
}

function renderContactsList() {
  el.contactsList.textContent = "";
  el.contactsEmpty.hidden = contactsCache.length > 0;
  for (const contact of contactsCache) {
    const li = document.createElement("li");
    li.className = "contact-row";

    const main = document.createElement("div");
    main.className = "contact-main";
    const name = document.createElement("span");
    name.className = "contact-name";
    name.textContent = contact.name;
    const meta = document.createElement("span");
    meta.className = "contact-meta";
    meta.textContent = contact.kind === "public" ? "Public" : "Private";
    const addr = document.createElement("span");
    addr.className = "contact-addr";
    addr.textContent = contact.address;
    main.append(name, meta, addr);

    const actions = document.createElement("div");
    actions.className = "contact-actions";
    const editBtn = document.createElement("button");
    editBtn.type = "button";
    editBtn.className = "btn btn-ghost btn-sm";
    editBtn.textContent = "Edit";
    editBtn.addEventListener("click", () => void editContactFlow(contact));
    const delBtn = document.createElement("button");
    delBtn.type = "button";
    delBtn.className = "btn btn-ghost btn-sm";
    delBtn.textContent = "Delete";
    delBtn.addEventListener("click", () => void deleteContactFlow(contact));
    actions.append(editBtn, delBtn);

    li.append(main, actions);
    el.contactsList.appendChild(li);
  }
}

async function refreshContacts() {
  try {
    contactsCache = (await invoke<ContactRecord[]>("list_contacts")) ?? [];
  } catch {
    contactsCache = [];
  }
  renderContactsList();
}

async function contactEditorFlow(existing: ContactRecord | null) {
  let nameInput!: HTMLInputElement;
  let addressInput!: HTMLInputElement;
  let kindSelect!: HTMLSelectElement;
  const result = await openModal({
    title: existing ? "Edit contact" : "Add contact",
    build: (body) => {
      const nameField = document.createElement("label");
      nameField.className = "field";
      nameField.innerHTML = `<span class="field-label">Name</span>`;
      nameInput = document.createElement("input");
      nameInput.type = "text";
      nameInput.maxLength = 64;
      nameInput.value = existing?.name ?? "";
      nameInput.autocomplete = "off";
      nameField.append(nameInput);

      const addrField = document.createElement("label");
      addrField.className = "field";
      addrField.innerHTML = `<span class="field-label">Address</span>`;
      addressInput = document.createElement("input");
      addressInput.className = "mono";
      addressInput.type = "text";
      addressInput.spellcheck = false;
      addressInput.autocomplete = "off";
      addressInput.placeholder = "ltc1… or ltcmweb1…";
      addressInput.value = existing?.address ?? "";
      addrField.append(addressInput);

      const kindField = document.createElement("label");
      kindField.className = "field";
      kindField.innerHTML = `<span class="field-label">Type</span>`;
      kindSelect = document.createElement("select");
      kindSelect.innerHTML = `
        <option value="public">Public</option>
        <option value="private">Private</option>
      `;
      kindSelect.value = existing?.kind ?? "public";
      kindField.append(kindSelect);

      const hint = document.createElement("p");
      hint.className = "hint";
      hint.textContent =
        "Public contacts use transparent addresses. Reusing them can link payments to this name.";
      body.append(nameField, addrField, kindField, hint);
    },
    actions: [
      { id: "cancel", label: "Cancel", kind: "ghost" },
      { id: "save", label: "Save", kind: "primary" },
    ],
  });
  if (result !== "save") return;
  const name = nameInput.value.trim();
  const address = addressInput.value.trim();
  const kind = kindSelect.value as ContactKind;
  if (!name || !address) {
    setStatus("Name and address are required.", "error");
    return;
  }
  try {
    await invoke<ContactRecord>("upsert_contact", {
      req: {
        id: existing?.id ?? null,
        name,
        address,
        kind,
      },
    });
    await refreshContacts();
    setStatus(existing ? "Contact updated." : "Contact saved.", "success");
  } catch (e) {
    setStatus(String(e), "error");
  }
}

async function editContactFlow(contact: ContactRecord) {
  await contactEditorFlow(contact);
}

async function deleteContactFlow(contact: ContactRecord) {
  const result = await openModal({
    title: "Delete contact",
    build: (body) => {
      const p = document.createElement("p");
      p.className = "lede";
      p.textContent = `Remove ${contact.name} from your address book?`;
      body.append(p);
    },
    actions: [
      { id: "cancel", label: "Cancel", kind: "ghost" },
      { id: "delete", label: "Delete", kind: "danger" },
    ],
  });
  if (result !== "delete") return;
  try {
    await invoke("delete_contact", { req: { id: contact.id } });
    await refreshContacts();
    setStatus("Contact deleted.", "success");
  } catch (e) {
    setStatus(String(e), "error");
  }
}

async function pickContactFlow(preferred: ContactKind) {
  await refreshContacts();
  if (contactsCache.length === 0) {
    setStatus("No contacts yet — add one in Settings.", "info");
    return;
  }
  const ordered = [
    ...contactsCache.filter((c) => c.kind === preferred),
    ...contactsCache.filter((c) => c.kind !== preferred),
  ];
  const picked = await pickContactModal(ordered);
  if (!picked) return;
  applyContactToSend(picked);
}

function applyContactToSend(contact: ContactRecord) {
  sendMode = contact.kind === "public" ? "public" : "private";
  applySegModes();
  setCard("send");
  if (contact.kind === "public") {
    el.sendAddress.value = contact.address;
    el.sendAddress.focus();
  } else {
    el.mwebSendAddress.value = contact.address;
    el.mwebSendAddress.focus();
  }
}

async function pickContactModal(contacts: ContactRecord[]): Promise<ContactRecord | null> {
  let chosen: ContactRecord | null = null;
  const action = await openModal({
    title: "Choose contact",
    build: (body) => {
      const list = document.createElement("ul");
      list.className = "contact-pick-list";
      for (const contact of contacts) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "contact-pick-row";
        const name = document.createElement("span");
        name.className = "contact-name";
        name.textContent = contact.name;
        const meta = document.createElement("span");
        meta.className = "contact-meta";
        meta.textContent = contact.kind === "public" ? "Public" : "Private";
        const addr = document.createElement("span");
        addr.className = "contact-addr";
        addr.textContent = contact.address;
        btn.append(name, meta, addr);
        btn.addEventListener("click", () => {
          chosen = contact;
          closeModal("picked");
        });
        const li = document.createElement("li");
        li.append(btn);
        list.append(li);
      }
      body.append(list);
    },
    actions: [{ id: "cancel", label: "Cancel", kind: "ghost" }],
  });
  return action === "picked" ? chosen : null;
}

async function loadSettings() {
  try {
    const s = await invoke<WalletSettings>("get_settings");
    el.settingsExplorer.value = s.explorer_base_url || "https://litview.space";
    el.settingsShowFiat.checked = s.show_fiat ?? true;
    el.settingsFeeHints.checked = s.use_explorer_fee_hints ?? true;
    showFiat = s.show_fiat ?? true;
    useExplorerFeeHints = s.use_explorer_fee_hints ?? true;
    el.settingsElectrum.value = s.electrum_url;
    el.settingsValidateTls.checked = s.electrum_validate_domain ?? true;
    el.settingsPublicFallback.checked = s.electrum_use_public_fallback ?? true;
    el.settingsAutoLock.value = String(s.auto_lock_minutes ?? 15);
    autoLockMinutes = s.auto_lock_minutes ?? 15;
    if (s.electrum_active_url && s.electrum_active_url !== s.electrum_url) {
      el.settingsActiveServer.hidden = false;
      el.settingsActiveServer.textContent = `Currently connected to fallback server: ${s.electrum_active_url}`;
    } else if (s.electrum_active_url) {
      el.settingsActiveServer.hidden = false;
      el.settingsActiveServer.textContent = `Currently connected to: ${s.electrum_active_url}`;
    } else {
      el.settingsActiveServer.hidden = true;
      el.settingsActiveServer.textContent = "";
    }
    el.settingsRpc.value = s.litecoin_rpc_url ?? "";
    el.settingsPeers.value = s.mweb_peers.join(", ");
    el.settingsMwebScheme.value = s.mweb_scheme ?? "litecoin-core";
    updateSecurityChecklist();
    await refreshContacts();
  } catch {
    /* ignore */
  }
}

function renderMwebProgress(p: MwebSyncProgress) {
  // Only worth showing for real downloads; steady-state diffs finish instantly.
  if (!p.active || p.total < 100) {
    el.mwebProgress.hidden = true;
    return;
  }
  const pct = Math.min(100, Math.round((p.fetched / p.total) * 100));
  const text = `Downloading MWEB outputs: ${p.fetched.toLocaleString(
    "en-US",
  )} / ${p.total.toLocaleString("en-US")} (${pct}%)`;
  el.mwebProgress.hidden = false;
  el.mwebProgressFill.style.width = `${pct}%`;
  el.mwebProgressText.textContent = text;
  // The loading overlay covers the bar during a resync, so mirror it there.
  setLoadingLabel(text);
}

function startMwebProgressPolling() {
  stopMwebProgressPolling();
  mwebProgressTimer = window.setInterval(async () => {
    try {
      const p = await invoke<MwebSyncProgress>("mweb_sync_progress");
      renderMwebProgress(p);
    } catch {
      /* ignore while locked / not loaded */
    }
  }, 400);
}

function stopMwebProgressPolling() {
  if (mwebProgressTimer != null) {
    clearInterval(mwebProgressTimer);
    mwebProgressTimer = null;
  }
  el.mwebProgress.hidden = true;
}

function startAutoSync() {
  stopAutoSync();
  autoSyncTimer = window.setInterval(() => {
    if (currentPhase !== "ready" || syncing || sending) return;
    void runSync({ quiet: true });
    void refreshSpotPrice();
  }, AUTO_SYNC_MS);
}

function stopAutoSync() {
  if (autoSyncTimer != null) {
    clearInterval(autoSyncTimer);
    autoSyncTimer = null;
  }
}

/* ---------------------------------------------------------------------------
   Auto-lock: drop the decrypted key material after a period without user
   input. The backend clears it on lock_wallet; this timer only decides when.
   --------------------------------------------------------------------------- */

let autoLockMinutes = 15;
let lastActivityTs = Date.now();

for (const event of ["pointerdown", "keydown", "wheel", "mousemove"] as const) {
  document.addEventListener(event, () => {
    lastActivityTs = Date.now();
  });
}

window.setInterval(() => {
  // Allow auto-lock during sync (lock is non-blocking). Still wait out an in-flight send.
  if (currentPhase !== "ready" || autoLockMinutes <= 0 || sending) return;
  if (Date.now() - lastActivityTs < autoLockMinutes * 60_000) return;
  void lockWallet("Wallet locked after inactivity.");
}, 30_000);

type PassStrength = {
  ok: boolean;
  level: 0 | 1 | 2 | 3;
  label: string;
  reason: string | null;
};

function scorePassphrase(pw: string): PassStrength {
  if (!pw) {
    return { ok: false, level: 0, label: "", reason: "Passphrase is required." };
  }
  if (pw.length < MIN_PASSPHRASE_LEN) {
    return {
      ok: false,
      level: 0,
      label: "Too short",
      reason: `Use at least ${MIN_PASSPHRASE_LEN} characters.`,
    };
  }
  const variety = [
    /[a-z]/.test(pw),
    /[A-Z]/.test(pw),
    /\d/.test(pw),
    /[^A-Za-z0-9]/.test(pw),
  ].filter(Boolean).length;
  let level: 1 | 2 | 3 = 1;
  if (pw.length >= 16 && variety >= 3) level = 3;
  else if (pw.length >= 12 && variety >= 2) level = 2;
  const labels = ["", "Weak", "OK", "Strong"] as const;
  return { ok: true, level, label: labels[level], reason: null };
}

function requireWalletPassphrase(a: string, b: string): string | null {
  const strength = scorePassphrase(a);
  if (!strength.ok) return strength.reason;
  if (a !== b) return "Passphrases do not match.";
  return null;
}

function renderPassMeter(
  pw: string,
  confirm: string,
  meter: HTMLElement,
  fill: HTMLElement,
  label: HTMLElement,
) {
  if (!pw) {
    meter.hidden = true;
    fill.dataset.level = "0";
    label.textContent = "";
    return;
  }
  meter.hidden = false;
  const strength = scorePassphrase(pw);
  fill.dataset.level = String(strength.level);
  if (!strength.ok) {
    label.textContent = strength.reason ?? "Too short";
    return;
  }
  if (confirm && confirm !== pw) {
    label.textContent = `${strength.label} — passphrases do not match.`;
    return;
  }
  label.textContent =
    strength.level <= 1
      ? `${strength.label} — consider a longer passphrase with mixed characters.`
      : strength.label;
}

function syncPassMeters() {
  renderPassMeter(
    el.onboardPassphrase.value,
    el.onboardPassphrase2.value,
    el.onboardPassMeter,
    el.onboardPassFill,
    el.onboardPassLabel,
  );
  renderPassMeter(
    el.restorePassphrase.value,
    el.restorePassphrase2.value,
    el.restorePassMeter,
    el.restorePassFill,
    el.restorePassLabel,
  );
  renderPassMeter(
    el.migratePassphrase.value,
    el.migratePassphrase2.value,
    el.migratePassMeter,
    el.migratePassFill,
    el.migratePassLabel,
  );
  updateBusyUi();
}

async function boot() {
  setPhase("boot");
  setError(null);
  applyTheme(readThemePref());
  syncPassMeters();
  try {
    const exists = await invoke<boolean>("wallet_exists");
    if (!exists) {
      setPhase("onboarding");
      return;
    }
    const needsMigration = await invoke<boolean>("wallet_needs_migration");
    if (needsMigration) {
      setPhase("migrate");
      return;
    }
    const locked = await invoke<boolean>("wallet_is_locked");
    if (locked) {
      setPhase("unlock");
      return;
    }
    await enterReady();
  } catch (e) {
    setError(String(e));
    setPhase("fatal");
  }
}

async function enterReady() {
  displayUnit = readDisplayUnit();
  hideBalances = readHideBalances();
  syncAmountFieldLabels();
  const s = await invoke<WalletSummary>("load_wallet");
  renderSummary(s);
  // Legacy installs that already hold funds skip the first-receive modal.
  if (lastTotalSats > 0 && !isFirstReceiveSeen()) setFirstReceiveSeen(true);
  lastPendingSats = transparentPendingSats(s);
  setPhase("ready");
  setView("balance");
  await refreshCombined();
  await refreshHistory();
  await loadSettings();
  void refreshSpotPrice();
  void refreshFeeLadder();
  void runSync({ quiet: false });
  maybeShowMwebCoach();
}

/** Phrase required by the `wipe_wallet` command; enforced backend-side too. */
const WIPE_PHRASE = "DELETE WALLET";

/**
 * Destructive-action gate: the user must type the wipe phrase. Returns the
 * typed value (passed through IPC so the backend check is meaningful) or
 * null when cancelled or mismatched.
 */
async function confirmWipePhrase(): Promise<string | null> {
  let value = "";
  let input: HTMLInputElement | null = null;
  const result = await openModal({
    title: "Reset wallet data?",
    build: (body) => {
      appendParagraph(
        body,
        "This deletes the local wallet, its encrypted mnemonic and all cached chain data from this machine.",
        "lede",
      );
      appendParagraph(
        body,
        "Funds are only recoverable afterwards with your recovery phrase. Without that backup they are gone for good.",
        "hint",
      );
      const label = document.createElement("label");
      label.className = "field";
      const caption = document.createElement("span");
      caption.className = "field-label";
      caption.textContent = `Type "${WIPE_PHRASE}" to confirm`;
      input = document.createElement("input");
      input.type = "text";
      input.autocomplete = "off";
      input.spellcheck = false;
      input.className = "mono";
      input.addEventListener("input", () => {
        value = input!.value;
      });
      input.addEventListener("keydown", (event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          closeModal("confirm");
        }
      });
      label.append(caption, input);
      body.appendChild(label);
    },
    actions: [
      { id: "cancel", label: "Cancel", kind: "ghost" },
      { id: "confirm", label: "Delete wallet data", kind: "danger" },
    ],
    focus: () => input,
  });
  if (result !== "confirm") return null;
  if (value.trim() !== WIPE_PHRASE) {
    setStatus(`Wallet not reset — you must type "${WIPE_PHRASE}" exactly.`, "error");
    return null;
  }
  return value;
}

async function wipeAndOnboard() {
  // No passphrase gate here: this button exists precisely for people who can no
  // longer unlock, so requiring the passphrase would block the only way out.
  // The typed phrase (checked again backend-side) is the destructive-action gate.
  const confirmation = await confirmWipePhrase();
  if (confirmation === null) return;

  setError(null);
  showLoading("Resetting wallet data…");
  try {
    await invoke("wipe_wallet", { confirmation });
  } finally {
    hideLoading();
  }
  lastTxid = null;
  clearPendingMnemonic();
  clearBackupLocalFlags();
  renderLastTxid();
  renderHistory([]);
  setPhase("onboarding");
  syncPassMeters();
  setStatus("Wallet data reset — create or restore a wallet.");
}

el.btnWipe.addEventListener("click", () => void wipeAndOnboard().catch((e) => setError(String(e))));
el.btnWipeUnlock.addEventListener("click", () =>
  void wipeAndOnboard().catch((e) => setError(String(e))),
);

el.btnUnlock.addEventListener("click", async () => {
  setError(null);
  try {
    await invoke("unlock_wallet", {
      req: { passphrase: el.unlockPassphrase.value },
    });
    el.unlockPassphrase.value = "";
    await enterReady();
  } catch (e) {
    setError(String(e));
  }
});

el.btnMigrate.addEventListener("click", async () => {
  const err = requireWalletPassphrase(
    el.migratePassphrase.value,
    el.migratePassphrase2.value,
  );
  if (err) {
    setError(err);
    return;
  }
  setError(null);
  try {
    await invoke("migrate_encrypt", {
      req: { passphrase: el.migratePassphrase.value },
    });
    el.migratePassphrase.value = "";
    el.migratePassphrase2.value = "";
    syncPassMeters();
    await enterReady();
  } catch (e) {
    setError(String(e));
  }
});

async function runSync(opts: { quiet: boolean }): Promise<boolean> {
  if (syncing || sending) return false;
  syncing = true;
  if (!opts.quiet) setError(null);
  updateBusyUi();
  startMwebProgressPolling();
  const prevPending = lastPendingSats;
  const prevTotal = lastTotalSats;
  const hadFundsBefore = prevTotal > 0 || sawNonZeroBalance;
  try {
    const result = await invoke<SyncResult>("sync_wallet");
    // Lock may have completed while sync was finishing.
    if (currentPhase !== "ready") return false;
    renderSummary(result.summary);
    await refreshCombined();
    await refreshHistory();
    syncState = "ok";
    const timing =
      result.mweb_ms > 0
        ? `${formatMs(result.electrum_ms)} + ${formatMs(result.mweb_ms)} MWEB`
        : formatMs(result.electrum_ms);
    const pendingNow =
      transparentPendingSats(result.summary) +
      (lastCombined?.mweb_unconfirmed_sats ?? 0);
    const pendingRose = pendingNow > prevPending;
    const receivedSignal = result.new_txs > 0 || pendingRose;
    lastPendingSats = pendingNow;
    if (result.electrum_server) {
      el.settingsActiveServer.hidden = false;
      el.settingsActiveServer.textContent = `Last sync used: ${result.electrum_server}`;
    }
    if (result.warnings?.length) {
      // Cross-check findings outrank the feel-good sync message.
      for (const warning of result.warnings) console.warn(warning);
      setStatus(result.warnings[0], "error");
    } else if (receivedSignal) {
      setStatus(`Received funds — syncing details… (${timing})`, "success");
      pulseRecentHistoryRows();
      const firstReceive =
        !isFirstReceiveSeen() && !hadFundsBefore && lastTotalSats > 0;
      if (firstReceive) void showFirstReceiveModal();
      else if (!isFirstReceiveSeen() && lastTotalSats > 0) {
        // Legacy wallet already funded before this feature — don't nag.
        setFirstReceiveSeen(true);
      }
    } else {
      setStatus(`Synced in ${timing}`, "success");
    }
    return true;
  } catch (e) {
    if (currentPhase !== "ready") return false;
    syncState = "error";
    const message = String(e);
    // Expected when the user locks mid-sync — not a failure to surface.
    if (/locked/i.test(message)) return false;
    if (opts.quiet) setStatus(`Auto-sync failed: ${e}`, "error");
    else setError(message);
    return false;
  } finally {
    stopMwebProgressPolling();
    syncing = false;
    if (currentPhase === "ready") updateBusyUi();
  }
}

async function lockWallet(statusMessage = "Wallet locked.") {
  stopAutoSync();
  stopMwebProgressPolling();
  syncing = false;
  setPhase("unlock");
  setStatus(statusMessage);
  updateBusyUi();
  try {
    await invoke("lock_wallet");
  } catch (e) {
    setError(String(e));
  }
}

el.restoreMnemonic.addEventListener("input", updateBusyUi);

for (const input of [
  el.onboardPassphrase,
  el.onboardPassphrase2,
  el.restorePassphrase,
  el.restorePassphrase2,
  el.migratePassphrase,
  el.migratePassphrase2,
]) {
  input.addEventListener("input", syncPassMeters);
}

el.btnCreate.addEventListener("click", async () => {
  if (syncing || sending) return;
  if (el.restoreMnemonic.value.trim()) {
    setError(
      "You entered a recovery phrase — click “Restore wallet” below, or clear the phrase to create a new wallet.",
    );
    return;
  }
  const err = requireWalletPassphrase(
    el.onboardPassphrase.value,
    el.onboardPassphrase2.value,
  );
  if (err) {
    setError(err);
    return;
  }
  syncing = true;
  setError(null);
  updateBusyUi();
  try {
    const passphrase = el.onboardPassphrase.value;
    const resp = await invoke<CreateWalletResponse>("create_wallet", {
      req: { network: "mainnet" },
      passphrase,
    });
    pendingMnemonic = resp.mnemonic;
    renderMnemonic(resp.mnemonic);
    renderSummary(resp.summary);
    el.onboardPassphrase.value = "";
    el.onboardPassphrase2.value = "";
    syncPassMeters();
    showMnemonicStep("show");
    setPhase("mnemonic");
    setStatus(null);
  } catch (e) {
    setError(String(e));
  } finally {
    syncing = false;
    updateBusyUi();
  }
});

el.btnRestore.addEventListener("click", async () => {
  if (syncing || sending) return;
  const mnemonic = el.restoreMnemonic.value.trim();
  if (!mnemonic) {
    setError("Enter a recovery phrase or extended key to restore.");
    return;
  }
  const err = requireWalletPassphrase(
    el.restorePassphrase.value,
    el.restorePassphrase2.value,
  );
  if (err) {
    setError(err);
    return;
  }
  syncing = true;
  setError(null);
  updateBusyUi();
  showLoading("Restoring wallet and scanning for coins…");
  try {
    const passphrase = el.restorePassphrase.value;
    const aezeedPass = el.restoreAezeedPass.value;
    await invoke<WalletSummary>("restore_wallet", {
      req: {
        mnemonic,
        network: "mainnet",
        mweb_scheme: el.restoreMwebScheme.value as MwebScheme,
        aezeed_passphrase: aezeedPass ? aezeedPass : null,
      },
      passphrase,
    });
    // Restoring implies the user already holds a backup; treat as verified.
    setBackupVerified(true);
    setBackupBannerDismissed(false);
    el.restorePassphrase.value = "";
    el.restorePassphrase2.value = "";
    el.restoreMnemonic.value = "";
    el.restoreAezeedPass.value = "";
    syncPassMeters();
    syncing = false;
    updateBusyUi();
    await enterReady();
  } catch (e) {
    setError(String(e));
    syncing = false;
    updateBusyUi();
  } finally {
    hideLoading();
  }
});

el.btnMnemonicToVerify.addEventListener("click", () => {
  if (!pendingMnemonic) {
    setError("Recovery phrase is no longer available. Reset and create a new wallet.");
    return;
  }
  const words = splitMnemonicWords(pendingMnemonic);
  quizPositions = pickQuizPositions(words.length);
  buildMnemonicQuiz(quizPositions);
  showMnemonicStep("verify");
  el.mnemonicQuiz.querySelector<HTMLButtonElement>(".mnemonic-quiz-bank .mnemonic-quiz-chip")?.focus();
});

el.btnMnemonicShowAgain.addEventListener("click", () => {
  if (!pendingMnemonic) {
    setError("Recovery phrase is no longer available. Reset and create a new wallet.");
    return;
  }
  renderMnemonic(pendingMnemonic);
  showMnemonicStep("show");
  el.btnMnemonicDone.disabled = true;
});

el.btnMnemonicDone.addEventListener("click", () => {
  if (!pendingMnemonic) {
    setError("Recovery phrase is no longer available. Reset and create a new wallet.");
    return;
  }
  if (!quizIsComplete()) {
    el.mnemonicQuizError.hidden = false;
    el.mnemonicQuizError.textContent =
      "Fill each numbered slot with the matching word from your written phrase.";
    return;
  }
  clearPendingMnemonic();
  setBackupVerified(true);
  setBackupBannerDismissed(false);
  void enterReady();
});

el.btnBackupBannerDismiss.addEventListener("click", () => {
  setBackupBannerDismissed(true);
  updateBackupBanner();
});

el.btnFundReceive.addEventListener("click", () => openPublicReceive());
el.btnFundReceiveHistory.addEventListener("click", () => openPublicReceive());

el.btnSecurityChecklistDismiss.addEventListener("click", () => {
  setSecurityChecklistDismissed(true);
  updateSecurityChecklist();
});

el.btnSync.addEventListener("click", () => {
  void runSync({ quiet: false });
});

el.btnAddress.addEventListener("click", async () => {
  if (syncing || sending) return;
  syncing = true;
  setError(null);
  updateBusyUi();
  try {
    const address = await invoke<string>("get_receive_address");
    el.address.textContent = address;
    await refreshPublicReceiveQr();
    setStatus("New receive address generated.", "success");
  } catch (e) {
    setError(String(e));
  } finally {
    syncing = false;
    updateBusyUi();
  }
});

el.btnCopy.addEventListener("click", async () => {
  const address = el.address.textContent?.trim() ?? "";
  if (!address) {
    setStatus("No address to copy yet.", "error");
    return;
  }
  try {
    await navigator.clipboard.writeText(address);
    flashLabel(el.btnCopy, "Copied");
  } catch {
    setStatus("Copy failed — select the address manually.", "error");
  }
});

el.btnCopyPayment.addEventListener("click", async () => {
  const address = el.address.textContent?.trim() ?? "";
  if (!address) {
    setStatus("No address to copy yet.", "error");
    return;
  }
  const { amountSats, label } = publicReceiveRequest();
  if (el.receiveAmount.value.trim() && (amountSats == null || amountSats <= 0)) {
    setStatus(amountError("request", el.receiveAmount.value), "error");
    return;
  }
  const uri = buildPaymentUri(address, { amountSats, label });
  try {
    await navigator.clipboard.writeText(uri);
    flashLabel(el.btnCopyPayment, "Copied");
  } catch {
    setStatus("Copy failed — select the payment link manually.", "error");
  }
});

function tryParseSendPaymentUri(raw: string): boolean {
  const trimmed = raw.trim();
  if (!/^litecoin:/i.test(trimmed)) return false;
  const parsed = parsePaymentUri(trimmed);
  if (!parsed) {
    setStatus("Could not parse that payment request.", "error");
    return true;
  }
  el.sendAddress.value = parsed.address;
  if (parsed.amountSats != null) {
    el.sendAmount.value = formatAmountInput(parsed.amountSats);
    el.sendDrain.checked = false;
  }
  setStatus("Parsed payment request.", "success");
  updateBusyUi();
  return true;
}

el.sendAddress.addEventListener("paste", (event) => {
  const text = event.clipboardData?.getData("text") ?? "";
  if (!/^litecoin:/i.test(text.trim())) return;
  event.preventDefault();
  tryParseSendPaymentUri(text);
});

el.sendAddress.addEventListener("blur", () => {
  tryParseSendPaymentUri(el.sendAddress.value);
});

el.receiveAmount.addEventListener("input", () => {
  void refreshPublicReceiveQr();
});
el.receiveLabel.addEventListener("input", () => {
  void refreshPublicReceiveQr();
});

/** Hero tap: LTC → litoshis → hidden → LTC. */
function cycleDisplayUnit() {
  if (hideBalances) {
    setDisplayUnit("ltc");
    return;
  }
  if (displayUnit === "ltc") {
    setDisplayUnit("litoshis", { keepHidden: true });
    return;
  }
  setHideBalances(true);
}

el.balanceTotal.addEventListener("click", () => cycleDisplayUnit());
el.balanceTotal.addEventListener("keydown", (event) => {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    cycleDisplayUnit();
  }
});

el.settingsUnitLtc.addEventListener("change", () => {
  if (el.settingsUnitLtc.checked) setDisplayUnit("ltc");
});
el.settingsUnitLitoshis.addEventListener("change", () => {
  if (el.settingsUnitLitoshis.checked) setDisplayUnit("litoshis");
});
el.settingsHideBalances.addEventListener("change", () => {
  setHideBalances(el.settingsHideBalances.checked);
});

el.feeCustom.addEventListener("input", () => {
  const raw = el.feeCustom.value.trim();
  if (!raw) {
    customFeeActive = false;
    selectedFeeRateSatVb = null;
    el.sendFeeHint.textContent = useExplorerFeeHints
      ? "Network fee is calculated automatically."
      : el.sendFeeHint.textContent;
    void refreshFeeLadder();
    return;
  }
  const n = Math.trunc(Number(raw));
  if (!Number.isFinite(n) || n < 1) return;
  customFeeActive = true;
  selectedFeeRateSatVb = n;
  el.sendFeeHint.textContent = `Using custom ${n} sat/vB.`;
  if (useExplorerFeeHints) void refreshFeeLadder();
  else void refreshFeeEstimate();
});

el.btnCopyMweb.addEventListener("click", async () => {
  const address = el.mwebAddress.textContent?.trim() ?? "";
  if (!address) return;
  try {
    await navigator.clipboard.writeText(address);
    flashLabel(el.btnCopyMweb, "Copied");
  } catch {
    setStatus("Copy failed — select the address manually.", "error");
  }
});

el.btnResyncMweb.addEventListener("click", async () => {
  if (syncing || sending) return;
  const confirmed = await openConfirm({
    title: "Resync MWEB from scratch?",
    message:
      "This wipes the local MWEB coin database and re-downloads the full UTXO set from the network.",
    detail: "Your coins are re-discovered from the chain. It can take a while on a slow connection.",
    confirmLabel: "Resync MWEB",
    danger: true,
  });
  if (!confirmed) return;
  syncing = true;
  setError(null);
  updateBusyUi();
  showLoading("Resyncing MWEB from scratch…");
  startMwebProgressPolling();
  try {
    await invoke("resync_mweb");
    await refreshCombined();
    setStatus("MWEB resynced.", "success");
  } catch (e) {
    setError(String(e));
  } finally {
    stopMwebProgressPolling();
    hideLoading();
    syncing = false;
    updateBusyUi();
  }
});

el.btnApplyMwebScheme.addEventListener("click", async () => {
  if (syncing || sending) return;
  const scheme = el.settingsMwebScheme.value as MwebScheme;
  const confirmed = await openConfirm({
    title: "Change MWEB derivation?",
    message:
      "Switching schemes wipes the local MWEB data and rescans the chain for coins under a different key branch.",
    rows: [["New scheme", scheme]],
    detail:
      "Pick the wrong scheme and your private balance will read as empty until you switch back. Transparent funds are untouched.",
    confirmLabel: "Change and rescan",
    danger: true,
  });
  if (!confirmed) return;
  if (!(await requirePassphrase("Changing the MWEB derivation scheme rebuilds your private coin database."))) {
    return;
  }
  syncing = true;
  setError(null);
  updateBusyUi();
  showLoading("Rescanning MWEB under the new derivation scheme…");
  startMwebProgressPolling();
  try {
    await invoke("set_mweb_scheme", { scheme });
    await refreshCombined();
    setStatus("MWEB derivation scheme applied.", "success");
  } catch (e) {
    setError(String(e));
  } finally {
    stopMwebProgressPolling();
    hideLoading();
    syncing = false;
    updateBusyUi();
  }
});

type CoinControlPanel = {
  details: HTMLDetailsElement;
  sum: HTMLElement;
  list: HTMLUListElement;
  empty: HTMLElement;
  drain: HTMLInputElement;
  selected: Set<string>;
};

function selectedOutpointsFor(panel: CoinControlPanel): string[] | undefined {
  if (panel.drain.checked || panel.selected.size === 0) return undefined;
  return Array.from(panel.selected);
}

function updateCoinControlSum(panel: CoinControlPanel) {
  if (panel.selected.size === 0) {
    panel.sum.hidden = true;
    panel.sum.textContent = "";
    return;
  }
  let sum = 0;
  for (const utxo of utxoCache) {
    if (panel.selected.has(utxo.outpoint)) sum += utxo.amount_sats;
  }
  panel.sum.hidden = false;
  panel.sum.textContent = `Selected ${panel.selected.size} coin${
    panel.selected.size === 1 ? "" : "s"
  } · ${formatAmountPlain(sum)}`;
}

function renderUtxoList(panel: CoinControlPanel) {
  panel.list.textContent = "";
  panel.empty.hidden = utxoCache.length > 0;
  for (const utxo of utxoCache) {
    const li = document.createElement("li");
    li.className = utxo.locked ? "utxo-row is-locked" : "utxo-row";

    const check = document.createElement("input");
    check.type = "checkbox";
    check.disabled = utxo.locked || panel.drain.checked;
    check.checked = panel.selected.has(utxo.outpoint);
    check.addEventListener("change", () => {
      if (check.checked) panel.selected.add(utxo.outpoint);
      else panel.selected.delete(utxo.outpoint);
      updateCoinControlSum(panel);
    });

    const main = document.createElement("div");
    main.className = "utxo-main";
    const amt = document.createElement("span");
    amt.textContent = formatAmountPlain(utxo.amount_sats);
    const meta = document.createElement("span");
    meta.className = "utxo-meta";
    const kind = utxo.keychain === "internal" ? "change" : "receive";
    const conf =
      utxo.confirmations === 0
        ? "pending"
        : `${utxo.confirmations.toLocaleString("en-US")} conf`;
    meta.textContent = `${kind} · ${conf}${utxo.locked ? " · frozen" : ""}`;
    const id = document.createElement("span");
    id.className = "utxo-id";
    id.textContent = utxo.outpoint;
    main.append(amt, meta, id);

    const freeze = document.createElement("button");
    freeze.type = "button";
    freeze.className = "btn btn-ghost btn-sm utxo-freeze";
    freeze.textContent = utxo.locked ? "Unfreeze" : "Freeze";
    freeze.addEventListener("click", () => void toggleUtxoLocked(utxo));

    li.append(check, main, freeze);
    panel.list.appendChild(li);
  }
  updateCoinControlSum(panel);
}

const sendCoinPanel: CoinControlPanel = {
  details: el.coinControl,
  sum: el.coinControlSum,
  list: el.utxoList,
  empty: el.utxoEmpty,
  drain: el.sendDrain,
  selected: sendSelectedOutpoints,
};

const peginCoinPanel: CoinControlPanel = {
  details: el.peginCoinControl,
  sum: el.peginCoinControlSum,
  list: el.peginUtxoList,
  empty: el.peginUtxoEmpty,
  drain: el.peginDrain,
  selected: peginSelectedOutpoints,
};

function pruneSelectedOutpoints(selected: Set<string>) {
  const live = new Set(utxoCache.map((u) => u.outpoint));
  for (const op of Array.from(selected)) {
    if (!live.has(op)) selected.delete(op);
  }
}

async function refreshUtxos() {
  try {
    utxoCache = (await invoke<UtxoRecord[]>("list_unspent")) ?? [];
  } catch {
    utxoCache = [];
  }
  pruneSelectedOutpoints(sendSelectedOutpoints);
  pruneSelectedOutpoints(peginSelectedOutpoints);
  renderUtxoList(sendCoinPanel);
  renderUtxoList(peginCoinPanel);
}

async function toggleUtxoLocked(utxo: UtxoRecord) {
  try {
    await invoke("set_utxo_locked", {
      req: { outpoint: utxo.outpoint, locked: !utxo.locked },
    });
    if (!utxo.locked) {
      sendSelectedOutpoints.delete(utxo.outpoint);
      peginSelectedOutpoints.delete(utxo.outpoint);
    }
    await refreshUtxos();
    setStatus(utxo.locked ? "Coin unfrozen." : "Coin frozen.", "success");
  } catch (e) {
    setStatus(String(e), "error");
  }
}

el.coinControl.addEventListener("toggle", () => {
  if (el.coinControl.open) void refreshUtxos();
});
el.peginCoinControl.addEventListener("toggle", () => {
  if (el.peginCoinControl.open) void refreshUtxos();
});
el.btnRefreshUtxos.addEventListener("click", () => void refreshUtxos());
el.btnRefreshPeginUtxos.addEventListener("click", () => void refreshUtxos());

el.sendDrain.addEventListener("change", () => {
  if (el.sendDrain.checked) sendSelectedOutpoints.clear();
  renderUtxoList(sendCoinPanel);
  updateBusyUi();
});
el.peginDrain.addEventListener("change", () => {
  if (el.peginDrain.checked) peginSelectedOutpoints.clear();
  renderUtxoList(peginCoinPanel);
  updateBusyUi();
});
el.mwebSendDrain.addEventListener("change", () => {
  updateBusyUi();
});
el.pegoutDrain.addEventListener("change", () => {
  updateBusyUi();
});

el.sendForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (syncing || sending) return;

  const drain = el.sendDrain.checked;
  // Prefer URI parse if the address field still holds a payment request.
  tryParseSendPaymentUri(el.sendAddress.value);
  const address = el.sendAddress.value.trim();
  if (!address || /^litecoin:/i.test(address)) {
    setError(
      !address
        ? "Enter a destination address."
        : "Could not parse that payment request.",
    );
    return;
  }

  let amount_sats = 0;
  if (!drain) {
    const parsed = parseAmountToSats(el.sendAmount.value);
    if (parsed == null) {
      setError(amountError("send", el.sendAmount.value));
      return;
    }
    if (parsed < DUST_LITOSHIS) {
      setError(`Amount must be ≥ ${DUST_LITOSHIS} litoshis (Litecoin dust limit for ltc1).`);
      return;
    }
    amount_sats = parsed;
  }

  const selected_outpoints = selectedOutpointsFor(sendCoinPanel);
  if (drain && selected_outpoints?.length) {
    setError("Turn off Choose coins or Send all — they cannot be used together.");
    return;
  }

  sending = true;
  setError(null);
  updateBusyUi();
  showLoading("Calculating fee…");
  const fee_rate_sat_vb = selectedFeeRateSatVb ?? undefined;
  let preview: SendPreview;
  try {
    preview = await invoke<SendPreview>("preview_send", {
      req: { address, amount_sats, drain, fee_rate_sat_vb, selected_outpoints },
    });
  } catch (e) {
    setError(String(e));
    sending = false;
    updateBusyUi();
    hideLoading();
    return;
  } finally {
    hideLoading();
  }

  const amountLabel = formatAmountPlain(preview.amount_sats);
  const feeSource =
    selectedFeeRateSatVb != null
      ? customFeeActive
        ? "custom"
        : "explorer suggestion"
      : "estimated";
  const totalLeave = preview.amount_sats + preview.fee_sats;
  const reuseWarnings: string[] = [];
  try {
    const hint = await invoke<AddressReuseHint>("address_reuse_hint", { address });
    if (hint.reused) {
      reuseWarnings.push("This address appears to have been used before.");
    }
  } catch {
    /* soft-fail: do not block send */
  }
  if (isHighFee(preview.fee_sats, preview.amount_sats)) {
    reuseWarnings.push(
      "Network fee is at least half of the amount you are sending. You can still proceed if this is intentional.",
    );
  }
  let readLabel = () => "";
  const confirmed = await openConfirm({
    title: "Review transaction",
    message:
      "Check the destination carefully. Once broadcast, a Litecoin transaction cannot be recalled.",
    destination: address,
    warning: reuseWarnings.length ? reuseWarnings : undefined,
    rows: [
      ["Amount", amountLabel],
      ["Network fee", formatAmountPlain(preview.fee_sats)],
      ["Total leaving wallet", formatAmountPlain(totalLeave)],
      ...(drain ? ([["Emptying", "All transparent funds"]] as DetailRow[]) : []),
      ...(selected_outpoints?.length
        ? ([["Coins", `${selected_outpoints.length} selected`]] as DetailRow[])
        : []),
    ],
    detail: `Fee rate ${preview.fee_rate_sat_vb} sat/vB (${feeSource}).`,
    confirmLabel: drain ? "Send all now" : "Send now",
    afterDetail: (body) => {
      readLabel = appendTxLabelField(body, "");
    },
  });
  if (!confirmed) {
    sending = false;
    updateBusyUi();
    return;
  }
  const pendingLabel = readLabel();

  updateBusyUi();
  showLoading("Broadcasting transaction…");
  let result: SendResult;
  try {
    result = await invoke<SendResult>("send_ltc", {
      req: {
        address,
        amount_sats,
        fee_rate_sat_vb: preview.fee_rate_sat_vb,
        drain,
        selected_outpoints,
      },
    });
  } catch (e) {
    setError(String(e));
    return;
  } finally {
    hideLoading();
    sending = false;
    updateBusyUi();
  }

  lastTxid = result.txid;
  renderLastTxid();
  await persistTxLabel(result.txid, pendingLabel);
  el.sendAddress.value = "";
  el.sendAmount.value = "";
  el.sendDrain.checked = false;
  sendSelectedOutpoints.clear();
  el.coinControl.open = false;
  selectedFeeRateSatVb = null;
  customFeeActive = false;
  el.feeCustom.value = "";
  el.sendFeeHint.textContent = "Network fee is calculated automatically.";
  void refreshFeeLadder();
  updateBusyUi();

  void runSync({ quiet: false });
  await showResult({
    title: "Transaction sent",
    message: "Broadcast to the network. It stays pending until a block includes it.",
    rows: [
      ["To", address, true],
      ["Amount", amountLabel],
      ["Network fee", formatAmountPlain(result.fee_sats)],
      ["Transaction ID", result.txid, true],
    ],
    copy: { value: result.txid, label: "Copy ID", toast: "Transaction ID copied." },
    explorerTxid: result.txid,
  });
});

/** tcp:// to anything but the local machine sends wallet data in cleartext. */
function isPlaintextRemoteElectrum(url: string): boolean {
  if (!url.startsWith("tcp://")) return false;
  const host = url.slice("tcp://".length).replace(/:\d+$/, "").replace(/^\[|\]$/g, "");
  return !["localhost", "127.0.0.1", "::1"].includes(host);
}

el.btnSaveSettings.addEventListener("click", async () => {
  setError(null);
  const electrumUrl = el.settingsElectrum.value.trim();
  if (isPlaintextRemoteElectrum(electrumUrl)) {
    const proceed = await openConfirm({
      title: "Unencrypted connection",
      message:
        "This server uses tcp:// without TLS. Everyone on the network path can read your wallet addresses and transactions, and can tamper with the responses.",
      detail: "Use an ssl:// server unless this is your own node on a trusted network.",
      confirmLabel: "Save anyway",
      danger: true,
    });
    if (!proceed) return;
  }
  const autoLock = Math.max(0, Math.min(1440, Math.trunc(Number(el.settingsAutoLock.value) || 0)));
  try {
    await invoke("update_settings", {
      req: {
        electrum_url: electrumUrl,
        electrum_validate_domain: el.settingsValidateTls.checked,
        electrum_use_public_fallback: el.settingsPublicFallback.checked,
        auto_lock_minutes: autoLock,
        litecoin_rpc_url: el.settingsRpc.value.trim() || null,
        mweb_peers: el.settingsPeers.value
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
        explorer_base_url: el.settingsExplorer.value.trim() || "https://litview.space",
        show_fiat: el.settingsShowFiat.checked,
        use_explorer_fee_hints: el.settingsFeeHints.checked,
      },
    });
    autoLockMinutes = autoLock;
    showFiat = el.settingsShowFiat.checked;
    useExplorerFeeHints = el.settingsFeeHints.checked;
    if (!showFiat) {
      spotPriceUsd = null;
      renderFiat();
    } else {
      void refreshSpotPrice();
    }
    if (!useExplorerFeeHints) {
      if (!customFeeActive) selectedFeeRateSatVb = null;
      renderFeeChips(null);
      void refreshFeeEstimate();
    } else {
      void refreshFeeLadder();
    }
    updateSecurityChecklist();
    setStatus("Settings saved.", "success");
  } catch (e) {
    setError(String(e));
  }
});

el.btnLock.addEventListener("click", () => {
  void lockWallet();
});

el.btnPegin.addEventListener("click", async () => {
  if (syncing || sending) return;
  const drain = el.peginDrain.checked;
  const selected_outpoints = selectedOutpointsFor(peginCoinPanel);
  if (drain && selected_outpoints?.length) {
    setError("Turn off Choose coins or Move all — they cannot be used together.");
    return;
  }
  let amount_sats = 0;
  if (!drain) {
    const parsed = parseAmountToSats(el.peginAmount.value);
    if (parsed == null || parsed <= 0) {
      setError(amountError("peg-in", el.peginAmount.value));
      return;
    }
    amount_sats = parsed;
  }

  sending = true;
  setError(null);
  updateBusyUi();
  showLoading("Calculating fees…");
  let preview: PeginPreview;
  try {
    preview = await invoke<PeginPreview>("preview_pegin", {
      req: { amount_sats, drain, selected_outpoints },
    });
  } catch (e) {
    setError(String(e));
    sending = false;
    updateBusyUi();
    hideLoading();
    return;
  } finally {
    hideLoading();
  }

  const pegInFees = preview.mweb_fee_sats + preview.transparent_fee_sats;
  const warnings: string[] = [
    `Cannot spend privately until ~${MWEB_PEGIN_MATURITY_BLOCKS} confirmations.`,
  ];
  if (isHighFee(pegInFees, preview.amount_sats)) {
    warnings.push(
      "Combined fees are at least half of the amount you are moving. You can still proceed if this is intentional.",
    );
  }
  let readLabel = () => "";
  const confirmed = await openConfirm({
    title: "Move funds to private",
    message:
      "A peg-in moves transparent funds onto the MWEB side of the chain, where balances and amounts are confidential. The public broadcast pays a miner fee; MWEB credits pay a private network fee.",
    warning: warnings,
    rows: [
      ["Amount", formatAmountPlain(preview.amount_sats)],
      ["Private credit", formatAmountPlain(preview.private_credit_sats)],
      ["Private network fee (MWEB)", formatAmountPlain(preview.mweb_fee_sats)],
      ["Miner fee (public chain)", formatAmountPlain(preview.transparent_fee_sats)],
      ["Leaves transparent", formatAmountPlain(preview.total_from_transparent_sats)],
      ...(selected_outpoints?.length
        ? ([["Coins", `${selected_outpoints.length} selected`]] as DetailRow[])
        : []),
    ],
    afterDetail: (body) => {
      const details = document.createElement("details");
      details.className = "fee-why";
      const summary = document.createElement("summary");
      summary.textContent = "Why two fees?";
      const p = document.createElement("p");
      p.textContent =
        "The miner fee pays Litecoin miners to confirm the public peg-in transaction. The private network fee is burned on MWEB when your coins are credited. Both are required for a peg-in.";
      details.append(summary, p);
      body.appendChild(details);
      readLabel = appendTxLabelField(body, "");
    },
    confirmLabel: drain ? "Move all to private" : "Move to private",
  });
  if (!confirmed) {
    sending = false;
    updateBusyUi();
    return;
  }
  const pendingLabel = readLabel();

  updateBusyUi();
  showLoading("Broadcasting peg-in…");
  let result: { txid: string; maturity_blocks: number; fee_sats: number };
  try {
    result = await invoke("pegin_ltc", {
      req: {
        amount_sats: preview.amount_sats,
        mweb_fee_sats: preview.mweb_fee_sats,
        transparent_fee_sats: preview.transparent_fee_sats,
        selected_outpoints,
      },
    });
  } catch (e) {
    setError(String(e));
    return;
  } finally {
    hideLoading();
    sending = false;
    updateBusyUi();
  }

  lastTxid = result.txid;
  renderLastTxid();
  await persistTxLabel(result.txid, pendingLabel);
  el.peginAmount.value = "";
  el.peginDrain.checked = false;
  peginSelectedOutpoints.clear();
  el.peginCoinControl.open = false;

  void runSync({ quiet: false });
  const pegInAction = await showResult({
    title: "Peg-in sent",
    message: "Broadcast to the network. The funds become spendable on the MWEB side once mature.",
    rows: [
      ["Private credit", formatAmountPlain(preview.private_credit_sats)],
      ["Total fees", formatAmountPlain(result.fee_sats)],
      ["Matures in", `${result.maturity_blocks} blocks`],
      ["Transaction ID", result.txid, true],
    ],
    copy: { value: result.txid, label: "Copy ID", toast: "Transaction ID copied." },
    explorerTxid: result.txid,
    extraActions: [{ id: "history", label: "View in History", kind: "secondary" }],
  });
  if (pegInAction === "history") setView("history");
});

el.btnMwebSend.addEventListener("click", async () => {
  if (syncing || sending) return;
  const address = el.mwebSendAddress.value.trim();
  if (!address) {
    setError("Enter an MWEB send address (ltcmweb1…).");
    return;
  }
  const drain = el.mwebSendDrain.checked;
  let amount_sats = 0;
  if (!drain) {
    const parsed = parseAmountToSats(el.mwebSendAmount.value);
    if (parsed == null) {
      setError(amountError("MWEB send", el.mwebSendAmount.value));
      return;
    }
    amount_sats = parsed;
  }

  sending = true;
  setError(null);
  updateBusyUi();
  showLoading("Calculating fee…");
  let preview: MwebSendPreview;
  try {
    preview = await invoke<MwebSendPreview>("preview_mweb_send", {
      req: { address, amount_sats, drain },
    });
  } catch (e) {
    setError(String(e));
    sending = false;
    updateBusyUi();
    hideLoading();
    return;
  } finally {
    hideLoading();
  }

  let readLabel = () => "";
  const confirmed = await openConfirm({
    title: "Review private send",
    message:
      "Check the stealth address carefully. Once broadcast, a private transfer cannot be recalled.",
    destination: address,
    warning: isHighFee(preview.fee_sats, preview.amount_sats)
      ? "Network fee is at least half of the amount you are sending. You can still proceed if this is intentional."
      : undefined,
    rows: [
      ["Amount", formatAmountPlain(preview.amount_sats)],
      ["Network fee", formatAmountPlain(preview.fee_sats)],
      ["Total leaving private", formatAmountPlain(preview.amount_sats + preview.fee_sats)],
    ],
    confirmLabel: drain ? "Send all private" : "Send private",
    afterDetail: (body) => {
      readLabel = appendTxLabelField(body, "");
    },
  });
  if (!confirmed) {
    sending = false;
    updateBusyUi();
    return;
  }
  const pendingLabel = readLabel();

  updateBusyUi();
  showLoading("Broadcasting private send…");
  let result: { wtxid: string; fee_sats: number };
  try {
    result = await invoke("mweb_send_ltc", {
      req: {
        address,
        amount_sats: preview.amount_sats,
        fee_sats: preview.fee_sats,
      },
    });
  } catch (e) {
    setError(String(e));
    return;
  } finally {
    hideLoading();
    sending = false;
    updateBusyUi();
  }

  await persistTxLabel(result.wtxid, pendingLabel);
  el.mwebSendAddress.value = "";
  el.mwebSendAmount.value = "";
  el.mwebSendDrain.checked = false;
  await refreshCombined();
  await refreshHistory();
  await showResult({
    title: "Private send sent",
    message:
      "Broadcast over the MWEB network. Private transfers are not listed on public explorers — that is expected. Keep the Kernel ID as your reference.",
    rows: [
      ["To", address, true],
      ["Amount", formatAmountPlain(preview.amount_sats)],
      ["Network fee", formatAmountPlain(result.fee_sats)],
      ["Kernel ID", result.wtxid, true],
    ],
    copy: { value: result.wtxid, label: "Copy ID", toast: "Kernel ID copied." },
  });
});

el.btnPegout.addEventListener("click", async () => {
  if (syncing || sending) return;
  const drain = el.pegoutDrain.checked;
  let amount_sats = 0;
  if (!drain) {
    const parsed = parseAmountToSats(el.pegoutAmount.value);
    if (parsed == null) {
      setError(amountError("swap", el.pegoutAmount.value));
      return;
    }
    amount_sats = parsed;
  }

  sending = true;
  setError(null);
  updateBusyUi();
  showLoading("Preparing swap…");
  // Funds return to the wallet itself: a fresh transparent address per peg-out
  // keeps the public history harder to link.
  let address: string;
  let preview: PegoutPreview;
  try {
    address = await invoke<string>("get_receive_address");
    preview = await invoke<PegoutPreview>("preview_pegout", {
      req: { address, amount_sats, drain },
    });
  } catch (e) {
    setError(String(e));
    sending = false;
    updateBusyUi();
    hideLoading();
    return;
  } finally {
    hideLoading();
  }

  let readLabel = () => "";
  const confirmed = await openConfirm({
    title: "Move funds to public",
    message:
      "This returns private funds to a fresh public address of your own, where the amount becomes publicly visible.",
    destination: address,
    warning: isHighFee(preview.fee_sats, preview.amount_sats)
      ? "Network fee is at least half of the amount you are moving. You can still proceed if this is intentional."
      : undefined,
    rows: [
      ["Amount", formatAmountPlain(preview.amount_sats)],
      ["Network fee", formatAmountPlain(preview.fee_sats)],
      ["Total leaving private", formatAmountPlain(preview.amount_sats + preview.fee_sats)],
    ],
    detail: `Destination dust floor is ${preview.dust_sats.toLocaleString("en-US")} litoshis.`,
    confirmLabel: drain ? "Move all to public" : "Move to public",
    afterDetail: (body) => {
      readLabel = appendTxLabelField(body, "");
    },
  });
  if (!confirmed) {
    sending = false;
    updateBusyUi();
    return;
  }
  const pendingLabel = readLabel();

  updateBusyUi();
  showLoading("Broadcasting swap…");
  let result: { wtxid: string; fee_sats: number };
  try {
    result = await invoke("pegout_ltc", {
      req: {
        address,
        amount_sats: preview.amount_sats,
        fee_sats: preview.fee_sats,
      },
    });
  } catch (e) {
    setError(String(e));
    return;
  } finally {
    hideLoading();
    sending = false;
    updateBusyUi();
  }

  await persistTxLabel(result.wtxid, pendingLabel);
  el.pegoutAmount.value = "";
  el.pegoutDrain.checked = false;

  void runSync({ quiet: false });
  await showResult({
    title: "Swap to public sent",
    message:
      "Broadcast to the network. The public funds arrive once the swap confirms. Private transfers are not listed on public explorers — that is expected. Keep the Kernel ID as your reference.",
    rows: [
      ["To (your new public address)", address, true],
      ["Amount", formatAmountPlain(preview.amount_sats)],
      ["Network fee", formatAmountPlain(result.fee_sats)],
      ["Kernel ID", result.wtxid, true],
    ],
    copy: { value: result.wtxid, label: "Copy ID", toast: "Kernel ID copied." },
  });
});

displayUnit = readDisplayUnit();
hideBalances = readHideBalances();
syncAmountFieldLabels();
void boot();
