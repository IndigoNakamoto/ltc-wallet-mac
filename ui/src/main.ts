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

type Phase = "boot" | "onboarding" | "mnemonic" | "ready" | "fatal";

const el = {
  phase: document.querySelector<HTMLElement>("#phase")!,
  error: document.querySelector<HTMLElement>("#error")!,
  onboarding: document.querySelector<HTMLElement>("#onboarding")!,
  mnemonic: document.querySelector<HTMLElement>("#mnemonic")!,
  ready: document.querySelector<HTMLElement>("#ready")!,
  mnemonicText: document.querySelector<HTMLElement>("#mnemonic-text")!,
  summary: document.querySelector<HTMLElement>("#summary")!,
  address: document.querySelector<HTMLElement>("#address")!,
  status: document.querySelector<HTMLElement>("#status")!,
  restoreMnemonic: document.querySelector<HTMLTextAreaElement>("#restore-mnemonic")!,
  btnCreate: document.querySelector<HTMLButtonElement>("#btn-create")!,
  btnRestore: document.querySelector<HTMLButtonElement>("#btn-restore")!,
  btnMnemonicDone: document.querySelector<HTMLButtonElement>("#btn-mnemonic-done")!,
  btnSync: document.querySelector<HTMLButtonElement>("#btn-sync")!,
  btnAddress: document.querySelector<HTMLButtonElement>("#btn-address")!,
  btnRefresh: document.querySelector<HTMLButtonElement>("#btn-refresh")!,
};

let syncing = false;

function setPhase(next: Phase) {
  el.phase.textContent = next;
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

function renderSummary(s: WalletSummary) {
  el.summary.textContent = JSON.stringify(s, null, 2);
  el.address.textContent = s.receive_address;
}

function setBusy(busy: boolean) {
  syncing = busy;
  el.btnSync.disabled = busy;
  el.btnAddress.disabled = busy;
  el.btnRefresh.disabled = busy;
  el.btnCreate.disabled = busy;
  el.btnRestore.disabled = busy;
  el.status.textContent = busy ? "working…" : "";
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
    // Background sync — don't block first paint on Electrum.
    void runSync();
  } catch (e) {
    setPhase("fatal");
    setError(String(e));
  }
}

async function runSync() {
  setBusy(true);
  setError(null);
  try {
    const result = await invoke<SyncResult>("sync_wallet");
    renderSummary(result.summary);
    el.status.textContent = `synced (+${result.new_txs} txs)`;
  } catch (e) {
    setError(String(e));
  } finally {
    setBusy(false);
  }
}

el.btnCreate.addEventListener("click", async () => {
  setBusy(true);
  setError(null);
  try {
    const resp = await invoke<CreateWalletResponse>("create_wallet", {
      req: { network: "testnet" },
    });
    el.mnemonicText.textContent = resp.mnemonic;
    renderSummary(resp.summary);
    setPhase("mnemonic");
  } catch (e) {
    setError(String(e));
  } finally {
    setBusy(false);
  }
});

el.btnRestore.addEventListener("click", async () => {
  const mnemonic = el.restoreMnemonic.value.trim();
  if (!mnemonic) {
    setError("Enter a mnemonic to restore.");
    return;
  }
  setBusy(true);
  setError(null);
  try {
    const s = await invoke<WalletSummary>("restore_wallet", {
      req: { mnemonic, network: "testnet" },
    });
    renderSummary(s);
    setPhase("ready");
    void runSync();
  } catch (e) {
    setError(String(e));
  } finally {
    setBusy(false);
  }
});

el.btnMnemonicDone.addEventListener("click", () => {
  el.mnemonicText.textContent = "";
  setPhase("ready");
  void runSync();
});

el.btnSync.addEventListener("click", () => {
  if (!syncing) void runSync();
});

el.btnAddress.addEventListener("click", async () => {
  setBusy(true);
  setError(null);
  try {
    const address = await invoke<string>("get_receive_address");
    el.address.textContent = address;
    el.status.textContent = "receive address refreshed";
  } catch (e) {
    setError(String(e));
  } finally {
    setBusy(false);
  }
});

el.btnRefresh.addEventListener("click", async () => {
  setBusy(true);
  setError(null);
  try {
    const s = await invoke<WalletSummary>("get_summary");
    renderSummary(s);
  } catch (e) {
    setError(String(e));
  } finally {
    setBusy(false);
  }
});

void boot();
