/**
 * Orus Auth Callback Page
 * Handles OAuth redirects (Google) and email verification links
 */
import { useEffect, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { orusSupabase } from "@/features/orus-integration/supabase-client";
import {
  ApplicationShell,
  Icons,
} from "@wealthfolio/ui";

export default function OrusAuthCallbackPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const handleCallback = async () => {
      try {
        // Check for error in URL params (OAuth errors)
        const errorParam = searchParams.get("error");
        const errorDescription = searchParams.get("error_description");

        if (errorParam) {
          setError(errorDescription || "Une erreur d'authentification s'est produite");
          setTimeout(() => {
            navigate(`/login?error=${encodeURIComponent(errorDescription || errorParam)}`);
          }, 2000);
          return;
        }

        // Get session from URL fragment (OAuth callback)
        // Supabase handles this automatically via onAuthStateChange
        const { data: { session }, error: sessionError } = await orusSupabase.auth.getSession();

        if (sessionError) {
          console.error("[OrusAuth] Session error:", sessionError);
          setError(sessionError.message);
          setTimeout(() => {
            navigate("/login?error=" + encodeURIComponent(sessionError.message));
          }, 2000);
          return;
        }

        if (session) {
          console.log("[OrusAuth] Session established, redirecting to dashboard");
          // Successful authentication, redirect to dashboard
          navigate("/");
        } else {
          // No session, might be email verification
          const type = searchParams.get("type");
          
          if (type === "recovery") {
            // Password reset flow
            navigate("/reset-password");
          } else if (type === "signup" || type === "email_change") {
            // Email verification successful
            navigate("/login?message=Email vérifié avec succès. Vous pouvez maintenant vous connecter.");
          } else {
            // Fallback - no session and unknown type
            navigate("/login");
          }
        }
      } catch (err) {
        console.error("[OrusAuth] Callback error:", err);
        setError("Une erreur est survenue lors de l'authentification");
        setTimeout(() => {
          navigate("/login?error=callback_error");
        }, 2000);
      }
    };

    handleCallback();
  }, [navigate, searchParams]);

  return (
    <ApplicationShell className="fixed inset-0 flex items-center justify-center">
      <div className="flex flex-col items-center gap-4 text-center">
        {error ? (
          <>
            <div className="flex h-16 w-16 items-center justify-center rounded-full bg-red-100">
              <Icons.AlertCircle className="h-8 w-8 text-red-600" />
            </div>
            <div className="space-y-2">
              <h2 className="text-lg font-semibold">Erreur d'authentification</h2>
              <p className="text-sm text-muted-foreground max-w-sm">{error}</p>
              <p className="text-xs text-muted-foreground">Redirection en cours...</p>
            </div>
          </>
        ) : (
          <>
            <div className="h-12 w-12 animate-spin rounded-full border-4 border-primary border-t-transparent" />
            <div className="space-y-2">
              <h2 className="text-lg font-semibold">Authentification en cours</h2>
              <p className="text-sm text-muted-foreground">Veuillez patienter...</p>
            </div>
          </>
        )}
      </div>
    </ApplicationShell>
  );
}
