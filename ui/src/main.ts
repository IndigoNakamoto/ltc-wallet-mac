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

const DUST_LITOSHIS = 2940;
const AUTO_SYNC_MS = 60_000;
const QR_CSS_SIZE = 176;
const RECENT_TX_COUNT = 6;

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
  ready: document.querySelector<HTMLElement>("#ready")!,
  mnemonicText: document.querySelector<HTMLElement>("#mnemonic-text")!,
  viewTitle: document.querySelector<HTMLElement>("#view-title")!,
  networkBadge: document.querySelector<HTMLElement>("#network-badge")!,
  syncDot: document.querySelector<HTMLElement>("#sync-dot")!,
  syncLabel: document.querySelector<HTMLElement>("#sync-label")!,
  balanceTotal: document.querySelector<HTMLElement>("#balance-total")!,
  balanceSats: document.querySelector<HTMLElement>("#balance-sats")!,
  balanceConfirmed: document.querySelector<HTMLElement>("#balance-confirmed")!,
  balanceMweb: document.querySelector<HTMLElement>("#balance-mweb")!,
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
  restoreMnemonic: document.querySelector<HTMLTextAreaElement>("#restore-mnemonic")!,
  createRestoreHint: document.querySelector<HTMLElement>("#create-restore-hint")!,
  restoreMwebScheme: document.querySelector<HTMLSelectElement>("#restore-mweb-scheme")!,
  restoreAezeedPass: document.querySelector<HTMLInputElement>("#restore-aezeed-pass")!,
  restorePassphrase: document.querySelector<HTMLInputElement>("#restore-passphrase")!,
  restorePassphrase2: document.querySelector<HTMLInputElement>("#restore-passphrase2")!,
  onboardPassphrase: document.querySelector<HTMLInputElement>("#onboard-passphrase")!,
  onboardPassphrase2: document.querySelector<HTMLInputElement>("#onboard-passphrase2")!,
  unlockPassphrase: document.querySelector<HTMLInputElement>("#unlock-passphrase")!,
  migratePassphrase: document.querySelector<HTMLInputElement>("#migrate-passphrase")!,
  migratePassphrase2: document.querySelector<HTMLInputElement>("#migrate-passphrase2")!,
  sendForm: document.querySelector<HTMLFormElement>("#send-form")!,
  sendAddress: document.querySelector<HTMLInputElement>("#send-address")!,
  sendAmount: document.querySelector<HTMLInputElement>("#send-amount")!,
  sendDrain: document.querySelector<HTMLInputElement>("#send-drain")!,
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
  btnMnemonicDone: document.querySelector<HTMLButtonElement>("#btn-mnemonic-done")!,
  btnSync: document.querySelector<HTMLButtonElement>("#btn-sync")!,
  btnAddress: document.querySelector<HTMLButtonElement>("#btn-address")!,
  btnCopy: document.querySelector<HTMLButtonElement>("#btn-copy")!,
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
let autoSyncTimer: number | null = null;
let mwebProgressTimer: number | null = null;
let statusTimer: number | null = null;

function formatLtc(sats: number): string {
  const whole = Math.trunc(sats / 100_000_000);
  const frac = Math.abs(sats % 100_000_000)
    .toString()
    .padStart(8, "0");
  const sign = sats < 0 ? "-" : "";
  return `${sign}${whole}.${frac} LTC`;
}

function formatLitoshis(sats: number): string {
  return `(${sats.toLocaleString("en-US")} litoshis)`;
}

function formatMs(ms: number): string {
  return ms >= 1_000 ? `${(ms / 1_000).toFixed(1)}s` : `${ms}ms`;
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

function amountError(field: string, rawValue: string): string {
  const shown = rawValue.trim();
  if (!shown) return `Enter a ${field} amount in LTC.`;
  if (shown.includes(",")) {
    return `Invalid ${field} amount "${shown}" — use a dot as the decimal separator (e.g. 0.009), no commas.`;
  }
  return `Invalid ${field} amount "${shown}" — enter LTC like 0.009 (max 8 decimal places).`;
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
}

for (const view of VIEWS) {
  views[view].nav.addEventListener("click", () => setView(view));
}

for (const card of CARDS) {
  cards[card].nav.addEventListener("click", () => setCard(card));
}

el.btnSeeAll.addEventListener("click", () => setView("history"));

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

async function openConfirm(opts: {
  title: string;
  message: string;
  rows?: DetailRow[];
  detail?: string;
  confirmLabel?: string;
  danger?: boolean;
}): Promise<boolean> {
  const result = await openModal({
    title: opts.title,
    build: (body) => {
      appendParagraph(body, opts.message, "lede");
      if (opts.rows?.length) body.appendChild(buildDetailList(opts.rows));
      if (opts.detail) appendParagraph(body, opts.detail, "hint");
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
}) {
  const actions: ModalAction[] = [];
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
  if (result === "copy" && opts.copy) await copyText(opts.copy.value, opts.copy.toast);
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
  el.btnSend.disabled = busy;
  // A filled restore field means the user intends to restore; block Create so
  // the primary button can't silently generate a fresh wallet instead.
  const restorePending = el.restoreMnemonic.value.trim().length > 0;
  el.btnCreate.disabled = busy || restorePending;
  el.createRestoreHint.hidden = !restorePending;
  el.btnRestore.disabled = busy;
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

function paymentUri(address: string): string {
  return address.startsWith("ltcmweb") || address.startsWith("tmweb")
    ? address
    : `litecoin:${address}`;
}

async function renderQr(canvas: HTMLCanvasElement, address: string) {
  const ctx = canvas.getContext("2d");
  if (!address) {
    ctx?.clearRect(0, 0, canvas.width, canvas.height);
    return;
  }
  // Render at device resolution, then pin the CSS box so it stays crisp on Retina.
  const dpr = Math.min(3, Math.max(1, Math.round(window.devicePixelRatio || 1)));
  try {
    await QRCode.toCanvas(canvas, paymentUri(address), {
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

function renderMnemonic(mnemonic: string) {
  el.mnemonicText.textContent = "";
  const words = mnemonic.trim().split(/\s+/).filter(Boolean);
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

function renderSummary(s: WalletSummary) {
  el.networkBadge.textContent = s.network;
  el.balanceTotal.classList.remove("skeleton");
  el.balanceTotal.textContent = formatLtc(s.total_sats);
  el.balanceSats.textContent = formatLitoshis(s.total_sats);
  el.balanceConfirmed.textContent = formatLtc(s.confirmed_sats);
  el.balanceTip.textContent = s.tip_height.toLocaleString("en-US");
  el.address.textContent = s.receive_address;
  void renderQr(el.receiveQr, s.receive_address);

  // Spendable public balance shown on the Public toggle segments.
  const publicBalance = formatLtc(s.confirmed_sats);
  el.sendBalancePublic.textContent = publicBalance;
  el.receiveBalancePublic.textContent = publicBalance;
  el.swapBalancePublic.textContent = publicBalance;

  const pendingParts: string[] = [];
  if (s.trusted_pending_sats > 0) {
    pendingParts.push(`trusted pending ${formatLtc(s.trusted_pending_sats)}`);
  }
  if (s.untrusted_pending_sats > 0) {
    pendingParts.push(`untrusted pending ${formatLtc(s.untrusted_pending_sats)}`);
  }
  if (s.immature_sats > 0) {
    pendingParts.push(`immature ${formatLtc(s.immature_sats)}`);
  }
  if (pendingParts.length > 0) {
    el.balancePending.hidden = false;
    el.balancePending.textContent = pendingParts.join(" · ");
  } else {
    el.balancePending.hidden = true;
    el.balancePending.textContent = "";
  }
}

function renderCombined(c: CombinedSummary) {
  renderSummary(c.transparent);
  // Hero "Total balance" is wallet-wide: transparent + MWEB.
  const grandTotal = c.transparent.total_sats + c.mweb_total_sats;
  el.balanceTotal.textContent = formatLtc(grandTotal);
  el.balanceSats.textContent = formatLitoshis(grandTotal);
  setMwebVisible(true);
  let mwebText = formatLtc(c.mweb_total_sats);
  if (c.mweb_immature_sats > 0) {
    mwebText += ` · maturing ${formatLtc(c.mweb_immature_sats)}`;
  }
  if (c.mweb_stale) {
    mwebText += c.mweb_synced_height != null
      ? ` · stale as of height ${c.mweb_synced_height}`
      : " · stale";
  }
  el.balanceMweb.textContent = mwebText;
  el.mwebStatus.hidden = false;
  el.mwebStatus.textContent = c.mweb_status;
  if (c.mweb_receive_address) {
    el.mwebAddress.textContent = c.mweb_receive_address;
    void renderQr(el.mwebQr, c.mweb_receive_address);
  }

  // Spendable private balance shown on the Private toggle segments.
  let privateBalance = formatLtc(c.mweb_confirmed_sats);
  if (c.mweb_immature_sats > 0) {
    privateBalance += ` · maturing ${formatLtc(c.mweb_immature_sats)}`;
  }
  el.sendBalancePrivate.textContent = privateBalance;
  el.receiveBalancePrivate.textContent = privateBalance;
  el.swapBalancePrivate.textContent = privateBalance;
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

function formatSignedLtc(tx: TxRecord): string {
  return `${txDirection(tx) === "in" ? "+" : "−"}${formatLtc(Math.abs(tx.net_sats))}`;
}

function buildTxRow(tx: TxRecord, index: number): HTMLLIElement {
  const dir = txDirection(tx);

  const icon = document.createElement("span");
  icon.className = `tx-icon ${dir}`;
  icon.innerHTML = dir === "in" ? SVG_ARROW_IN : SVG_ARROW_OUT;

  const amount = document.createElement("span");
  amount.className = dir === "in" ? "tx-amt in" : "tx-amt";
  amount.textContent = formatSignedLtc(tx);

  const meta = document.createElement("span");
  meta.className = "tx-meta";
  meta.textContent = [TX_KIND_LABELS[tx.kind], formatTxTime(tx.timestamp)]
    .filter(Boolean)
    .join(" · ");
  meta.hidden = meta.textContent === "";

  const main = document.createElement("div");
  main.className = "tx-main";
  main.append(amount, meta);

  const pill = document.createElement("span");
  pill.className = tx.confirmations === 0 ? "pill pending" : "pill";
  pill.textContent =
    tx.confirmations === 0 ? "pending" : `${tx.confirmations.toLocaleString("en-US")} conf`;

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
  li.setAttribute("role", "button");
  li.setAttribute("aria-label", `Transaction ${formatSignedLtc(tx)} — show details`);
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
  el.txList.textContent = "";
  el.txListRecent.textContent = "";
  el.txEmpty.hidden = txs.length > 0;
  el.txEmptyRecent.hidden = txs.length > 0;
  el.btnSeeAll.hidden = txs.length <= RECENT_TX_COUNT;
  txs.forEach((tx, index) => el.txList.appendChild(buildTxRow(tx, index)));
  txs
    .slice(0, RECENT_TX_COUNT)
    .forEach((tx, index) => el.txListRecent.appendChild(buildTxRow(tx, index)));
}

/** Detail panel for one transaction; prev/next walk the cached list in place. */
async function openTxDetail(index: number) {
  let at = index;
  for (;;) {
    const tx = txRecords[at];
    if (!tx) return;

    const rows: DetailRow[] = [
      [
        "Status",
        tx.confirmations === 0
          ? "Pending — not in a block yet"
          : `${tx.confirmations.toLocaleString("en-US")} confirmations`,
      ],
      ["Type", TX_KIND_LABELS[tx.kind] || (txDirection(tx) === "in" ? "received" : "sent")],
      ["Time", formatTxTimeLong(tx.timestamp)],
    ];
    if (tx.height != null) rows.push(["Block height", tx.height.toLocaleString("en-US")]);
    if (tx.received_sats > 0) rows.push(["Received", formatLtc(tx.received_sats)]);
    if (tx.sent_sats > 0) rows.push(["Sent", formatLtc(tx.sent_sats)]);
    rows.push([
      "Fee",
      tx.fee_sats != null ? `${tx.fee_sats.toLocaleString("en-US")} litoshis` : "unknown",
    ]);
    rows.push(["Transaction ID", tx.txid, true]);

    const hasPrev = at > 0;
    const hasNext = at < txRecords.length - 1;
    const actions: ModalAction[] = [];
    if (hasPrev) actions.push({ id: "prev", label: "‹ Prev", kind: "ghost", nav: true });
    if (hasNext) actions.push({ id: "next", label: "Next ›", kind: "ghost", nav: true });
    actions.push({ id: "copy", label: "Copy ID", kind: "secondary" });
    actions.push({ id: "close", label: "Close", kind: "primary" });

    const dir = txDirection(tx);
    const result = await openModal({
      title: `Transaction ${at + 1} of ${txRecords.length}`,
      wide: true,
      build: (body) => {
        const amount = document.createElement("p");
        amount.className = dir === "in" ? "detail-amount in" : "detail-amount";
        amount.textContent = formatSignedLtc(tx);
        body.append(amount, buildDetailList(rows));
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

    if (result === "prev") {
      at -= 1;
      continue;
    }
    if (result === "next") {
      at += 1;
      continue;
    }
    if (result === "copy") await copyText(tx.txid, "Transaction ID copied.");
    return;
  }
}

async function refreshHistory() {
  try {
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

async function loadSettings() {
  try {
    const s = await invoke<WalletSettings>("get_settings");
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
  if (currentPhase !== "ready" || autoLockMinutes <= 0 || syncing || sending) return;
  if (Date.now() - lastActivityTs < autoLockMinutes * 60_000) return;
  void (async () => {
    try {
      await invoke("lock_wallet");
    } catch {
      return;
    }
    stopAutoSync();
    setPhase("unlock");
    setStatus("Wallet locked after inactivity.");
  })();
}, 30_000);

function requireMatchingPassphrases(a: string, b: string): string | null {
  if (!a) return "Passphrase is required.";
  if (a !== b) return "Passphrases do not match.";
  return null;
}

async function boot() {
  setPhase("boot");
  setError(null);
  applyTheme(readThemePref());
  updateBusyUi();
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
  const s = await invoke<WalletSummary>("load_wallet");
  renderSummary(s);
  setPhase("ready");
  setView("balance");
  await refreshCombined();
  await refreshHistory();
  await loadSettings();
  void runSync({ quiet: false });
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
  renderLastTxid();
  renderHistory([]);
  setPhase("onboarding");
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
  const err = requireMatchingPassphrases(
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
  try {
    const result = await invoke<SyncResult>("sync_wallet");
    renderSummary(result.summary);
    await refreshCombined();
    await refreshHistory();
    syncState = "ok";
    const timing =
      result.mweb_ms > 0
        ? `${formatMs(result.electrum_ms)} + ${formatMs(result.mweb_ms)} MWEB`
        : formatMs(result.electrum_ms);
    const newTxs =
      result.new_txs > 0
        ? ` · ${result.new_txs} new transaction${result.new_txs === 1 ? "" : "s"}`
        : "";
    if (result.electrum_server) {
      el.settingsActiveServer.hidden = false;
      el.settingsActiveServer.textContent = `Last sync used: ${result.electrum_server}`;
    }
    if (result.warnings?.length) {
      // Cross-check findings outrank the feel-good sync message.
      for (const warning of result.warnings) console.warn(warning);
      setStatus(result.warnings[0], "error");
    } else {
      setStatus(`Synced in ${timing}${newTxs}`, "success");
    }
    return true;
  } catch (e) {
    syncState = "error";
    if (opts.quiet) setStatus(`Auto-sync failed: ${e}`, "error");
    else setError(String(e));
    return false;
  } finally {
    stopMwebProgressPolling();
    syncing = false;
    updateBusyUi();
  }
}

el.restoreMnemonic.addEventListener("input", updateBusyUi);

el.btnCreate.addEventListener("click", async () => {
  if (syncing || sending) return;
  if (el.restoreMnemonic.value.trim()) {
    setError(
      "You entered a recovery phrase — click “Restore wallet” below, or clear the phrase to create a new wallet.",
    );
    return;
  }
  const err = requireMatchingPassphrases(
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
    renderMnemonic(resp.mnemonic);
    renderSummary(resp.summary);
    el.onboardPassphrase.value = "";
    el.onboardPassphrase2.value = "";
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
  const err = requireMatchingPassphrases(
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
    const s = await invoke<WalletSummary>("restore_wallet", {
      req: {
        mnemonic,
        network: "mainnet",
        mweb_scheme: el.restoreMwebScheme.value as MwebScheme,
        aezeed_passphrase: aezeedPass ? aezeedPass : null,
      },
      passphrase,
    });
    renderSummary(s);
    el.restorePassphrase.value = "";
    el.restorePassphrase2.value = "";
    el.restoreMnemonic.value = "";
    el.restoreAezeedPass.value = "";
    setPhase("ready");
    setView("balance");
    syncing = false;
    updateBusyUi();
    await loadSettings();
    void runSync({ quiet: false });
  } catch (e) {
    setError(String(e));
    syncing = false;
    updateBusyUi();
  } finally {
    hideLoading();
  }
});

el.btnMnemonicDone.addEventListener("click", () => {
  el.mnemonicText.textContent = "";
  setPhase("ready");
  setView("balance");
  void loadSettings();
  void runSync({ quiet: false });
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
    await renderQr(el.receiveQr, address);
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

el.sendDrain.addEventListener("change", () => {
  updateBusyUi();
});
el.peginDrain.addEventListener("change", () => {
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

  const address = el.sendAddress.value.trim();
  const drain = el.sendDrain.checked;

  if (!address) {
    setError("Enter a destination address.");
    return;
  }

  let amount_sats = 0;
  if (!drain) {
    const parsed = parseLtcToSats(el.sendAmount.value);
    if (parsed == null) {
      setError("Amount must be a valid LTC value (max 8 decimals).");
      return;
    }
    if (parsed < DUST_LITOSHIS) {
      setError(`Amount must be ≥ ${DUST_LITOSHIS} litoshis (Litecoin dust limit for ltc1).`);
      return;
    }
    amount_sats = parsed;
  }

  sending = true;
  setError(null);
  updateBusyUi();
  showLoading("Calculating fee…");
  let preview: SendPreview;
  try {
    preview = await invoke<SendPreview>("preview_send", {
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

  const amountLabel = formatLtc(preview.amount_sats);
  const confirmed = await openConfirm({
    title: "Review transaction",
    message:
      "Check the destination carefully. Once broadcast, a Litecoin transaction cannot be recalled.",
    rows: [
      ["To", address, true],
      ["Amount", amountLabel],
      ["Network fee", formatLtc(preview.fee_sats)],
      ...(drain ? ([["Emptying", "All transparent funds"]] as DetailRow[]) : []),
    ],
    detail: `Fee rate ${preview.fee_rate_sat_vb} sat/vB (estimated).`,
    confirmLabel: drain ? "Send all now" : "Send now",
  });
  if (!confirmed) {
    sending = false;
    updateBusyUi();
    return;
  }

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
  el.sendAddress.value = "";
  el.sendAmount.value = "";
  el.sendDrain.checked = false;
  updateBusyUi();

  void runSync({ quiet: false });
  await showResult({
    title: "Transaction sent",
    message: "Broadcast to the network. It stays pending until a block includes it.",
    rows: [
      ["To", address, true],
      ["Amount", amountLabel],
      ["Network fee", formatLtc(result.fee_sats)],
      ["Transaction ID", result.txid, true],
    ],
    copy: { value: result.txid, label: "Copy ID", toast: "Transaction ID copied." },
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
      },
    });
    autoLockMinutes = autoLock;
    setStatus("Settings saved.", "success");
  } catch (e) {
    setError(String(e));
  }
});

el.btnLock.addEventListener("click", async () => {
  await invoke("lock_wallet");
  stopAutoSync();
  setPhase("unlock");
  setStatus("Wallet locked.");
});

el.btnPegin.addEventListener("click", async () => {
  if (syncing || sending) return;
  const drain = el.peginDrain.checked;
  let amount_sats = 0;
  if (!drain) {
    const parsed = parseLtcToSats(el.peginAmount.value);
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
      req: { amount_sats, drain },
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

  const confirmed = await openConfirm({
    title: "Move funds to private",
    message:
      "A peg-in moves transparent funds onto the MWEB side of the chain, where balances and amounts are confidential.",
    rows: [
      ["Private credit", formatLtc(preview.private_credit_sats)],
      ["Private network fee", formatLtc(preview.mweb_fee_sats)],
      ["Miner fee", formatLtc(preview.transparent_fee_sats)],
      ["Leaves transparent", formatLtc(preview.total_from_transparent_sats)],
    ],
    detail: "Pegged-in coins mature after 6 blocks before they can be spent privately.",
    confirmLabel: drain ? "Move all to private" : "Move to private",
  });
  if (!confirmed) {
    sending = false;
    updateBusyUi();
    return;
  }

  updateBusyUi();
  showLoading("Broadcasting peg-in…");
  let result: { txid: string; maturity_blocks: number; fee_sats: number };
  try {
    result = await invoke("pegin_ltc", {
      req: {
        amount_sats: preview.amount_sats,
        mweb_fee_sats: preview.mweb_fee_sats,
        transparent_fee_sats: preview.transparent_fee_sats,
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
  el.peginAmount.value = "";
  el.peginDrain.checked = false;

  void runSync({ quiet: false });
  await showResult({
    title: "Peg-in sent",
    message: "Broadcast to the network. The funds become spendable on the MWEB side once mature.",
    rows: [
      ["Private credit", formatLtc(preview.private_credit_sats)],
      ["Total fees", formatLtc(result.fee_sats)],
      ["Matures in", `${result.maturity_blocks} blocks`],
      ["Transaction ID", result.txid, true],
    ],
    copy: { value: result.txid, label: "Copy ID", toast: "Transaction ID copied." },
  });
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
    const parsed = parseLtcToSats(el.mwebSendAmount.value);
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

  const confirmed = await openConfirm({
    title: "Review private send",
    message:
      "Check the stealth address carefully. Once broadcast, a private transfer cannot be recalled.",
    rows: [
      ["To", address, true],
      ["Amount", formatLtc(preview.amount_sats)],
      ["Network fee", formatLtc(preview.fee_sats)],
    ],
    confirmLabel: drain ? "Send all private" : "Send private",
  });
  if (!confirmed) {
    sending = false;
    updateBusyUi();
    return;
  }

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

  el.mwebSendAddress.value = "";
  el.mwebSendAmount.value = "";
  el.mwebSendDrain.checked = false;
  await refreshCombined();
  await refreshHistory();
  await showResult({
    title: "Private send sent",
    message: "Broadcast over the MWEB network. It stays pending until a block includes it.",
    rows: [
      ["To", address, true],
      ["Amount", formatLtc(preview.amount_sats)],
      ["Network fee", formatLtc(result.fee_sats)],
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
    const parsed = parseLtcToSats(el.pegoutAmount.value);
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

  const confirmed = await openConfirm({
    title: "Move funds to public",
    message:
      "This returns private funds to a fresh public address of your own, where the amount becomes publicly visible.",
    rows: [
      ["To (your new public address)", address, true],
      ["Amount", formatLtc(preview.amount_sats)],
      ["Network fee", formatLtc(preview.fee_sats)],
    ],
    detail: `Destination dust floor is ${preview.dust_sats.toLocaleString("en-US")} litoshis.`,
    confirmLabel: drain ? "Move all to public" : "Move to public",
  });
  if (!confirmed) {
    sending = false;
    updateBusyUi();
    return;
  }

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

  el.pegoutAmount.value = "";
  el.pegoutDrain.checked = false;

  void runSync({ quiet: false });
  await showResult({
    title: "Swap to public sent",
    message: "Broadcast to the network. The public funds arrive once the swap confirms.",
    rows: [
      ["To (your new public address)", address, true],
      ["Amount", formatLtc(preview.amount_sats)],
      ["Network fee", formatLtc(result.fee_sats)],
      ["Kernel ID", result.wtxid, true],
    ],
    copy: { value: result.wtxid, label: "Copy ID", toast: "Kernel ID copied." },
  });
});

void boot();
