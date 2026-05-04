/**
 * Orus Forgot Password Page
 * Send password reset email
 */
import { useState, FormEvent } from "react";
import { Link } from "react-router-dom";
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

export default function OrusForgotPasswordPage() {
  const { resetPassword } = useOrusAuth();

  const [email, setEmail] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);

    try {
      const { error: resetError } = await resetPassword(email);
      if (resetError) {
        setError(resetError.message);
        return;
      }
      setSuccess(true);
    } catch (err) {
      setError("Une erreur est survenue. Veuillez réessayer.");
      console.error("Reset password error:", err);
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
                  <Icons.Mail className="h-8 w-8 text-green-600" />
                </div>
              </div>
              <div className="space-y-2">
                <CardTitle className="text-2xl">Email envoyé</CardTitle>
                <CardDescription className="text-base">
                  Nous avons envoyé un lien de réinitialisation à{" "}
                  <span className="font-medium text-foreground">{email}</span>
                </CardDescription>
              </div>
            </CardHeader>

            <CardContent className="space-y-4">
              <p className="text-center text-sm text-muted-foreground">
                Cliquez sur le lien dans l'email pour réinitialiser votre mot de passe.
                Le lien expire dans 1 heure.
              </p>
              <div className="rounded-lg bg-muted p-4">
                <p className="text-sm text-muted-foreground">
                  <strong>Vous n'avez pas reçu l'email ?</strong>
                  <br />
                  Vérifiez votre dossier spam ou{" "}
                  <button
                    onClick={() => {
                      setSuccess(false);
                      setEmail("");
                    }}
                    className="text-primary hover:underline"
                  >
                    réessayez
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
      <div className="w-full max-w-md -translate-y-[5vh]">
        <Card className="w-full border shadow-lg">
          <CardHeader className="space-y-4 text-center">
            <div className="flex justify-center">
              <div className="flex h-16 w-16 items-center justify-center rounded-full bg-primary/10">
                <Icons.Key className="h-8 w-8 text-primary" />
              </div>
            </div>
            <div className="space-y-2">
              <CardTitle className="text-2xl">Mot de passe oublié ?</CardTitle>
              <CardDescription>
                Entrez votre email pour recevoir un lien de réinitialisation
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

              <Button
                type="submit"
                className="w-full h-12"
                disabled={loading || !email}
              >
                {loading ? (
                  <>
                    <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                    Envoi en cours...
                  </>
                ) : (
                  "Envoyer le lien"
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
