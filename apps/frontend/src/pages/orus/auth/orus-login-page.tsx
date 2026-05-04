/**
 * Orus Login Page
 * Supports email/password and Google OAuth authentication
 */
import { useState, FormEvent } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { useOrusAuth } from "@/features/orus-integration/orus-auth-context";
import {
  ApplicationShell,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Input,
  Label,
  Icons,
  Alert,
  AlertDescription,
} from "@wealthfolio/ui";

export default function OrusLoginPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { signIn, signInWithGoogle, loading: authLoading } = useOrusAuth();

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Check for messages from redirect
  const message = searchParams.get("message");
  const errorParam = searchParams.get("error");

  const handleEmailLogin = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);

    console.log("[OrusLogin] Attempting login with email:", email);
    
    try {
      const { error: signInError } = await signIn(email, password);
      console.log("[OrusLogin] SignIn result:", signInError ? `Error: ${signInError.message}` : "Success");
      
      if (signInError) {
        console.error("[OrusLogin] Auth error details:", signInError);
        if (signInError.message.includes("Invalid login credentials")) {
          setError("Email ou mot de passe incorrect");
        } else if (signInError.message.includes("Email not confirmed")) {
          setError("Veuillez confirmer votre email avant de vous connecter");
        } else {
          setError(signInError.message);
        }
        return;
      }
      // Redirect to dashboard on success
      navigate("/administration");
    } catch (err) {
      setError("Une erreur est survenue. Veuillez réessayer.");
      console.error("[OrusLogin] Unexpected error:", err);
    } finally {
      setLoading(false);
    }
  };

  const handleGoogleLogin = async () => {
    setError(null);
    try {
      const { error: oauthError } = await signInWithGoogle();
      if (oauthError) {
        setError(oauthError.message);
      }
      // OAuth will redirect to callback URL
    } catch (err) {
      setError("Erreur lors de la connexion avec Google");
      console.error("Google login error:", err);
    }
  };

  if (authLoading) {
    return (
      <ApplicationShell className="fixed inset-0 flex items-center justify-center">
        <div className="flex flex-col items-center gap-4">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
          <p className="text-muted-foreground text-sm">Chargement...</p>
        </div>
      </ApplicationShell>
    );
  }

  return (
    <ApplicationShell className="fixed inset-0 flex items-center justify-center p-6 bg-gradient-to-b from-background to-muted/20">
      <div className="w-full max-w-md -translate-y-[5vh]">
        <Card className="w-full border shadow-lg">
          <CardHeader className="space-y-4 text-center">
            <div className="flex justify-center">
              <div className="flex h-16 w-16 items-center justify-center rounded-full bg-primary/10">
                <Icons.Briefcase className="h-8 w-8 text-primary" />
              </div>
            </div>
            <div className="space-y-2">
              <CardTitle className="text-2xl">Orus</CardTitle>
              <CardDescription>
                Connectez-vous pour accéder à votre espace
              </CardDescription>
            </div>
          </CardHeader>

          <CardContent className="space-y-6">
            {/* Success/Error Messages */}
            {message && (
              <Alert>
                <Icons.CheckCircle className="h-4 w-4" />
                <AlertDescription>{message}</AlertDescription>
              </Alert>
            )}
            {(error || errorParam) && (
              <Alert variant="destructive">
                <Icons.AlertCircle className="h-4 w-4" />
                <AlertDescription>{error || errorParam}</AlertDescription>
              </Alert>
            )}

            {/* Google Login Button */}
            <Button
              type="button"
              variant="outline"
              className="w-full h-12 gap-3"
              onClick={handleGoogleLogin}
              disabled={loading}
            >
              <svg className="h-5 w-5" viewBox="0 0 24 24">
                <path
                  d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
                  fill="#4285F4"
                />
                <path
                  d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                  fill="#34A853"
                />
                <path
                  d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
                  fill="#FBBC05"
                />
                <path
                  d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
                  fill="#EA4335"
                />
              </svg>
              Continuer avec Google
            </Button>

            <div className="relative">
              <div className="absolute inset-0 flex items-center">
                <span className="w-full border-t" />
              </div>
              <div className="relative flex justify-center text-xs uppercase">
                <span className="bg-background px-2 text-muted-foreground">
                  ou avec votre email
                </span>
              </div>
            </div>

            {/* Email/Password Form */}
            <form onSubmit={handleEmailLogin} className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="email">Email</Label>
                <Input
                  id="email"
                  type="email"
                  placeholder="nom@example.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  disabled={loading}
                  required
                  autoComplete="email"
                  className="h-12"
                />
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <Label htmlFor="password">Mot de passe</Label>
                  <Link
                    to="/forgot-password"
                    className="text-sm text-primary hover:underline"
                  >
                    Mot de passe oublié ?
                  </Link>
                </div>
                <Input
                  id="password"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  disabled={loading}
                  required
                  autoComplete="current-password"
                  className="h-12"
                />
              </div>

              <Button
                type="submit"
                className="w-full h-12"
                disabled={loading || !email || !password}
              >
                {loading ? (
                  <>
                    <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                    Connexion...
                  </>
                ) : (
                  "Se connecter"
                )}
              </Button>
            </form>
          </CardContent>

          <CardFooter className="flex flex-col gap-4 text-center">
            <p className="text-sm text-muted-foreground">
              Pas encore de compte ?{" "}
              <Link
                to="/signup"
                className="font-medium text-primary hover:underline"
              >
                Créer un compte
              </Link>
            </p>
            <p className="text-xs text-muted-foreground">
              En vous connectant, vous acceptez nos{" "}
              <a href="#" className="underline">
                conditions d'utilisation
              </a>{" "}
              et notre{" "}
              <a href="#" className="underline">
                politique de confidentialité
              </a>
              .
            </p>
          </CardFooter>
        </Card>
      </div>
    </ApplicationShell>
  );
}
