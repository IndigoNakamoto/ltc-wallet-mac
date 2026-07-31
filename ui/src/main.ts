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

const DUST_LITOSHIS = 2940;
const AUTO_SYNC_MS = 60_000;

const el = {
  phase: document.querySelector<HTMLElement>("#phase")!,
  error: document.querySelector<HTMLElement>("#error")!,
  fatal: document.querySelector<HTMLElement>("#fatal")!,
  unlock: document.querySelector<HTMLElement>("#unlock")!,
  migrate: document.querySelector<HTMLElement>("#migrate")!,
  onboarding: document.querySelector<HTMLElement>("#onboarding")!,
  mnemonic: document.querySelector<HTMLElement>("#mnemonic")!,
  ready: document.querySelector<HTMLElement>("#ready")!,
  mnemonicText: document.querySelector<HTMLElement>("#mnemonic-text")!,
  networkBadge: document.querySelector<HTMLElement>("#network-badge")!,
  balanceTotal: document.querySelector<HTMLElement>("#balance-total")!,
  balanceSats: document.querySelector<HTMLElement>("#balance-sats")!,
  balanceConfirmed: document.querySelector<HTMLElement>("#balance-confirmed")!,
  balanceMweb: document.querySelector<HTMLElement>("#balance-mweb")!,
  balanceTip: document.querySelector<HTMLElement>("#balance-tip")!,
  balancePending: document.querySelector<HTMLElement>("#balance-pending")!,
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
  status: document.querySelector<HTMLElement>("#status")!,
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

let syncing = false;
let sending = false;
let currentPhase: Phase = "boot";
let lastTxid: string | null = null;
let autoSyncTimer: number | null = null;
let mwebProgressTimer: number | null = null;

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
  el.phase.textContent = next;
  el.fatal.hidden = next !== "fatal";
  el.unlock.hidden = next !== "unlock";
  el.migrate.hidden = next !== "migrate";
  el.onboarding.hidden = next !== "onboarding";
  el.mnemonic.hidden = next !== "mnemonic";
  el.ready.hidden = next !== "ready";
  if (next === "ready") startAutoSync();
  else stopAutoSync();
}

function setError(message: string | null) {
  if (!message) {
    el.error.hidden = true;
    el.error.textContent = "";
    return;
  }
  el.error.hidden = false;
  el.error.textContent = message;
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

  if (sending) el.status.textContent = "sending…";
  else if (syncing) el.status.textContent = "syncing…";
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
  try {
    await QRCode.toCanvas(canvas, paymentUri(address), {
      errorCorrectionLevel: "M",
      margin: 2,
      width: 180,
      color: { dark: "#000000", light: "#ffffff" },
    });
  } catch (e) {
    ctx?.clearRect(0, 0, canvas.width, canvas.height);
    setError(`QR render failed: ${e}`);
  }
}

function renderSummary(s: WalletSummary) {
  el.networkBadge.textContent = s.network;
  el.balanceTotal.textContent = formatLtc(s.total_sats);
  el.balanceSats.textContent = formatLitoshis(s.total_sats);
  el.balanceConfirmed.textContent = `Confirmed: ${formatLtc(s.confirmed_sats)}`;
  el.balanceTip.textContent = `Tip height: ${s.tip_height}`;
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
  el.balanceMweb.hidden = false;
  let mwebText = `Private (MWEB): ${formatLtc(c.mweb_total_sats)}`;
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
  el.mwebActions.hidden = false;
  el.mwebReceive.hidden = false;
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

function renderHistory(txs: TxRecord[]) {
  el.txList.innerHTML = "";
  el.txEmpty.hidden = txs.length > 0;
  for (const tx of txs) {
    const li = document.createElement("li");
    const dir = tx.net_sats >= 0 ? "in" : "out";
    const conf =
      tx.confirmations === 0 ? "pending" : `${tx.confirmations} conf`;
    const kindLabel = TX_KIND_LABELS[tx.kind] ?? "";
    const confText = kindLabel ? `${kindLabel} · ${conf}` : conf;
    const short = `${tx.txid.slice(0, 8)}…${tx.txid.slice(-8)}`;
    li.innerHTML = `<span class="tx-dir ${dir}">${dir}</span>
      <span class="tx-amt">${formatLtc(Math.abs(tx.net_sats))}</span>
      <span class="tx-conf muted">${confText}</span>
      <span class="tx-id mono muted">${short}</span>`;
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
  el.status.textContent = "wallet data reset — create or restore";
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
    el.status.textContent = `synced (+${result.new_txs} txs)`;
    return true;
  } catch (e) {
    if (opts.quiet) {
      el.status.textContent = `auto-sync failed: ${e}`;
    } else {
      setError(String(e));
      el.status.textContent = "";
    }
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
    el.mnemonicText.textContent = resp.mnemonic;
    renderSummary(resp.summary);
    el.onboardPassphrase.value = "";
    el.onboardPassphrase2.value = "";
    setPhase("mnemonic");
    el.status.textContent = "";
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
    setPhase("ready");
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
    el.status.textContent = "receive address refreshed";
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
    el.status.textContent = "no address to copy";
    return;
  }
  try {
    await navigator.clipboard.writeText(address);
    el.status.textContent = "address copied";
  } catch {
    el.status.textContent = "copy failed — select the address manually";
  }
});

el.btnCopyMweb.addEventListener("click", async () => {
  const address = el.mwebAddress.textContent?.trim() ?? "";
  if (!address) return;
  try {
    await navigator.clipboard.writeText(address);
    el.status.textContent = "MWEB address copied";
  } catch {
    el.status.textContent = "copy failed";
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
  el.status.textContent = "resyncing MWEB from scratch…";
  startMwebProgressPolling();
  try {
    await invoke("resync_mweb");
    await refreshCombined();
    el.status.textContent = "MWEB resynced";
  } catch (e) {
    setError(String(e));
    el.status.textContent = "";
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
    el.status.textContent = `broadcast (${result.fee_sats} litoshis fee) — syncing…`;
    sending = false;
    updateBusyUi();
    const ok = await runSync({ quiet: false });
    if (ok) {
      el.status.textContent = `sent · fee ${result.fee_sats} litoshis`;
    }
  } catch (e) {
    setError(String(e));
    el.status.textContent = "";
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
    el.status.textContent = "settings saved";
  } catch (e) {
    setError(String(e));
  }
});

el.btnLock.addEventListener("click", async () => {
  await invoke("lock_wallet");
  stopAutoSync();
  setPhase("unlock");
  el.status.textContent = "wallet locked";
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
    el.status.textContent = `peg-in broadcast — wait ${result.maturity_blocks} blocks to mature`;
    sending = false;
    updateBusyUi();
    await runSync({ quiet: false });
  } catch (e) {
    setError(String(e));
    el.status.textContent = "";
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
    el.status.textContent = `MWEB sent · wtxid ${result.wtxid}`;
    sending = false;
    updateBusyUi();
    await refreshCombined();
    await refreshHistory();
  } catch (e) {
    setError(String(e));
    el.status.textContent = "";
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
    el.status.textContent = `peg-out broadcast · wtxid ${result.wtxid}`;
    sending = false;
    updateBusyUi();
    await runSync({ quiet: false });
  } catch (e) {
    setError(String(e));
    el.status.textContent = "";
    sending = false;
    updateBusyUi();
  }
});

void boot();
