import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { useToast } from "@wealthfolio/ui/components/ui/use-toast";
import { useEffect, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

/**
 * Revolut OAuth callback page.
 * Parses ?code= and ?state= from URL, exchanges for token, then redirects.
 */
export default function RevolutCallbackPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { toast } = useToast();
  const [error, setError] = useState<string | null>(null);
  const [isProcessing, setIsProcessing] = useState(true);

  useEffect(() => {
    const code = searchParams.get("code");
    const state = searchParams.get("state");

    if (!code) {
      setError("Code d'autorisation manquant");
      setIsProcessing(false);
      return;
    }

    const exchangeToken = async () => {
      try {
        const response = await fetch("/api/v1/dfc/revolut/auth/callback", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ code, state: state || "" }),
          credentials: "same-origin",
        });

        if (!response.ok) {
          const data = await response.json().catch(() => ({}));
          throw new Error(data.message || `Erreur ${response.status}`);
        }

        toast({
          title: "Revolut connecté",
          description: "Votre compte Revolut Business est maintenant lié.",
        });

        navigate("/settings/revolut", { replace: true });
      } catch (err) {
        console.error("Revolut auth callback failed:", err);
        setError(err instanceof Error ? err.message : "Échec de l'authentification");
        setIsProcessing(false);
      }
    };

    exchangeToken();
  }, [searchParams, navigate, toast]);

  if (error) {
    return (
      <div className="bg-background text-foreground flex min-h-screen flex-col items-center justify-center">
        <div className="flex flex-col items-center gap-4 max-w-md text-center">
          <div className="bg-destructive/10 rounded-full p-3">
            <Icons.XCircle className="text-destructive h-8 w-8" />
          </div>
          <h2 className="text-lg font-semibold">Échec de la connexion Revolut</h2>
          <p className="text-muted-foreground text-sm">{error}</p>
          <button
            onClick={() => navigate("/settings/revolut")}
            className="text-primary hover:underline text-sm"
          >
            Retourner aux paramètres Revolut
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-background text-foreground flex min-h-screen flex-col items-center justify-center">
      <div className="flex flex-col items-center gap-4">
        <Icons.Spinner className="text-muted-foreground h-8 w-8 animate-spin" />
        <p className="text-muted-foreground text-sm">
          {isProcessing ? "Connexion à Revolut en cours..." : "Redirection..."}
        </p>
      </div>
    </div>
  );
}
