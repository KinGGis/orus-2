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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@wealthfolio/ui/components/ui/table";
import { useToast } from "@wealthfolio/ui/components/ui/use-toast";
import { useCallback, useEffect, useState } from "react";
import { SettingsHeader } from "../settings-header";

interface SnapTradeStatus {
  configured: boolean;
  registered: boolean;
}

interface SnapTradeConnection {
  id: string;
  name: string | null;
  brokerageAuthorizationId: string | null;
  institutionName: string | null;
  createdDate: string | null;
}

interface SnapTradeAccount {
  id: string;
  name: string | null;
  number: string | null;
  institutionName: string | null;
  currency: { code: string | null } | null;
  balance: { total: { amount: number | null } | null } | null;
}

export default function SnapTradeSettingsPage() {
  const { toast } = useToast();
  const [status, setStatus] = useState<SnapTradeStatus | null>(null);
  const [connections, setConnections] = useState<SnapTradeConnection[]>([]);
  const [accounts, setAccounts] = useState<SnapTradeAccount[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isRegistering, setIsRegistering] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [isSyncing, setIsSyncing] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [showAccounts, setShowAccounts] = useState(false);

  const fetchStatus = useCallback(async () => {
    try {
      const response = await fetch("/api/v1/dfc/snaptrade/status", {
        credentials: "same-origin",
      });
      if (!response.ok) {
        throw new Error("Failed to fetch status");
      }
      const data = await response.json();
      setStatus(data);
    } catch (error) {
      console.error("Error fetching status:", error);
      setStatus({ configured: false, registered: false });
    } finally {
      setIsLoading(false);
    }
  }, []);

  const fetchConnections = useCallback(async () => {
    try {
      const response = await fetch("/api/v1/dfc/snaptrade/connections", {
        credentials: "same-origin",
      });
      if (!response.ok) {
        throw new Error("Failed to fetch connections");
      }
      const data = await response.json();
      setConnections(data);
    } catch (error) {
      console.error("Error fetching connections:", error);
    }
  }, []);

  const fetchAccounts = useCallback(async () => {
    try {
      const response = await fetch("/api/v1/dfc/snaptrade/accounts", {
        credentials: "same-origin",
      });
      if (!response.ok) {
        throw new Error("Failed to fetch accounts");
      }
      const data = await response.json();
      setAccounts(data);
      setShowAccounts(true);
    } catch (error) {
      console.error("Error fetching accounts:", error);
      toast({
        title: "Erreur",
        description: "Impossible de récupérer les comptes",
        variant: "destructive",
      });
    }
  }, [toast]);

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  useEffect(() => {
    if (status?.registered) {
      fetchConnections();
    }
  }, [status?.registered, fetchConnections]);

  const handleRegister = async () => {
    setIsRegistering(true);
    try {
      const response = await fetch("/api/v1/dfc/snaptrade/register", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "same-origin",
        body: JSON.stringify({ userId: "dfc-user" }),
      });
      if (!response.ok) {
        const data = await response.json().catch(() => ({}));
        throw new Error(data.message || "Registration failed");
      }
      toast({
        title: "Succès",
        description: "Utilisateur SnapTrade enregistré",
      });
      await fetchStatus();
    } catch (error) {
      console.error("Error registering:", error);
      toast({
        title: "Erreur",
        description:
          error instanceof Error ? error.message : "Échec de l'enregistrement",
        variant: "destructive",
      });
    } finally {
      setIsRegistering(false);
    }
  };

  const handleConnect = async () => {
    setIsConnecting(true);
    try {
      const response = await fetch("/api/v1/dfc/snaptrade/connect-url", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "same-origin",
        body: JSON.stringify({
          redirectUri: `${window.location.origin}/snaptrade/callback`,
        }),
      });
      if (!response.ok) {
        const data = await response.json().catch(() => ({}));
        throw new Error(data.message || "Failed to get connect URL");
      }
      const { url } = await response.json();
      window.open(url, "_blank", "noopener,noreferrer");
    } catch (error) {
      console.error("Error getting connect URL:", error);
      toast({
        title: "Erreur",
        description:
          error instanceof Error
            ? error.message
            : "Impossible d'initier la connexion",
        variant: "destructive",
      });
    } finally {
      setIsConnecting(false);
    }
  };

  const handleDeleteConnection = async (authId: string) => {
    try {
      const response = await fetch(
        `/api/v1/dfc/snaptrade/connections/${authId}`,
        {
          method: "DELETE",
          credentials: "same-origin",
        }
      );
      if (!response.ok) {
        throw new Error("Failed to delete connection");
      }
      toast({
        title: "Succès",
        description: "Connexion supprimée",
      });
      await fetchConnections();
    } catch (error) {
      console.error("Error deleting connection:", error);
      toast({
        title: "Erreur",
        description: "Impossible de supprimer la connexion",
        variant: "destructive",
      });
    }
  };

  const handleSync = async () => {
    setIsSyncing(true);
    try {
      const response = await fetch("/api/v1/dfc/snaptrade/sync", {
        method: "POST",
        credentials: "same-origin",
      });
      if (!response.ok) {
        const data = await response.json().catch(() => ({}));
        throw new Error(data.message || "Sync failed");
      }
      const result = await response.json();
      toast({
        title: "Synchronisation réussie",
        description: `✓ ${result.accountsSynced} comptes, ${result.activitiesSynced} activités synchronisés`,
      });
    } catch (error) {
      console.error("Error syncing:", error);
      toast({
        title: "Erreur",
        description:
          error instanceof Error ? error.message : "Échec de la synchronisation",
        variant: "destructive",
      });
    } finally {
      setIsSyncing(false);
    }
  };

  const handleDeleteUser = async () => {
    if (
      !window.confirm(
        "Êtes-vous sûr de vouloir supprimer l'utilisateur SnapTrade? Cette action est irréversible."
      )
    ) {
      return;
    }
    setIsDeleting(true);
    try {
      const response = await fetch("/api/v1/dfc/snaptrade/user", {
        method: "DELETE",
        credentials: "same-origin",
      });
      if (!response.ok) {
        throw new Error("Failed to delete user");
      }
      toast({
        title: "Succès",
        description: "Utilisateur SnapTrade supprimé",
      });
      setConnections([]);
      setAccounts([]);
      setShowAccounts(false);
      await fetchStatus();
    } catch (error) {
      console.error("Error deleting user:", error);
      toast({
        title: "Erreur",
        description: "Impossible de supprimer l'utilisateur",
        variant: "destructive",
      });
    } finally {
      setIsDeleting(false);
    }
  };

  if (isLoading) {
    return (
      <div className="space-y-6">
        <SettingsHeader
          heading="SnapTrade"
          text="Connectez vos comptes de courtage via SnapTrade pour synchroniser vos transactions."
        />
        <Separator />
        <div className="flex items-center justify-center py-12">
          <Icons.Spinner className="text-muted-foreground h-8 w-8 animate-spin" />
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <SettingsHeader
        heading="SnapTrade"
        text="Connectez vos comptes de courtage via SnapTrade pour synchroniser vos transactions."
      />
      <Separator />

      {/* Status Card */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="bg-muted flex h-10 w-10 items-center justify-center rounded-lg">
                <Icons.Link className="h-5 w-5" />
              </div>
              <div>
                <CardTitle className="text-base">SnapTrade</CardTitle>
                <CardDescription>Agrégation de comptes courtiers</CardDescription>
              </div>
            </div>
            <Badge variant={status?.configured ? "default" : "secondary"}>
              {status?.configured ? "Configuré" : "Non configuré"}
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          {!status?.configured && (
            <Alert>
              <Icons.AlertCircle className="h-4 w-4" />
              <AlertDescription>
                Ajoutez SNAPTRADE_CLIENT_ID et SNAPTRADE_CONSUMER_KEY dans .env
                pour activer SnapTrade.
              </AlertDescription>
            </Alert>
          )}

          {status?.configured && !status?.registered && (
            <>
              <p className="text-muted-foreground text-sm">
                Enregistrez-vous auprès de SnapTrade pour pouvoir connecter vos
                comptes courtiers.
              </p>
              <Button onClick={handleRegister} disabled={isRegistering}>
                {isRegistering && (
                  <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                )}
                Enregistrer DFC
              </Button>
            </>
          )}

          {status?.configured && status?.registered && (
            <div className="bg-muted/50 rounded-lg p-4">
              <div className="flex items-center gap-2 text-sm">
                <Icons.CheckCircle className="h-4 w-4 text-green-500" />
                <span className="text-muted-foreground">
                  Utilisateur SnapTrade enregistré
                </span>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Connections Card */}
      {status?.configured && status?.registered && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Connexions courtiers</CardTitle>
            <CardDescription>
              Gérez vos connexions aux différentes plateformes de courtage.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <Button onClick={handleConnect} disabled={isConnecting}>
              {isConnecting && (
                <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
              )}
              <Icons.Plus className="mr-2 h-4 w-4" />
              Connecter un courtier
            </Button>

            {connections.length > 0 && (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Institution</TableHead>
                    <TableHead>Nom</TableHead>
                    <TableHead>Date de connexion</TableHead>
                    <TableHead className="w-[100px]">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {connections.map((conn) => (
                    <TableRow key={conn.id}>
                      <TableCell>
                        {conn.institutionName || "Inconnu"}
                      </TableCell>
                      <TableCell>{conn.name || "-"}</TableCell>
                      <TableCell>
                        {conn.createdDate
                          ? new Date(conn.createdDate).toLocaleDateString(
                              "fr-FR"
                            )
                          : "-"}
                      </TableCell>
                      <TableCell>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() =>
                            handleDeleteConnection(
                              conn.brokerageAuthorizationId || conn.id
                            )
                          }
                        >
                          <Icons.Trash className="h-4 w-4 text-destructive" />
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}

            {connections.length === 0 && (
              <p className="text-muted-foreground text-sm">
                Aucune connexion courtier. Cliquez sur "Connecter un courtier"
                pour commencer.
              </p>
            )}
          </CardContent>
        </Card>
      )}

      {/* Sync Card */}
      {status?.configured && status?.registered && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Synchronisation</CardTitle>
            <CardDescription>
              Synchronisez vos comptes et transactions depuis SnapTrade.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex gap-2">
              <Button onClick={handleSync} disabled={isSyncing}>
                {isSyncing && (
                  <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                )}
                <Icons.RefreshCw className="mr-2 h-4 w-4" />
                Synchroniser
              </Button>
              <Button variant="outline" onClick={fetchAccounts}>
                Afficher les comptes
              </Button>
            </div>

            {showAccounts && accounts.length > 0 && (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Nom</TableHead>
                    <TableHead>Institution</TableHead>
                    <TableHead>Devise</TableHead>
                    <TableHead className="text-right">Solde</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {accounts.map((acc) => (
                    <TableRow key={acc.id}>
                      <TableCell>{acc.name || acc.number || acc.id}</TableCell>
                      <TableCell>{acc.institutionName || "-"}</TableCell>
                      <TableCell>{acc.currency?.code || "-"}</TableCell>
                      <TableCell className="text-right">
                        {acc.balance?.total?.amount != null
                          ? acc.balance.total.amount.toLocaleString("fr-FR", {
                              minimumFractionDigits: 2,
                              maximumFractionDigits: 2,
                            })
                          : "-"}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>
      )}

      {/* Danger Zone */}
      {status?.configured && status?.registered && (
        <Card className="border-destructive">
          <CardHeader>
            <CardTitle className="text-destructive text-base">
              Zone de danger
            </CardTitle>
            <CardDescription>
              Actions irréversibles sur votre compte SnapTrade.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Button
              variant="destructive"
              onClick={handleDeleteUser}
              disabled={isDeleting}
            >
              {isDeleting && (
                <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
              )}
              <Icons.Trash className="mr-2 h-4 w-4" />
              Supprimer l'utilisateur SnapTrade
            </Button>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
