import { invoke } from "@tauri-apps/api/core";

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

type Phase = "boot" | "onboarding" | "mnemonic" | "ready" | "fatal";

const el = {
  phase: document.querySelector<HTMLElement>("#phase")!,
  error: document.querySelector<HTMLElement>("#error")!,
  fatal: document.querySelector<HTMLElement>("#fatal")!,
  onboarding: document.querySelector<HTMLElement>("#onboarding")!,
  mnemonic: document.querySelector<HTMLElement>("#mnemonic")!,
  ready: document.querySelector<HTMLElement>("#ready")!,
  mnemonicText: document.querySelector<HTMLElement>("#mnemonic-text")!,
  networkBadge: document.querySelector<HTMLElement>("#network-badge")!,
  balanceTotal: document.querySelector<HTMLElement>("#balance-total")!,
  balanceSats: document.querySelector<HTMLElement>("#balance-sats")!,
  balanceConfirmed: document.querySelector<HTMLElement>("#balance-confirmed")!,
  balanceTip: document.querySelector<HTMLElement>("#balance-tip")!,
  balancePending: document.querySelector<HTMLElement>("#balance-pending")!,
  address: document.querySelector<HTMLElement>("#address")!,
  status: document.querySelector<HTMLElement>("#status")!,
  lastTxid: document.querySelector<HTMLElement>("#last-txid")!,
  restoreMnemonic: document.querySelector<HTMLTextAreaElement>("#restore-mnemonic")!,
  sendForm: document.querySelector<HTMLFormElement>("#send-form")!,
  sendAddress: document.querySelector<HTMLInputElement>("#send-address")!,
  sendAmount: document.querySelector<HTMLInputElement>("#send-amount")!,
  sendFeeRate: document.querySelector<HTMLInputElement>("#send-fee-rate")!,
  btnCreate: document.querySelector<HTMLButtonElement>("#btn-create")!,
  btnRestore: document.querySelector<HTMLButtonElement>("#btn-restore")!,
  btnMnemonicDone: document.querySelector<HTMLButtonElement>("#btn-mnemonic-done")!,
  btnSync: document.querySelector<HTMLButtonElement>("#btn-sync")!,
  btnAddress: document.querySelector<HTMLButtonElement>("#btn-address")!,
  btnCopy: document.querySelector<HTMLButtonElement>("#btn-copy")!,
  btnSend: document.querySelector<HTMLButtonElement>("#btn-send")!,
  btnWipe: document.querySelector<HTMLButtonElement>("#btn-wipe")!,
};

let syncing = false;
let sending = false;
let lastTxid: string | null = null;

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

function setPhase(next: Phase) {
  el.phase.textContent = next;
  el.fatal.hidden = next !== "fatal";
  el.onboarding.hidden = next !== "onboarding";
  el.mnemonic.hidden = next !== "mnemonic";
  el.ready.hidden = next !== "ready";
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
  el.btnSync.disabled = busy;
  el.btnAddress.disabled = busy;
  el.btnCopy.disabled = busy;
  el.btnSend.disabled = busy;
  el.btnCreate.disabled = busy;
  el.btnRestore.disabled = busy;
  el.sendAddress.disabled = busy;
  el.sendAmount.disabled = busy;
  el.sendFeeRate.disabled = busy;

  if (sending) {
    el.status.textContent = "sending…";
  } else if (syncing) {
    el.status.textContent = "syncing…";
  }
}

function renderSummary(s: WalletSummary) {
  el.networkBadge.textContent = s.network;
  el.balanceTotal.textContent = formatLtc(s.total_sats);
  el.balanceSats.textContent = formatLitoshis(s.total_sats);
  el.balanceConfirmed.textContent = `Confirmed: ${formatLtc(s.confirmed_sats)}`;
  el.balanceTip.textContent = `Tip height: ${s.tip_height}`;
  el.address.textContent = s.receive_address;

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

function renderLastTxid() {
  if (!lastTxid) {
    el.lastTxid.hidden = true;
    el.lastTxid.textContent = "";
    return;
  }
  el.lastTxid.hidden = false;
  el.lastTxid.textContent = `Last txid: ${lastTxid}`;
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
    const s = await invoke<WalletSummary>("load_wallet");
    renderSummary(s);
    setPhase("ready");
    void runSync();
  } catch (e) {
    setError(String(e));
    setPhase("fatal");
  }
}

el.btnWipe.addEventListener("click", async () => {
  setError(null);
  el.btnWipe.disabled = true;
  try {
    await invoke("wipe_wallet");
    lastTxid = null;
    renderLastTxid();
    setPhase("onboarding");
    el.status.textContent = "wallet data reset — create or restore";
  } catch (e) {
    setError(String(e));
  } finally {
    el.btnWipe.disabled = false;
  }
});

async function runSync(): Promise<boolean> {
  if (syncing || sending) return false;
  syncing = true;
  setError(null);
  updateBusyUi();
  try {
    const result = await invoke<SyncResult>("sync_wallet");
    renderSummary(result.summary);
    el.status.textContent = `synced (+${result.new_txs} txs)`;
    return true;
  } catch (e) {
    setError(String(e));
    el.status.textContent = "";
    return false;
  } finally {
    syncing = false;
    updateBusyUi();
  }
}

el.btnCreate.addEventListener("click", async () => {
  if (syncing || sending) return;
  syncing = true;
  setError(null);
  updateBusyUi();
  try {
    const resp = await invoke<CreateWalletResponse>("create_wallet", {
      req: { network: "mainnet" },
    });
    el.mnemonicText.textContent = resp.mnemonic;
    renderSummary(resp.summary);
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
  syncing = true;
  setError(null);
  updateBusyUi();
  try {
    const s = await invoke<WalletSummary>("restore_wallet", {
      req: { mnemonic, network: "mainnet" },
    });
    renderSummary(s);
    setPhase("ready");
    syncing = false;
    updateBusyUi();
    void runSync();
  } catch (e) {
    setError(String(e));
    syncing = false;
    updateBusyUi();
  }
});

el.btnMnemonicDone.addEventListener("click", () => {
  el.mnemonicText.textContent = "";
  setPhase("ready");
  void runSync();
});

el.btnSync.addEventListener("click", () => {
  void runSync();
});

el.btnAddress.addEventListener("click", async () => {
  if (syncing || sending) return;
  syncing = true;
  setError(null);
  updateBusyUi();
  try {
    const address = await invoke<string>("get_receive_address");
    el.address.textContent = address;
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

el.sendForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (syncing || sending) return;

  const address = el.sendAddress.value.trim();
  const amount_sats = Number(el.sendAmount.value);
  const fee_rate_sat_vb = Number(el.sendFeeRate.value);

  if (!address) {
    setError("Enter a destination address.");
    return;
  }
  if (!Number.isFinite(amount_sats) || amount_sats <= 0 || !Number.isInteger(amount_sats)) {
    setError("Amount must be a positive whole number of litoshis.");
    return;
  }
  if (!Number.isFinite(fee_rate_sat_vb) || fee_rate_sat_vb < 1 || !Number.isInteger(fee_rate_sat_vb)) {
    setError("Fee rate must be an integer ≥ 1 sat/vB.");
    return;
  }

  sending = true;
  setError(null);
  updateBusyUi();
  try {
    const result = await invoke<SendResult>("send_ltc", {
      req: { address, amount_sats, fee_rate_sat_vb },
    });
    lastTxid = result.txid;
    renderLastTxid();
    el.status.textContent = `broadcast (${result.fee_sats} litoshis fee) — syncing…`;
    sending = false;
    updateBusyUi();
    const ok = await runSync();
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

void boot();
