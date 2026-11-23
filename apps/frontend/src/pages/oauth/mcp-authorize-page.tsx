import { Button } from "@wealthfolio/ui/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@wealthfolio/ui";
import { Alert, AlertDescription } from "@wealthfolio/ui/components/ui/alert";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Label } from "@wealthfolio/ui/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui/components/ui/select";
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";

type McpScope = "READ" | "WRITE";

const API = "/api/v1/mcp/oauth";

async function readError(response: Response): Promise<string> {
  const body = (await response.text()).trim();
  return body && !body.startsWith("<") ? body : `Le serveur a répondu ${response.status}.`;
}

function withParams(uri: string, params: Record<string, string>): string {
  const query = new URLSearchParams(params).toString();
  return `${uri}${uri.includes("?") ? "&" : "?"}${query}`;
}

/** OAuth consent screen for MCP clients (claude.ai connectors, etc.). */
export default function McpAuthorizePage() {
  const [params] = useSearchParams();
  const clientId = params.get("client_id") ?? "";
  const redirectUri = params.get("redirect_uri") ?? "";
  const state = params.get("state") ?? "";
  const requestedScope = params.get("scope")?.toUpperCase();

  const [clientName, setClientName] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scope, setScope] = useState<McpScope>(requestedScope === "WRITE" ? "WRITE" : "READ");
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (params.get("response_type") !== "code" || !clientId || !redirectUri) {
      setError("Requête d'autorisation invalide.");
      return;
    }
    const query = new URLSearchParams({ clientId, redirectUri }).toString();
    fetch(`${API}/client?${query}`, { credentials: "same-origin" })
      .then(async (response) => {
        if (!response.ok) throw new Error(await readError(response));
        const body = (await response.json()) as { clientName: string };
        setClientName(body.clientName);
      })
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
  }, [params, clientId, redirectUri]);

  const approve = async () => {
    setSubmitting(true);
    setError(null);
    try {
      const response = await fetch(`${API}/approve`, {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          clientId,
          redirectUri,
          codeChallenge: params.get("code_challenge") ?? "",
          codeChallengeMethod: params.get("code_challenge_method"),
          state,
          scope,
        }),
      });
      if (!response.ok) throw new Error(await readError(response));
      const { redirectUrl } = (await response.json()) as { redirectUrl: string };
      window.location.href = redirectUrl;
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setSubmitting(false);
    }
  };

  const deny = () => {
    const denied: Record<string, string> = { error: "access_denied" };
    if (state) denied.state = state;
    window.location.href = withParams(redirectUri, denied);
  };

  let redirectHost = redirectUri;
  try {
    redirectHost = new URL(redirectUri).host;
  } catch {
    // Displayed as-is.
  }

  return (
    <div className="bg-background flex min-h-screen items-center justify-center p-6">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icons.ShieldCheck className="h-5 w-5" />
            Connecter un agent IA à Orus
          </CardTitle>
          <CardDescription>
            {clientName ? (
              <>
                <strong>{clientName}</strong> demande l'accès à Orus. Après autorisation, vous serez
                renvoyé vers <strong>{redirectHost}</strong>.
              </>
            ) : (
              !error && "Vérification de la demande…"
            )}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {error && (
            <Alert variant="destructive">
              <Icons.AlertCircle className="h-4 w-4" />
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}
          {clientName && (
            <>
              <div className="space-y-2">
                <Label>Niveau d'accès</Label>
                <Select value={scope} onValueChange={(value) => setScope(value as McpScope)}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="READ">Lecture seule</SelectItem>
                    <SelectItem value="WRITE">Lecture et écriture</SelectItem>
                  </SelectContent>
                </Select>
                <p className="text-muted-foreground text-sm">
                  {scope === "WRITE"
                    ? "L'agent pourra aussi créer ou modifier des activités, lancer des synchronisations, des imports et des ordres IBKR."
                    : "L'agent pourra consulter les données du portefeuille et d'IBKR, sans rien modifier."}
                </p>
              </div>
              <p className="text-muted-foreground text-sm">
                Une clé MCP sera créée ; vous pourrez la révoquer dans Réglages &gt; Agents IA (MCP).
              </p>
              <div className="flex justify-end gap-2">
                <Button variant="outline" onClick={deny} disabled={submitting}>
                  Refuser
                </Button>
                <Button onClick={approve} disabled={submitting}>
                  {submitting && <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />}
                  Autoriser
                </Button>
              </div>
            </>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
