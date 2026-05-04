/**
 * Orus Signup Page
 * User registration with email/password
 */
import { useState, FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";
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

export default function OrusSignupPage() {
  const navigate = useNavigate();
  const { signUp, signInWithGoogle, loading: authLoading } = useOrusAuth();

  const [fullName, setFullName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const validateForm = (): string | null => {
    if (!fullName.trim()) {
      return "Veuillez entrer votre nom complet";
    }
    if (!email.trim()) {
      return "Veuillez entrer votre email";
    }
    if (password.length < 8) {
      return "Le mot de passe doit contenir au moins 8 caractères";
    }
    if (password !== confirmPassword) {
      return "Les mots de passe ne correspondent pas";
    }
    return null;
  };

  const handleSignup = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);

    const validationError = validateForm();
    if (validationError) {
      setError(validationError);
      return;
    }

    setLoading(true);

    try {
      const { error: signUpError } = await signUp(email, password, fullName);
      if (signUpError) {
        if (signUpError.message.includes("already registered")) {
          setError("Cet email est déjà utilisé. Essayez de vous connecter.");
        } else {
          setError(signUpError.message);
        }
        return;
      }
      setSuccess(true);
    } catch (err) {
      setError("Une erreur est survenue. Veuillez réessayer.");
      console.error("Signup error:", err);
    } finally {
      setLoading(false);
    }
  };

  const handleGoogleSignup = async () => {
    setError(null);
    try {
      const { error: oauthError } = await signInWithGoogle();
      if (oauthError) {
        setError(oauthError.message);
      }
    } catch (err) {
      setError("Erreur lors de l'inscription avec Google");
      console.error("Google signup error:", err);
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

  // Success state - show confirmation message
  if (success) {
    return (
      <ApplicationShell className="fixed inset-0 flex items-center justify-center p-6 bg-gradient-to-b from-background to-muted/20">
        <div className="w-full max-w-md -translate-y-[5vh]">
          <Card className="w-full border shadow-lg">
            <CardHeader className="space-y-4 text-center">
              <div className="flex justify-center">
                <div className="flex h-16 w-16 items-center justify-center rounded-full bg-green-100">
                  <Icons.CheckCircle className="h-8 w-8 text-green-600" />
                </div>
              </div>
              <div className="space-y-2">
                <CardTitle className="text-2xl">Vérifiez votre email</CardTitle>
                <CardDescription className="text-base">
                  Nous avons envoyé un lien de confirmation à{" "}
                  <span className="font-medium text-foreground">{email}</span>
                </CardDescription>
              </div>
            </CardHeader>

            <CardContent className="space-y-4">
              <p className="text-center text-sm text-muted-foreground">
                Cliquez sur le lien dans l'email pour activer votre compte. 
                Le lien expire dans 24 heures.
              </p>
              <div className="rounded-lg bg-muted p-4">
                <p className="text-sm text-muted-foreground">
                  <strong>Vous n'avez pas reçu l'email ?</strong>
                  <br />
                  Vérifiez votre dossier spam ou{" "}
                  <button
                    onClick={() => setSuccess(false)}
                    className="text-primary hover:underline"
                  >
                    réessayez avec un autre email
                  </button>
                </p>
              </div>
            </CardContent>

            <CardFooter className="flex justify-center">
              <Link
                to="/login"
                className="text-sm text-primary hover:underline"
              >
                ← Retour à la connexion
              </Link>
            </CardFooter>
          </Card>
        </div>
      </ApplicationShell>
    );
  }

  return (
    <ApplicationShell className="fixed inset-0 flex items-center justify-center p-6 bg-gradient-to-b from-background to-muted/20">
      <div className="w-full max-w-md">
        <Card className="w-full border shadow-lg">
          <CardHeader className="space-y-4 text-center">
            <div className="flex justify-center">
              <div className="flex h-16 w-16 items-center justify-center rounded-full bg-primary/10">
                <Icons.Briefcase className="h-8 w-8 text-primary" />
              </div>
            </div>
            <div className="space-y-2">
              <CardTitle className="text-2xl">Créer un compte</CardTitle>
              <CardDescription>
                Rejoignez Orus
              </CardDescription>
            </div>
          </CardHeader>

          <CardContent className="space-y-6">
            {error && (
              <Alert variant="destructive">
                <Icons.AlertCircle className="h-4 w-4" />
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            )}

            {/* Google Signup Button */}
            <Button
              type="button"
              variant="outline"
              className="w-full h-12 gap-3"
              onClick={handleGoogleSignup}
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
              S'inscrire avec Google
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
            <form onSubmit={handleSignup} className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="fullName">Nom complet</Label>
                <Input
                  id="fullName"
                  type="text"
                  placeholder="Jean Dupont"
                  value={fullName}
                  onChange={(e) => setFullName(e.target.value)}
                  disabled={loading}
                  required
                  autoComplete="name"
                  className="h-12"
                />
              </div>

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
                <Label htmlFor="password">Mot de passe</Label>
                <Input
                  id="password"
                  type="password"
                  placeholder="8 caractères minimum"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  disabled={loading}
                  required
                  autoComplete="new-password"
                  className="h-12"
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="confirmPassword">Confirmer le mot de passe</Label>
                <Input
                  id="confirmPassword"
                  type="password"
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  disabled={loading}
                  required
                  autoComplete="new-password"
                  className="h-12"
                />
              </div>

              <Button
                type="submit"
                className="w-full h-12"
                disabled={loading || !fullName || !email || !password || !confirmPassword}
              >
                {loading ? (
                  <>
                    <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                    Création du compte...
                  </>
                ) : (
                  "Créer mon compte"
                )}
              </Button>
            </form>
          </CardContent>

          <CardFooter className="flex flex-col gap-4 text-center">
            <p className="text-sm text-muted-foreground">
              Déjà un compte ?{" "}
              <Link
                to="/login"
                className="font-medium text-primary hover:underline"
              >
                Se connecter
              </Link>
            </p>
            <p className="text-xs text-muted-foreground">
              En créant un compte, vous acceptez nos{" "}
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
