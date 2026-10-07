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

interface IbkrDiagnostic {
  positions: number;
  cashBalances: number;
  activities: number;
}

export const IBKR_REDIRECT_PATH = "/ibkr/callback";

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
      if (!response.ok) throw new Error(await response.text());
      const result = await response.json();
      toast({
        title: "Synchronisation terminée",
        description: result.message ?? "Données IBKR importées.",
      });
    } catch (error) {
      console.error("Error syncing IBKR:", error);
      toast({
        title: "Erreur",
        description: "La synchronisation IBKR a échoué",
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
      if (!response.ok) throw new Error(await response.text());
      setDiagnostic(await response.json());
    } catch (error) {
      console.error("Error running IBKR diagnostic:", error);
      toast({
        title: "Erreur",
        description: "Le diagnostic IBKR a échoué",
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
            <Alert>
              <Icons.CheckCircle className="h-4 w-4 text-green-500" />
              <AlertDescription>
                IBKR déclare {diagnostic.positions} positions ouvertes,{" "}
                {diagnostic.cashBalances} soldes de trésorerie et{" "}
                {diagnostic.activities} transactions. Ces chiffres viennent
                directement du courtier : ils font foi.
              </AlertDescription>
            </Alert>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
