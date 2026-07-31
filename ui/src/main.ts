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
};

type SendResult = {
  txid: string;
  fee_sats: number;
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

type WalletSettings = {
  electrum_url: string;
  litecoin_rpc_url: string | null;
  mweb_peers: string[];
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

const VIEWS = ["balance", "receive", "send", "private", "history", "settings"] as const;
type View = (typeof VIEWS)[number];

const VIEW_TITLES: Record<View, string> = {
  balance: "Balance",
  receive: "Receive",
  send: "Send",
  private: "Private",
  history: "History",
  settings: "Settings",
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
  mwebReceive: document.querySelector<HTMLElement>("#mweb-receive")!,
  mwebQr: document.querySelector<HTMLCanvasElement>("#mweb-qr")!,
  mwebAddress: document.querySelector<HTMLElement>("#mweb-address")!,
  mwebActions: document.querySelector<HTMLElement>("#mweb-actions")!,
  toast: document.querySelector<HTMLElement>("#toast")!,
  status: document.querySelector<HTMLElement>("#status")!,
  btnToastClose: document.querySelector<HTMLButtonElement>("#btn-toast-close")!,
  btnTheme: document.querySelector<HTMLButtonElement>("#btn-theme")!,
  lastTxid: document.querySelector<HTMLElement>("#last-txid")!,
  txList: document.querySelector<HTMLUListElement>("#tx-list")!,
  txEmpty: document.querySelector<HTMLElement>("#tx-empty")!,
  restoreMnemonic: document.querySelector<HTMLTextAreaElement>("#restore-mnemonic")!,
  onboardPassphrase: document.querySelector<HTMLInputElement>("#onboard-passphrase")!,
  onboardPassphrase2: document.querySelector<HTMLInputElement>("#onboard-passphrase2")!,
  unlockPassphrase: document.querySelector<HTMLInputElement>("#unlock-passphrase")!,
  migratePassphrase: document.querySelector<HTMLInputElement>("#migrate-passphrase")!,
  migratePassphrase2: document.querySelector<HTMLInputElement>("#migrate-passphrase2")!,
  sendForm: document.querySelector<HTMLFormElement>("#send-form")!,
  sendAddress: document.querySelector<HTMLInputElement>("#send-address")!,
  sendAmount: document.querySelector<HTMLInputElement>("#send-amount")!,
  sendDrain: document.querySelector<HTMLInputElement>("#send-drain")!,
  sendFeeRate: document.querySelector<HTMLInputElement>("#send-fee-rate")!,
  settingsElectrum: document.querySelector<HTMLInputElement>("#settings-electrum")!,
  settingsRpc: document.querySelector<HTMLInputElement>("#settings-rpc")!,
  settingsPeers: document.querySelector<HTMLInputElement>("#settings-peers")!,
  peginAmount: document.querySelector<HTMLInputElement>("#pegin-amount")!,
  mwebSendAddress: document.querySelector<HTMLInputElement>("#mweb-send-address")!,
  mwebSendAmount: document.querySelector<HTMLInputElement>("#mweb-send-amount")!,
  pegoutAddress: document.querySelector<HTMLInputElement>("#pegout-address")!,
  pegoutAmount: document.querySelector<HTMLInputElement>("#pegout-amount")!,
  btnCreate: document.querySelector<HTMLButtonElement>("#btn-create")!,
  btnRestore: document.querySelector<HTMLButtonElement>("#btn-restore")!,
  btnMnemonicDone: document.querySelector<HTMLButtonElement>("#btn-mnemonic-done")!,
  btnSync: document.querySelector<HTMLButtonElement>("#btn-sync")!,
  btnAddress: document.querySelector<HTMLButtonElement>("#btn-address")!,
  btnCopy: document.querySelector<HTMLButtonElement>("#btn-copy")!,
  btnCopyMweb: document.querySelector<HTMLButtonElement>("#btn-copy-mweb")!,
  btnResyncMweb: document.querySelector<HTMLButtonElement>("#btn-resync-mweb")!,
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

let syncing = false;
let sending = false;
let currentPhase: Phase = "boot";
let currentView: View = "balance";
let syncState: SyncState = "idle";
let lastTxid: string | null = null;
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

function setView(next: View) {
  currentView = next;
  el.viewTitle.textContent = VIEW_TITLES[next];
  for (const view of VIEWS) {
    const { nav, pane } = views[view];
    const active = view === next;
    pane.hidden = !active;
    nav.setAttribute("aria-selected", String(active));
  }
}

for (const view of VIEWS) {
  views[view].nav.addEventListener("click", () => setView(view));
}

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
  el.btnCreate.disabled = busy;
  el.btnRestore.disabled = busy;
  el.sendAddress.disabled = busy;
  el.sendAmount.disabled = busy || drain;
  el.sendFeeRate.disabled = busy;
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
  el.mwebActions.hidden = !visible;
  el.mwebReceive.hidden = !visible;
  views.private.nav.hidden = !visible;
  if (!visible && currentView === "private") setView("balance");
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

function renderHistory(txs: TxRecord[]) {
  el.txList.innerHTML = "";
  el.txEmpty.hidden = txs.length > 0;
  for (const tx of txs) {
    const dir = tx.net_sats >= 0 ? "in" : "out";

    const icon = document.createElement("span");
    icon.className = `tx-icon ${dir}`;
    icon.innerHTML = dir === "in" ? SVG_ARROW_IN : SVG_ARROW_OUT;

    const amount = document.createElement("span");
    amount.className = dir === "in" ? "tx-amt in" : "tx-amt";
    amount.textContent = `${dir === "in" ? "+" : "−"}${formatLtc(Math.abs(tx.net_sats))}`;

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
    li.append(icon, main, side);
    el.txList.appendChild(li);
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
    el.settingsRpc.value = s.litecoin_rpc_url ?? "";
    el.settingsPeers.value = s.mweb_peers.join(", ");
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
  el.mwebProgress.hidden = false;
  el.mwebProgressFill.style.width = `${pct}%`;
  el.mwebProgressText.textContent = `Downloading MWEB outputs: ${p.fetched.toLocaleString(
    "en-US",
  )} / ${p.total.toLocaleString("en-US")} (${pct}%)`;
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

async function wipeAndOnboard() {
  setError(null);
  await invoke("wipe_wallet");
  lastTxid = null;
  renderLastTxid();
  el.txList.innerHTML = "";
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
    setStatus(
      result.new_txs > 0
        ? `Synced · ${result.new_txs} new transaction${result.new_txs === 1 ? "" : "s"}`
        : "Synced",
      "success",
    );
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

el.btnCreate.addEventListener("click", async () => {
  if (syncing || sending) return;
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
    setError("Enter a mnemonic to restore.");
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
    const s = await invoke<WalletSummary>("restore_wallet", {
      req: { mnemonic, network: "mainnet" },
      passphrase,
    });
    renderSummary(s);
    el.onboardPassphrase.value = "";
    el.onboardPassphrase2.value = "";
    el.restoreMnemonic.value = "";
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
  const confirmed = window.confirm(
    "Resync MWEB from scratch? This wipes local MWEB data and re-downloads the full UTXO set (may take a while).",
  );
  if (!confirmed) return;
  syncing = true;
  setError(null);
  updateBusyUi();
  setStatus("Resyncing MWEB from scratch…");
  startMwebProgressPolling();
  try {
    await invoke("resync_mweb");
    await refreshCombined();
    setStatus("MWEB resynced.", "success");
  } catch (e) {
    setError(String(e));
  } finally {
    stopMwebProgressPolling();
    syncing = false;
    updateBusyUi();
  }
});

el.sendDrain.addEventListener("change", () => {
  updateBusyUi();
});

el.sendForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (syncing || sending) return;

  const address = el.sendAddress.value.trim();
  const fee_rate_sat_vb = Number(el.sendFeeRate.value);
  const drain = el.sendDrain.checked;

  if (!address) {
    setError("Enter a destination address.");
    return;
  }
  if (!Number.isFinite(fee_rate_sat_vb) || fee_rate_sat_vb < 1 || !Number.isInteger(fee_rate_sat_vb)) {
    setError("Fee rate must be an integer ≥ 1 sat/vB.");
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
  try {
    const result = await invoke<SendResult>("send_ltc", {
      req: { address, amount_sats, fee_rate_sat_vb, drain },
    });
    lastTxid = result.txid;
    renderLastTxid();
    setStatus(`Broadcast · fee ${result.fee_sats} litoshis — syncing…`);
    sending = false;
    updateBusyUi();
    const ok = await runSync({ quiet: false });
    if (ok) {
      setStatus(`Sent · fee ${result.fee_sats} litoshis`, "success");
    }
  } catch (e) {
    setError(String(e));
    sending = false;
    updateBusyUi();
  }
});

el.btnSaveSettings.addEventListener("click", async () => {
  setError(null);
  try {
    await invoke("update_settings", {
      req: {
        electrum_url: el.settingsElectrum.value.trim(),
        litecoin_rpc_url: el.settingsRpc.value.trim() || null,
        mweb_peers: el.settingsPeers.value
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
      },
    });
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
  const amount_sats = parseLtcToSats(el.peginAmount.value);
  if (amount_sats == null || amount_sats <= 0) {
    setError(amountError("peg-in", el.peginAmount.value));
    return;
  }
  sending = true;
  setError(null);
  updateBusyUi();
  try {
    const result = await invoke<{ txid: string; maturity_blocks: number }>("pegin_ltc", {
      req: { amount_sats },
    });
    lastTxid = result.txid;
    renderLastTxid();
    setStatus(
      `Peg-in broadcast — matures in ${result.maturity_blocks} blocks.`,
      "success",
    );
    sending = false;
    updateBusyUi();
    await runSync({ quiet: false });
  } catch (e) {
    setError(String(e));
    sending = false;
    updateBusyUi();
  }
});

el.btnMwebSend.addEventListener("click", async () => {
  const address = el.mwebSendAddress.value.trim();
  if (!address) {
    setError("Enter an MWEB send address (ltcmweb1…).");
    return;
  }
  const amount_sats = parseLtcToSats(el.mwebSendAmount.value);
  if (amount_sats == null) {
    setError(amountError("MWEB send", el.mwebSendAmount.value));
    return;
  }
  sending = true;
  setError(null);
  updateBusyUi();
  try {
    const result = await invoke<{ wtxid: string; fee_sats: number }>("mweb_send_ltc", {
      req: { address, amount_sats },
    });
    setStatus(`MWEB sent · wtxid ${result.wtxid}`, "success");
    sending = false;
    updateBusyUi();
    await refreshCombined();
    await refreshHistory();
  } catch (e) {
    setError(String(e));
    sending = false;
    updateBusyUi();
  }
});

el.btnPegout.addEventListener("click", async () => {
  const address = el.pegoutAddress.value.trim();
  if (!address) {
    setError("Enter a peg-out address (ltc1…) in the peg-out address field.");
    return;
  }
  const amount_sats = parseLtcToSats(el.pegoutAmount.value);
  if (amount_sats == null) {
    setError(amountError("peg-out", el.pegoutAmount.value));
    return;
  }
  if (amount_sats < DUST_LITOSHIS) {
    setError(`Peg-out amount must be ≥ ${DUST_LITOSHIS} litoshis.`);
    return;
  }
  sending = true;
  setError(null);
  updateBusyUi();
  try {
    const result = await invoke<{ wtxid: string }>("pegout_ltc", {
      req: { address, amount_sats },
    });
    setStatus(`Peg-out broadcast · wtxid ${result.wtxid}`, "success");
    sending = false;
    updateBusyUi();
    await runSync({ quiet: false });
  } catch (e) {
    setError(String(e));
    sending = false;
    updateBusyUi();
  }
});

void boot();
