/**
 * Orus Reset Password Page
 * Set new password after clicking reset link
 */
import { useState, FormEvent, useEffect } from "react";
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

export default function OrusResetPasswordPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const { updatePassword, session } = useOrusAuth();

  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  // Check if we have a valid session (user clicked verification link)
  useEffect(() => {
    const errorParam = searchParams.get("error");
    const errorDescription = searchParams.get("error_description");
    
    if (errorParam) {
      setError(errorDescription || "Le lien de réinitialisation est invalide ou a expiré.");
    }
  }, [searchParams]);

  const validateForm = (): string | null => {
    if (password.length < 8) {
      return "Le mot de passe doit contenir au moins 8 caractères";
    }
    if (password !== confirmPassword) {
      return "Les mots de passe ne correspondent pas";
    }
    return null;
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);

    const validationError = validateForm();
    if (validationError) {
      setError(validationError);
      return;
    }

    setLoading(true);

    try {
      const { error: updateError } = await updatePassword(password);
      if (updateError) {
        setError(updateError.message);
        return;
      }
      setSuccess(true);
      // Redirect to login after 3 seconds
      setTimeout(() => {
        navigate("/login?message=Mot de passe modifié avec succès");
      }, 3000);
    } catch (err) {
      setError("Une erreur est survenue. Veuillez réessayer.");
      console.error("Update password error:", err);
    } finally {
      setLoading(false);
    }
  };

  // Success state
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
                <CardTitle className="text-2xl">Mot de passe modifié</CardTitle>
                <CardDescription className="text-base">
                  Votre mot de passe a été mis à jour avec succès
                </CardDescription>
              </div>
            </CardHeader>

            <CardContent className="space-y-4">
              <p className="text-center text-sm text-muted-foreground">
                Vous allez être redirigé vers la page de connexion...
              </p>
              <div className="flex justify-center">
                <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
              </div>
            </CardContent>

            <CardFooter className="flex justify-center">
              <Link
                to="/login"
                className="text-sm text-primary hover:underline"
              >
                Se connecter maintenant
              </Link>
            </CardFooter>
          </Card>
        </div>
      </ApplicationShell>
    );
  }

  // No session - show error
  if (!session && !searchParams.get("error")) {
    return (
      <ApplicationShell className="fixed inset-0 flex items-center justify-center p-6 bg-gradient-to-b from-background to-muted/20">
        <div className="w-full max-w-md -translate-y-[5vh]">
          <Card className="w-full border shadow-lg">
            <CardHeader className="space-y-4 text-center">
              <div className="flex justify-center">
                <div className="flex h-16 w-16 items-center justify-center rounded-full bg-red-100">
                  <Icons.AlertCircle className="h-8 w-8 text-red-600" />
                </div>
              </div>
              <div className="space-y-2">
                <CardTitle className="text-2xl">Lien invalide</CardTitle>
                <CardDescription className="text-base">
                  Ce lien de réinitialisation est invalide ou a expiré.
                </CardDescription>
              </div>
            </CardHeader>

            <CardContent>
              <p className="text-center text-sm text-muted-foreground">
                Veuillez demander un nouveau lien de réinitialisation.
              </p>
            </CardContent>

            <CardFooter className="flex justify-center">
              <Link to="/forgot-password">
                <Button>Demander un nouveau lien</Button>
              </Link>
            </CardFooter>
          </Card>
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
                <Icons.Key className="h-8 w-8 text-primary" />
              </div>
            </div>
            <div className="space-y-2">
              <CardTitle className="text-2xl">Nouveau mot de passe</CardTitle>
              <CardDescription>
                Choisissez un nouveau mot de passe sécurisé
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

            <form onSubmit={handleSubmit} className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="password">Nouveau mot de passe</Label>
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
                disabled={loading || !password || !confirmPassword}
              >
                {loading ? (
                  <>
                    <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                    Mise à jour...
                  </>
                ) : (
                  "Mettre à jour le mot de passe"
                )}
              </Button>
            </form>
          </CardContent>

          <CardFooter className="flex justify-center">
            <Link
              to="/login"
              className="text-sm text-muted-foreground hover:text-primary"
            >
              ← Retour à la connexion
            </Link>
          </CardFooter>
        </Card>
      </div>
    </ApplicationShell>
  );
}
