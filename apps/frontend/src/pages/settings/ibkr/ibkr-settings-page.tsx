import { Alert, AlertDescription } from "@wealthfolio/ui/components/ui/alert";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Button } from "@wealthfolio/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Separator } from "@wealthfolio/ui/components/ui/separator";
import { useToast } from "@wealthfolio/ui/components/ui/use-toast";
import { useCallback, useEffect, useState } from "react";
import { SettingsHeader } from "../settings-header";

interface IbkrStatus {
  connected: boolean;
  clientRegistered: boolean;
}

interface IbkrDiagnosticStage {
  name: string;
  ok: boolean;
  elapsedMs: number;
  error?: string | null;
}

interface IbkrDiagnostic {
  stages: IbkrDiagnosticStage[];
  positions: number;
  positionsPriced: number;
  unpricedSymbols: string[];
  activities: number;
  cashBalances: number;
  activitiesMapped: number;
  storedActivities: number;
  storedError?: string | null;
}

export const IBKR_REDIRECT_PATH = "/ibkr/callback";

/**
 * Turns a failed response into something worth showing.
 *
 * A request to this app does not reach the server directly; it goes through a
 * proxy rewrite that answers `502` with an HTML page of its own when the
 * upstream takes too long. Surfacing that page verbatim tells the user nothing,
 * so name the condition instead.
 */
async function describeFailure(response: Response): Promise<string> {
  if (response.status === 502 || response.status === 504) {
    return "Le serveur a mis trop de temps à répondre et la passerelle a coupé la connexion. IBKR est probablement lent ou indisponible.";
  }
  const body = (await response.text()).trim();
  if (!body || body.startsWith("<")) {
    return `Le serveur a répondu ${response.status}.`;
  }
  return body;
}

/**
 * Whether a given step of the diagnostic ran and succeeded.
 *
 * The server stops at the first failing step, so a step may simply be absent;
 * its figures are then meaningless and must not be shown as zeroes.
 */
function stageSucceeded(diagnostic: IbkrDiagnostic, name: string): boolean {
  return diagnostic.stages.some((stage) => stage.name === name && stage.ok);
}

export default function IbkrSettingsPage() {
  const { toast } = useToast();
  const [status, setStatus] = useState<IbkrStatus | null>(null);
  const [diagnostic, setDiagnostic] = useState<IbkrDiagnostic | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isConnecting, setIsConnecting] = useState(false);
  const [isSyncing, setIsSyncing] = useState(false);
  const [isDiagnosing, setIsDiagnosing] = useState(false);
  const [isDisconnecting, setIsDisconnecting] = useState(false);

  const fetchStatus = useCallback(async () => {
    try {
      const response = await fetch("/api/v1/dfc/ibkr/status", {
        credentials: "same-origin",
      });
      if (!response.ok) throw new Error("Failed to fetch status");
      setStatus(await response.json());
    } catch (error) {
      console.error("Error fetching IBKR status:", error);
      setStatus({ connected: false, clientRegistered: false });
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  const handleConnect = async () => {
    setIsConnecting(true);
    try {
      const response = await fetch("/api/v1/dfc/ibkr/auth-url", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          redirectUri: `${window.location.origin}${IBKR_REDIRECT_PATH}`,
        }),
      });
      if (!response.ok) throw new Error(await response.text());
      const { authUrl } = await response.json();
      window.location.href = authUrl;
    } catch (error) {
      console.error("Error starting IBKR authorization:", error);
      toast({
        title: "Erreur",
        description: "Impossible de démarrer l'autorisation IBKR",
        variant: "destructive",
      });
      setIsConnecting(false);
    }
  };

  const handleSync = async () => {
    setIsSyncing(true);
    try {
      const response = await fetch("/api/v1/dfc/ibkr/sync", {
        method: "POST",
        credentials: "same-origin",
      });
      if (!response.ok) throw new Error(await describeFailure(response));
      const result = await response.json();
      toast({
        title: "Synchronisation lancée",
        description:
          result.message ??
          "Synchronisation IBKR lancée. Le résultat s'affichera à la fin.",
      });
    } catch (error) {
      console.error("Error syncing IBKR:", error);
      toast({
        title: "La synchronisation IBKR a échoué",
        description:
          error instanceof Error && error.message
            ? error.message
            : "Raison inconnue.",
        variant: "destructive",
      });
    } finally {
      setIsSyncing(false);
    }
  };

  const handleDiagnostic = async () => {
    setIsDiagnosing(true);
    try {
      const response = await fetch("/api/v1/dfc/ibkr/diagnostic", {
        credentials: "same-origin",
      });
      if (!response.ok) throw new Error(await describeFailure(response));
      setDiagnostic(await response.json());
    } catch (error) {
      console.error("Error running IBKR diagnostic:", error);
      toast({
        title: "Le diagnostic IBKR a échoué",
        description:
          error instanceof Error && error.message
            ? error.message
            : "Raison inconnue.",
        variant: "destructive",
      });
    } finally {
      setIsDiagnosing(false);
    }
  };

  const handleDisconnect = async () => {
    setIsDisconnecting(true);
    try {
      const response = await fetch("/api/v1/dfc/ibkr/connection", {
        method: "DELETE",
        credentials: "same-origin",
      });
      if (!response.ok) throw new Error(await response.text());
      setDiagnostic(null);
      await fetchStatus();
      toast({ title: "IBKR déconnecté" });
    } catch (error) {
      console.error("Error disconnecting IBKR:", error);
      toast({
        title: "Erreur",
        description: "La déconnexion a échoué",
        variant: "destructive",
      });
    } finally {
      setIsDisconnecting(false);
    }
  };

  if (isLoading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Icons.Spinner className="text-muted-foreground h-8 w-8 animate-spin" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <SettingsHeader
        heading="Interactive Brokers"
        text="Connexion directe au serveur MCP d'IBKR : positions, soldes et transactions lus chez le courtier, sans agrégateur."
      />
      <Separator />

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="bg-muted flex h-10 w-10 items-center justify-center rounded-lg">
                <Icons.Link className="h-5 w-5" />
              </div>
              <div>
                <CardTitle className="text-base">
                  Interactive Brokers (MCP)
                </CardTitle>
                <CardDescription>Accès direct en lecture seule</CardDescription>
              </div>
            </div>
            <Badge variant={status?.connected ? "default" : "secondary"}>
              {status?.connected ? "Connecté" : "Non connecté"}
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          {!status?.connected && (
            <Alert>
              <Icons.AlertCircle className="h-4 w-4" />
              <AlertDescription>
                Une autorisation IBKR est nécessaire une seule fois. Le jeton de
                rafraîchissement obtenu permet ensuite les synchronisations
                automatiques.
              </AlertDescription>
            </Alert>
          )}

          <div className="flex flex-wrap gap-2">
            {!status?.connected ? (
              <Button onClick={handleConnect} disabled={isConnecting}>
                {isConnecting ? (
                  <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                ) : (
                  <Icons.Plus className="mr-2 h-4 w-4" />
                )}
                Autoriser IBKR
              </Button>
            ) : (
              <>
                <Button onClick={handleSync} disabled={isSyncing}>
                  {isSyncing ? (
                    <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                  ) : (
                    <Icons.RefreshCw className="mr-2 h-4 w-4" />
                  )}
                  Synchroniser
                </Button>
                <Button
                  variant="outline"
                  onClick={handleDiagnostic}
                  disabled={isDiagnosing}
                >
                  {isDiagnosing ? (
                    <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                  ) : (
                    <Icons.AlertCircle className="mr-2 h-4 w-4" />
                  )}
                  Diagnostic
                </Button>
                <Button
                  variant="outline"
                  onClick={handleDisconnect}
                  disabled={isDisconnecting}
                >
                  {isDisconnecting ? (
                    <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                  ) : (
                    <Icons.Trash className="mr-2 h-4 w-4" />
                  )}
                  Déconnecter
                </Button>
              </>
            )}
          </div>

          {diagnostic && (
            <div className="space-y-2">
              {diagnostic.stages.map((stage) => (
                <Alert
                  key={stage.name}
                  variant={stage.ok ? "default" : "destructive"}
                >
                  {stage.ok ? (
                    <Icons.CheckCircle className="h-4 w-4 text-green-500" />
                  ) : (
                    <Icons.AlertCircle className="h-4 w-4" />
                  )}
                  <AlertDescription>
                    {stage.name} — {stage.ok ? "OK" : stage.error}
                    {" ("}
                    {(stage.elapsedMs / 1000).toFixed(1)} s{")"}
                  </AlertDescription>
                </Alert>
              ))}

              {stageSucceeded(diagnostic, "Positions") && (
                <Alert>
                  <Icons.CheckCircle className="h-4 w-4 text-green-500" />
                  <AlertDescription>
                    IBKR déclare {diagnostic.positions} positions ouvertes (dont{" "}
                    {diagnostic.positionsPriced} valorisées) et{" "}
                    {diagnostic.cashBalances} soldes de trésorerie.
                    {diagnostic.unpricedSymbols.length > 0 && (
                      <> Sans cours : {diagnostic.unpricedSymbols.join(", ")}.</>
                    )}
                  </AlertDescription>
                </Alert>
              )}

              {stageSucceeded(diagnostic, "Transactions") && (
                <Alert>
                  <Icons.CheckCircle className="h-4 w-4 text-green-500" />
                  <AlertDescription>
                    IBKR renvoie {diagnostic.activities} transactions, dont{" "}
                    {diagnostic.activitiesMapped} exploitables par l'import.
                  </AlertDescription>
                </Alert>
              )}

              {diagnostic.storedError ? (
                <Alert variant="destructive">
                  <Icons.AlertCircle className="h-4 w-4" />
                  <AlertDescription>
                    Base locale illisible : {diagnostic.storedError}
                  </AlertDescription>
                </Alert>
              ) : (
                <Alert>
                  <Icons.CheckCircle className="h-4 w-4 text-green-500" />
                  <AlertDescription>
                    {diagnostic.storedActivities} transactions sont enregistrées
                    en base pour ce compte.
                  </AlertDescription>
                </Alert>
              )}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
