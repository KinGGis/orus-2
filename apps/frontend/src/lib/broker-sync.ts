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

/**
 * Starts an IBKR sync.
 *
 * The endpoint answers as soon as the job is accepted, not when it is done:
 * the import walks years of daily bars and outlives any reasonable request
 * timeout. The outcome arrives later as a broker sync event.
 */
async function runIbkrSync(): Promise<string> {
  const response = await fetch("/api/v1/dfc/ibkr/sync", {
    method: "POST",
    credentials: "same-origin",
  });

  if (!response.ok) {
    throw new Error((await response.text()) || "Sync failed");
  }

  const result: IbkrSyncResponse = await response.json();
  return result.message ?? "Synchronisation IBKR lancée.";
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
