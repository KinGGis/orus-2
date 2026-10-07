import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { useToast } from "@wealthfolio/ui/components/ui/use-toast";
import { useEffect, useRef } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

/**
 * Completes the IBKR authorization by handing the code to the backend, which
 * performs the token exchange so the PKCE verifier never leaves the server.
 */
export default function IbkrCallbackPage() {
  const { toast } = useToast();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  // React 18 mounts effects twice in development; the authorization code is
  // single-use, so the exchange must not be attempted again.
  const exchanged = useRef(false);

  useEffect(() => {
    if (exchanged.current) return;
    exchanged.current = true;

    const code = searchParams.get("code");
    const state = searchParams.get("state");
    const error = searchParams.get("error");

    const finish = (title: string, description: string, failed: boolean) => {
      toast({
        title,
        description,
        ...(failed ? { variant: "destructive" as const } : {}),
      });
      navigate("/settings/ibkr");
    };

    if (error) {
      finish("Autorisation refusée", error, true);
      return;
    }
    if (!code || !state) {
      finish(
        "Autorisation incomplète",
        "IBKR n'a pas renvoyé de code d'autorisation.",
        true,
      );
      return;
    }

    fetch("/api/v1/dfc/ibkr/callback", {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ code, state }),
    })
      .then(async (response) => {
        if (!response.ok) throw new Error(await response.text());
        finish("IBKR connecté", "La synchronisation est maintenant possible.", false);
      })
      .catch((err) => {
        console.error("IBKR token exchange failed:", err);
        finish(
          "Connexion IBKR échouée",
          "L'échange du code d'autorisation a échoué.",
          true,
        );
      });
  }, [navigate, searchParams, toast]);

  return (
    <div className="flex min-h-screen flex-col items-center justify-center">
      <Icons.Spinner className="text-muted-foreground h-12 w-12 animate-spin" />
      <p className="text-muted-foreground mt-4">Connexion à IBKR en cours...</p>
    </div>
  );
}
