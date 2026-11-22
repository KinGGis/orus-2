import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Button } from "@wealthfolio/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui";
import { Alert, AlertDescription } from "@wealthfolio/ui/components/ui/alert";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Input } from "@wealthfolio/ui/components/ui/input";
import { Label } from "@wealthfolio/ui/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui/components/ui/select";
import { Separator } from "@wealthfolio/ui/components/ui/separator";
import { useToast } from "@wealthfolio/ui/components/ui/use-toast";
import { useCallback, useEffect, useState } from "react";
import { SettingsHeader } from "../settings-header";

type McpScope = "READ" | "WRITE";

interface McpApiKey {
  id: string;
  name: string;
  keyPrefix: string;
  scope: McpScope;
  createdAt: string;
  lastUsedAt: string | null;
  revokedAt: string | null;
}

interface McpCall {
  id: string;
  keyId: string | null;
  toolName: string;
  success: boolean;
  error: string | null;
  durationMs: number;
  createdAt: string;
}

const API = "/api/v1/mcp";

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${API}${path}`, {
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    ...init,
  });
  if (!response.ok) {
    const body = (await response.text()).trim();
    throw new Error(body && !body.startsWith("<") ? body : `Le serveur a répondu ${response.status}.`);
  }
  return response.status === 204 ? (undefined as T) : response.json();
}

function formatDate(value: string | null): string {
  return value ? new Date(value).toLocaleString("fr-FR") : "jamais";
}

export default function McpSettingsPage() {
  const { toast } = useToast();
  const [keys, setKeys] = useState<McpApiKey[]>([]);
  const [calls, setCalls] = useState<McpCall[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [name, setName] = useState("");
  const [scope, setScope] = useState<McpScope>("READ");
  const [isCreating, setIsCreating] = useState(false);
  const [secret, setSecret] = useState<string | null>(null);

  const endpoint = `${window.location.origin}${API}`;

  const refresh = useCallback(async () => {
    try {
      const [keyList, callList] = await Promise.all([
        request<McpApiKey[]>("/keys"),
        request<McpCall[]>("/calls?limit=20"),
      ]);
      setKeys(keyList);
      setCalls(callList);
    } catch (error) {
      toast({
        title: "Impossible de charger les clés MCP",
        description: error instanceof Error ? error.message : undefined,
        variant: "destructive",
      });
    } finally {
      setIsLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const copy = async (text: string) => {
    await navigator.clipboard.writeText(text);
    toast({ title: "Copié dans le presse-papiers" });
  };

  const handleCreate = async () => {
    setIsCreating(true);
    try {
      const created = await request<{ key: McpApiKey; secret: string }>("/keys", {
        method: "POST",
        body: JSON.stringify({ name: name.trim(), scope }),
      });
      setSecret(created.secret);
      setName("");
      await refresh();
    } catch (error) {
      toast({
        title: "La création de la clé a échoué",
        description: error instanceof Error ? error.message : undefined,
        variant: "destructive",
      });
    } finally {
      setIsCreating(false);
    }
  };

  const handleRevoke = async (key: McpApiKey) => {
    if (!window.confirm(`Révoquer la clé « ${key.name} » ? Les agents qui l'utilisent perdront l'accès.`)) {
      return;
    }
    try {
      await request(`/keys/${key.id}`, { method: "DELETE" });
      await refresh();
      toast({ title: "Clé révoquée" });
    } catch (error) {
      toast({
        title: "La révocation a échoué",
        description: error instanceof Error ? error.message : undefined,
        variant: "destructive",
      });
    }
  };

  const keyNames = new Map(keys.map((key) => [key.id, key.name]));
  const configExample = JSON.stringify(
    {
      mcpServers: {
        orus: {
          command: "npx",
          args: ["-y", "mcp-remote", endpoint, "--header", "Authorization:Bearer ${ORUS_API_KEY}"],
          env: { ORUS_API_KEY: secret ?? "orus_…" },
        },
      },
    },
    null,
    2,
  );

  if (isLoading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Icons.Spinner className="text-muted-foreground h-8 w-8 animate-spin" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <SettingsHeader
        heading="Agents IA (MCP)"
        text="Connectez vos agents IA à Orus via le Model Context Protocol : lecture du portefeuille, requêtes IBKR et opérations."
      />
      <Separator />

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Nouvelle clé d'API</CardTitle>
          <CardDescription>
            Une clé en lecture consulte les données. Une clé en écriture peut aussi créer des activités et des
            comptes, lancer des synchronisations, importer des relevés et passer des ordres IBKR.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap items-end gap-3">
            <div className="min-w-56 flex-1 space-y-1">
              <Label htmlFor="mcp-key-name">Nom</Label>
              <Input
                id="mcp-key-name"
                placeholder="Claude Desktop"
                value={name}
                onChange={(event) => setName(event.target.value)}
              />
            </div>
            <div className="w-44 space-y-1">
              <Label>Portée</Label>
              <Select value={scope} onValueChange={(value) => setScope(value as McpScope)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="READ">Lecture</SelectItem>
                  <SelectItem value="WRITE">Écriture</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <Button onClick={handleCreate} disabled={isCreating || !name.trim()}>
              {isCreating ? (
                <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <Icons.Plus className="mr-2 h-4 w-4" />
              )}
              Créer
            </Button>
          </div>

          {secret && (
            <Alert>
              <Icons.ShieldAlert className="h-4 w-4" />
              <AlertDescription className="space-y-2">
                <p>Copiez cette clé maintenant : elle ne sera plus jamais affichée.</p>
                <div className="flex items-center gap-2">
                  <code className="bg-muted flex-1 overflow-x-auto rounded px-2 py-1 text-xs">{secret}</code>
                  <Button size="sm" variant="outline" onClick={() => copy(secret)}>
                    <Icons.Copy className="h-4 w-4" />
                  </Button>
                </div>
              </AlertDescription>
            </Alert>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Clés</CardTitle>
        </CardHeader>
        <CardContent className="space-y-2">
          {keys.length === 0 && <p className="text-muted-foreground text-sm">Aucune clé pour l'instant.</p>}
          {keys.map((key) => (
            <div key={key.id} className="flex items-center justify-between gap-3 rounded-lg border p-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span className="font-medium">{key.name}</span>
                  <Badge variant={key.scope === "WRITE" ? "default" : "secondary"}>
                    {key.scope === "WRITE" ? "Écriture" : "Lecture"}
                  </Badge>
                  {key.revokedAt && <Badge variant="destructive">Révoquée</Badge>}
                </div>
                <p className="text-muted-foreground text-xs">
                  {key.keyPrefix}… · créée le {formatDate(key.createdAt)} · dernière utilisation :{" "}
                  {formatDate(key.lastUsedAt)}
                </p>
              </div>
              {!key.revokedAt && (
                <Button size="sm" variant="outline" onClick={() => handleRevoke(key)}>
                  <Icons.Trash className="mr-2 h-4 w-4" />
                  Révoquer
                </Button>
              )}
            </div>
          ))}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Connecter un agent</CardTitle>
          <CardDescription>
            Point d'accès MCP (Streamable HTTP), authentifié par l'en-tête{" "}
            <code>Authorization: Bearer orus_…</code>.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex items-center gap-2">
            <code className="bg-muted flex-1 overflow-x-auto rounded px-2 py-1 text-xs">{endpoint}</code>
            <Button size="sm" variant="outline" onClick={() => copy(endpoint)}>
              <Icons.Copy className="h-4 w-4" />
            </Button>
          </div>
          <p className="text-muted-foreground text-sm">
            Claude Desktop (<code>claude_desktop_config.json</code>) ou Cursor (<code>.cursor/mcp.json</code>) :
          </p>
          <div className="relative">
            <pre className="bg-muted overflow-x-auto rounded p-3 text-xs">{configExample}</pre>
            <Button
              size="sm"
              variant="ghost"
              className="absolute right-2 top-2"
              onClick={() => copy(configExample)}
            >
              <Icons.Copy className="h-4 w-4" />
            </Button>
          </div>
          <p className="text-muted-foreground text-xs">
            Les clients compatibles HTTP (Cursor, agents maison) peuvent aussi utiliser directement l'URL avec
            l'en-tête, sans passer par <code>mcp-remote</code>.
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Derniers appels</CardTitle>
          <CardDescription>Journal des outils appelés par les agents.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-1">
          {calls.length === 0 && <p className="text-muted-foreground text-sm">Aucun appel pour l'instant.</p>}
          {calls.map((call) => (
            <div key={call.id} className="flex items-center gap-3 text-sm">
              {call.success ? (
                <Icons.CheckCircle className="h-4 w-4 shrink-0 text-green-500" />
              ) : (
                <Icons.AlertCircle className="text-destructive h-4 w-4 shrink-0" />
              )}
              <span className="font-mono">{call.toolName}</span>
              <span className="text-muted-foreground truncate">
                {(call.keyId && keyNames.get(call.keyId)) ?? "clé supprimée"} · {formatDate(call.createdAt)} ·{" "}
                {call.durationMs} ms{call.error ? ` · ${call.error}` : ""}
              </span>
            </div>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}
