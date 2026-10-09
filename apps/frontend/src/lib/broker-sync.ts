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
 * Runs an IBKR sync.
 *
 * Unlike SnapTrade's, this endpoint does the work inside the request and
 * answers once everything is imported, so there is no progress to poll.
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
  return result.message ?? "Données IBKR importées.";
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
