import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@wealthfolio/ui";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Separator } from "@wealthfolio/ui/components/ui/separator";
import { useToast } from "@wealthfolio/ui/components/ui/use-toast";
import { useCallback, useEffect, useState } from "react";
import { SettingsHeader } from "../settings-header";

interface TokenStatus {
  active: boolean;
  expiresAt: string | null;
}

export default function RevolutSettingsPage() {
  const { toast } = useToast();
  const [tokenStatus, setTokenStatus] = useState<TokenStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isConnecting, setIsConnecting] = useState(false);
  const [isDisconnecting, setIsDisconnecting] = useState(false);

  const fetchTokenStatus = useCallback(async () => {
    try {
      const response = await fetch("/api/v1/dfc/revolut/token/status", {
        credentials: "same-origin",
      });
      if (!response.ok) {
        throw new Error("Failed to fetch token status");
      }
      const data = await response.json();
      setTokenStatus({
        active: data.active,
        expiresAt: data.expiresAt,
      });
    } catch (error) {
      console.error("Error fetching token status:", error);
      setTokenStatus({ active: false, expiresAt: null });
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchTokenStatus();
  }, [fetchTokenStatus]);

  const handleConnect = async () => {
    setIsConnecting(true);
    try {
      const response = await fetch("/api/v1/dfc/revolut/auth/url", {
        credentials: "same-origin",
      });
      if (!response.ok) {
        const data = await response.json().catch(() => ({}));
        throw new Error(data.message || "Failed to get auth URL");
      }
      const { url } = await response.json();
      window.location.href = url;
    } catch (error) {
      console.error("Error getting auth URL:", error);
      toast({
        title: "Erreur",
        description: error instanceof Error ? error.message : "Impossible d'initier la connexion",
        variant: "destructive",
      });
      setIsConnecting(false);
    }
  };

  const handleDisconnect = async () => {
    setIsDisconnecting(true);
    try {
      const response = await fetch("/api/v1/dfc/revolut/auth/token", {
        method: "DELETE",
        credentials: "same-origin",
      });
      if (!response.ok) {
        throw new Error("Failed to disconnect");
      }
      toast({
        title: "Déconnecté",
        description: "Votre compte Revolut a été déconnecté.",
      });
      setTokenStatus({ active: false, expiresAt: null });
    } catch (error) {
      console.error("Error disconnecting:", error);
      toast({
        title: "Erreur",
        description: "Impossible de déconnecter Revolut",
        variant: "destructive",
      });
    } finally {
      setIsDisconnecting(false);
    }
  };

  const formatExpiryDate = (dateStr: string | null) => {
    if (!dateStr) return null;
    try {
      const date = new Date(dateStr);
      return date.toLocaleDateString("fr-FR", {
        day: "numeric",
        month: "long",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    } catch {
      return dateStr;
    }
  };

  if (isLoading) {
    return (
      <div className="space-y-6">
        <SettingsHeader
          heading="Revolut Business"
          text="Connectez votre compte Revolut Business pour synchroniser vos transactions."
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
        heading="Revolut Business"
        text="Connectez votre compte Revolut Business pour synchroniser vos transactions."
      />
      <Separator />

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="bg-muted flex h-10 w-10 items-center justify-center rounded-lg">
                <Icons.CreditCard className="h-5 w-5" />
              </div>
              <div>
                <CardTitle className="text-base">Revolut Business</CardTitle>
                <CardDescription>API Open Banking</CardDescription>
              </div>
            </div>
            <Badge variant={tokenStatus?.active ? "default" : "secondary"}>
              {tokenStatus?.active ? "Connecté" : "Non connecté"}
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          {tokenStatus?.active ? (
            <>
              <div className="bg-muted/50 rounded-lg p-4">
                <div className="flex items-center gap-2 text-sm">
                  <Icons.CheckCircle className="text-green-500 h-4 w-4" />
                  <span className="text-muted-foreground">Token actif</span>
                </div>
                {tokenStatus.expiresAt && (
                  <p className="text-muted-foreground mt-2 text-xs">
                    Expire le {formatExpiryDate(tokenStatus.expiresAt)}
                  </p>
                )}
              </div>
              <div className="flex gap-2">
                <Button
                  variant="outline"
                  onClick={handleConnect}
                  disabled={isConnecting}
                >
                  {isConnecting && <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />}
                  Reconnecter
                </Button>
                <Button
                  variant="destructive"
                  onClick={handleDisconnect}
                  disabled={isDisconnecting}
                >
                  {isDisconnecting && <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />}
                  Déconnecter
                </Button>
              </div>
            </>
          ) : (
            <>
              <p className="text-muted-foreground text-sm">
                Connectez votre compte Revolut Business pour importer automatiquement vos
                transactions et synchroniser vos soldes.
              </p>
              <Button onClick={handleConnect} disabled={isConnecting}>
                {isConnecting && <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />}
                Connecter Revolut
              </Button>
            </>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
