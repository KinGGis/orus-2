import { runSnaptradeSync } from "./snaptrade-sync";

/**
 * A brokerage integration that can refresh an account's data on demand.
 *
 * Accounts record which integration feeds them, and each one has its own sync
 * endpoint. Resolving the integration from the account keeps the UI from
 * offering a refresh that targets the wrong broker.
 */
export interface BrokerIntegration {
  /** Brand name, shown in buttons and toasts. */
  label: string;
  /** Runs the sync and resolves with a one-line summary of what changed. */
  run: () => Promise<string>;
}

interface IbkrSyncResponse {
  message?: string;
}

interface IbkrSyncStatus {
  syncRunning?: boolean;
  lastSync?: { finishedAt: string; success: boolean; message: string } | null;
}

const SYNC_POLL_INTERVAL_MS = 5_000;
const SYNC_POLL_LIMIT_MS = 35 * 60_000;

/**
 * Starts an IBKR sync and resolves once it has finished.
 *
 * The endpoint answers as soon as the job is accepted: the import walks years
 * of daily bars and outlives any request timeout. The completion event travels
 * over SSE, which the proxy cuts after about thirty seconds, so the outcome is
 * read by polling the status endpoint instead. Starting while a sync is already
 * running simply waits for that one.
 */
export async function runIbkrSync(): Promise<string> {
  const startedAt = Date.now();
  const response = await fetch("/api/v1/dfc/ibkr/sync", {
    method: "POST",
    credentials: "same-origin",
  });

  if (!response.ok) {
    throw new Error((await response.text()) || "Sync failed");
  }

  const started: IbkrSyncResponse = await response.json();

  while (Date.now() - startedAt < SYNC_POLL_LIMIT_MS) {
    await new Promise((resolve) => setTimeout(resolve, SYNC_POLL_INTERVAL_MS));
    const statusResponse = await fetch("/api/v1/dfc/ibkr/status", {
      credentials: "same-origin",
    });
    if (!statusResponse.ok) continue;
    const status: IbkrSyncStatus = await statusResponse.json();
    if (status.syncRunning) continue;

    const last = status.lastSync;
    if (!last) {
      // The server restarted mid-sync and lost track of it.
      throw new Error("La synchronisation IBKR a été interrompue.");
    }
    if (!last.success) throw new Error(last.message);
    return last.message || "Synchronisation IBKR terminée.";
  }

  return started.message ?? "Synchronisation IBKR toujours en cours.";
}

async function runSnaptrade(): Promise<string> {
  const result = await runSnaptradeSync();
  return `${result.accountsSynced ?? 0} comptes, ${result.activitiesSynced ?? 0} activités synchronisés`;
}

export function brokerIntegrationFor(
  provider?: string | null,
): BrokerIntegration | null {
  switch (provider?.toUpperCase()) {
    case "IBKR_MCP":
      return { label: "IBKR", run: runIbkrSync };
    case "SNAPTRADE":
      return { label: "SnapTrade", run: runSnaptrade };
    default:
      return null;
  }
}
