export interface SnaptradeSyncResult {
  accountsSynced?: number;
  activitiesSynced?: number;
  rawActivitiesFromSnaptrade?: number;
  skippedUnmappedAccount?: number;
  saved?: number;
  holdingsSynced?: number;
  positionsCount?: number;
  cashBalancesCount?: number;
}

interface SnaptradeSyncStatus {
  state: 'idle' | 'running' | 'succeeded' | 'failed';
  startedAt?: string | null;
  finishedAt?: string | null;
  error?: string | null;
  result?: SnaptradeSyncResult | null;
}

const POLL_INTERVAL_MS = 3000;
const MAX_WAIT_MS = 15 * 60 * 1000;

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Starts a SnapTrade sync and resolves once it finishes.
 *
 * The backend runs the sync off-request and answers 202 immediately: a full
 * sync takes minutes, far longer than the hosting gateway will hold a request
 * open. Progress is therefore polled instead of awaited on the POST.
 */
export async function runSnaptradeSync(): Promise<SnaptradeSyncResult> {
  const response = await fetch('/api/v1/dfc/snaptrade/sync', {
    method: 'POST',
    credentials: 'same-origin',
  });

  if (!response.ok) {
    const data = await response.json().catch(() => ({}));
    throw new Error(data.message || 'Sync failed');
  }

  const deadline = Date.now() + MAX_WAIT_MS;

  while (Date.now() < deadline) {
    await wait(POLL_INTERVAL_MS);

    const statusResponse = await fetch('/api/v1/dfc/snaptrade/sync-status', {
      credentials: 'same-origin',
    });

    if (!statusResponse.ok) {
      // A transient proxy hiccup must not abort a sync that is still running.
      continue;
    }

    const status: SnaptradeSyncStatus = await statusResponse.json();

    if (status.state === 'succeeded') {
      return status.result ?? {};
    }

    if (status.state === 'failed') {
      throw new Error(status.error || 'Sync failed');
    }
  }

  throw new Error("La synchronisation est plus longue que prévu. Elle continue en arrière-plan.");
}
