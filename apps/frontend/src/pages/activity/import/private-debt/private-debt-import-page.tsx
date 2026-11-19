import { useAccounts } from "@/hooks/use-accounts";
import { Page, PageContent, PageHeader } from "@wealthfolio/ui";
import { Alert, AlertDescription } from "@wealthfolio/ui/components/ui/alert";
import { Button } from "@wealthfolio/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui/components/ui/card";
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
import { useToast } from "@wealthfolio/ui/components/ui/use-toast";
import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  parseLoans,
  parseStatement,
  readSheet,
  type ImportedLoan,
  type ImportedTransaction,
  type ParseResult,
} from "./maclear-parser";

const PLATFORM = "MACLEAR";

interface ImportSummary {
  loansCreated: number;
  loansUpdated: number;
  loansUnchanged: number;
  activitiesCreated: number;
  activitiesSkipped: number;
  linesIgnored: number;
  warnings: string[];
}

async function readFile<T>(
  file: File,
  parse: (rows: ReturnType<typeof readSheet>) => ParseResult<T>,
): Promise<ParseResult<T>> {
  return parse(readSheet(await file.arrayBuffer()));
}

export default function PrivateDebtImportPage() {
  const navigate = useNavigate();
  const { toast } = useToast();
  const queryClient = useQueryClient();
  const { accounts } = useAccounts();
  const [accountId, setAccountId] = useState("");
  const [loans, setLoans] = useState<ParseResult<ImportedLoan> | null>(null);
  const [statement, setStatement] = useState<ParseResult<ImportedTransaction> | null>(null);
  const [isImporting, setIsImporting] = useState(false);
  const [summary, setSummary] = useState<ImportSummary | null>(null);

  const handleFile = async <T,>(
    file: File | undefined,
    parse: (rows: ReturnType<typeof readSheet>) => ParseResult<T>,
    set: (result: ParseResult<T> | null) => void,
  ) => {
    setSummary(null);
    if (!file) return set(null);
    try {
      set(await readFile(file, parse));
    } catch (error) {
      set({ rows: [], errors: [`Fichier illisible : ${String(error)}`] });
    }
  };

  const parseErrors = [...(loans?.errors ?? []), ...(statement?.errors ?? [])];
  const canImport =
    !!accountId &&
    (loans?.rows.length ?? 0) + (statement?.rows.length ?? 0) > 0 &&
    parseErrors.length === 0;

  const handleImport = async () => {
    setIsImporting(true);
    try {
      const response = await fetch("/api/v1/private-debt/import", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          accountId,
          platform: PLATFORM,
          loans: loans?.rows ?? [],
          transactions: statement?.rows ?? [],
        }),
      });
      if (!response.ok) {
        const text = (await response.text()).trim();
        let message = text || `Le serveur a répondu ${response.status}.`;
        try {
          message = (JSON.parse(text) as { message?: string }).message ?? message;
        } catch {
          // non-JSON error body
        }
        throw new Error(message);
      }
      setSummary((await response.json()) as ImportSummary);
      await queryClient.invalidateQueries();
      toast({ title: "Import terminé", description: "Le portefeuille est en cours de recalcul." });
    } catch (error) {
      toast({
        title: "Échec de l'import",
        description: error instanceof Error ? error.message : String(error),
        variant: "destructive",
      });
    } finally {
      setIsImporting(false);
    }
  };

  return (
    <Page>
      <PageHeader heading="Import dette privée (Maclear)" onBack={() => navigate(-1)} />
      <PageContent>
        <Card className="max-w-3xl">
          <CardHeader>
            <CardTitle>Fichiers Maclear</CardTitle>
            <CardDescription>
              La liste des prêts crée ou met à jour les prêts et leurs conditions ; le relevé de
              compte enregistre les flux réels. Un réimport ne duplique rien.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-5">
            <div className="space-y-2">
              <Label>Compte</Label>
              <Select value={accountId} onValueChange={setAccountId}>
                <SelectTrigger>
                  <SelectValue placeholder="Choisir le compte de dette privée" />
                </SelectTrigger>
                <SelectContent>
                  {accounts.map((account) => (
                    <SelectItem key={account.id} value={account.id}>
                      {account.name} ({account.currency})
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label htmlFor="pd-loans">Liste des prêts (conditions)</Label>
              <Input
                id="pd-loans"
                type="file"
                accept=".xlsx,.xls,.csv"
                onChange={(e) => handleFile(e.target.files?.[0], parseLoans, setLoans)}
              />
              {loans && (
                <p className="text-muted-foreground text-sm">{loans.rows.length} prêt(s) lu(s)</p>
              )}
            </div>

            <div className="space-y-2">
              <Label htmlFor="pd-statement">Relevé de compte</Label>
              <Input
                id="pd-statement"
                type="file"
                accept=".xlsx,.xls,.csv"
                onChange={(e) => handleFile(e.target.files?.[0], parseStatement, setStatement)}
              />
              {statement && (
                <p className="text-muted-foreground text-sm">
                  {statement.rows.length} ligne(s) lue(s)
                </p>
              )}
            </div>

            {parseErrors.length > 0 && (
              <Alert variant="destructive">
                <AlertDescription>
                  <ul className="list-disc pl-4">
                    {parseErrors.map((error) => (
                      <li key={error}>{error}</li>
                    ))}
                  </ul>
                </AlertDescription>
              </Alert>
            )}

            <Button onClick={handleImport} disabled={!canImport || isImporting}>
              {isImporting ? (
                <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <Icons.Import className="mr-2 h-4 w-4" />
              )}
              Importer
            </Button>

            {summary && (
              <Alert>
                <AlertDescription className="space-y-1">
                  <p>
                    Prêts : {summary.loansCreated} créé(s), {summary.loansUpdated} mis à jour,{" "}
                    {summary.loansUnchanged} inchangé(s).
                  </p>
                  <p>
                    Flux : {summary.activitiesCreated} enregistré(s), {summary.activitiesSkipped}{" "}
                    déjà présent(s), {summary.linesIgnored} ligne(s) ignorée(s).
                  </p>
                  {summary.warnings.length > 0 && (
                    <ul className="list-disc pl-4 text-amber-600">
                      {summary.warnings.map((warning) => (
                        <li key={warning}>{warning}</li>
                      ))}
                    </ul>
                  )}
                </AlertDescription>
              </Alert>
            )}
          </CardContent>
        </Card>
      </PageContent>
    </Page>
  );
}
