/**
 * AccountingOverviewView - Complete accounting/comptabilité view
 * Features: Periods, Financial Statements (Bilan, P&L, Balance), Thot Assistant, Entries
 */
import { useState, useMemo, useRef, useCallback } from "react";
import { streamChatResponse } from "@/features/ai-assistant";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui/components/ui/card";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Input } from "@wealthfolio/ui/components/ui/input";
import { Label } from "@wealthfolio/ui/components/ui/label";
// import { Textarea } from "@wealthfolio/ui/components/ui/textarea";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { AmountDisplay, EmptyPlaceholder } from "@wealthfolio/ui";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@wealthfolio/ui/components/ui/tabs";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@wealthfolio/ui/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@wealthfolio/ui/components/ui/table";
import { ScrollArea } from "@wealthfolio/ui/components/ui/scroll-area";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import {
  Calendar,
  Plus,
  FileText,
  Brain,
  Send,
  Upload,
  Sparkles,
  // MessageCircle,
  TrendingUp,
  // TrendingDown,
  Search,
  Download,
  CheckCircle,
  Clock,
  Scale,
} from "lucide-react";
import { toast } from "sonner";

import { 
  useOrusAccountingEntries, 
  useOrusAccountingPeriods,
  useOrusChartOfAccounts,
} from "@/features/orus-integration";
import type { 
  OrusAccountingEntry, 
  OrusChartOfAccounts,
} from "@/features/orus-integration";

// =====================================================
// TYPES
// =====================================================

interface AccountingPeriod {
  id: string;
  name: string;
  startDate: string;
  endDate: string;
  isClosed: boolean;
}

interface FinancialStatementLine {
  code: string;
  label: string;
  amount: number;
  level: number;
  isTotal?: boolean;
}

interface ThotMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  timestamp: Date;
}

// =====================================================
// DEMO DATA
// =====================================================

const DEMO_PERIODS: AccountingPeriod[] = [
  { id: "2024", name: "Exercice 2024", startDate: "2024-01-01", endDate: "2024-12-31", isClosed: false },
  { id: "2023", name: "Exercice 2023", startDate: "2023-01-01", endDate: "2023-12-31", isClosed: true },
  { id: "2022", name: "Exercice 2022", startDate: "2022-01-01", endDate: "2022-12-31", isClosed: true },
];

const DEMO_BILAN: FinancialStatementLine[] = [
  { code: "ACTIF", label: "ACTIF", amount: 0, level: 0, isTotal: true },
  { code: "AI", label: "Actif Immobilisé", amount: 450000, level: 1 },
  { code: "AI1", label: "Immobilisations incorporelles", amount: 50000, level: 2 },
  { code: "AI2", label: "Immobilisations corporelles", amount: 350000, level: 2 },
  { code: "AI3", label: "Immobilisations financières", amount: 50000, level: 2 },
  { code: "AC", label: "Actif Circulant", amount: 280000, level: 1 },
  { code: "AC1", label: "Stocks", amount: 80000, level: 2 },
  { code: "AC2", label: "Créances clients", amount: 120000, level: 2 },
  { code: "AC3", label: "Disponibilités", amount: 80000, level: 2 },
  { code: "TOTAL_ACTIF", label: "TOTAL ACTIF", amount: 730000, level: 0, isTotal: true },
  { code: "PASSIF", label: "PASSIF", amount: 0, level: 0, isTotal: true },
  { code: "CP", label: "Capitaux Propres", amount: 520000, level: 1 },
  { code: "CP1", label: "Capital social", amount: 200000, level: 2 },
  { code: "CP2", label: "Réserves", amount: 180000, level: 2 },
  { code: "CP3", label: "Résultat de l'exercice", amount: 140000, level: 2 },
  { code: "DT", label: "Dettes", amount: 210000, level: 1 },
  { code: "DT1", label: "Dettes financières", amount: 100000, level: 2 },
  { code: "DT2", label: "Dettes fournisseurs", amount: 80000, level: 2 },
  { code: "DT3", label: "Dettes fiscales et sociales", amount: 30000, level: 2 },
  { code: "TOTAL_PASSIF", label: "TOTAL PASSIF", amount: 730000, level: 0, isTotal: true },
];

const DEMO_PNL: FinancialStatementLine[] = [
  { code: "CA", label: "Chiffre d'affaires", amount: 850000, level: 0 },
  { code: "PS", label: "Production stockée", amount: 5000, level: 1 },
  { code: "PI", label: "Production immobilisée", amount: 0, level: 1 },
  { code: "SE", label: "Subventions d'exploitation", amount: 10000, level: 1 },
  { code: "PROD", label: "TOTAL PRODUITS", amount: 865000, level: 0, isTotal: true },
  { code: "ACH", label: "Achats consommés", amount: 320000, level: 1 },
  { code: "SE_CHG", label: "Services extérieurs", amount: 150000, level: 1 },
  { code: "PERS", label: "Charges de personnel", amount: 180000, level: 1 },
  { code: "IMP", label: "Impôts et taxes", amount: 25000, level: 1 },
  { code: "DAP", label: "Dotations aux amortissements", amount: 35000, level: 1 },
  { code: "AUT", label: "Autres charges", amount: 15000, level: 1 },
  { code: "CHG", label: "TOTAL CHARGES", amount: 725000, level: 0, isTotal: true },
  { code: "REX", label: "RÉSULTAT D'EXPLOITATION", amount: 140000, level: 0, isTotal: true },
];

const QUICK_QUESTIONS = [
  "Quelle est la rentabilité de l'exercice ?",
  "Analyser le BFR",
  "Identifier les postes à risque",
  "Préparer la clôture",
];

// =====================================================
// HELPER FUNCTIONS
// =====================================================

const formatCurrency = (value: number, currency = "EUR"): string => {
  return new Intl.NumberFormat("fr-FR", {
    style: "currency",
    currency,
    maximumFractionDigits: 0,
  }).format(value);
};

// =====================================================
// SUB-COMPONENTS
// =====================================================

function PeriodSelector({
  periods,
  selectedPeriod,
  onSelectPeriod,
  onCreatePeriod,
}: {
  periods: AccountingPeriod[];
  selectedPeriod: AccountingPeriod | null;
  onSelectPeriod: (period: AccountingPeriod) => void;
  onCreatePeriod: (period: Omit<AccountingPeriod, "id">) => void;
}) {
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [newPeriod, setNewPeriod] = useState({
    name: "",
    startDate: "",
    endDate: "",
  });

  const handleCreate = () => {
    onCreatePeriod({
      name: newPeriod.name || `Exercice ${new Date().getFullYear()}`,
      startDate: newPeriod.startDate,
      endDate: newPeriod.endDate,
      isClosed: false,
    });
    setIsDialogOpen(false);
    setNewPeriod({ name: "", startDate: "", endDate: "" });
    toast.success("Période créée");
  };

  return (
    <Card className="bg-card/50 border-border/50">
      <CardContent className="pt-4">
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <Calendar className="h-5 w-5 text-muted-foreground" />
            <div>
              <p className="text-sm font-medium">Période comptable</p>
              <p className="text-xs text-muted-foreground">
                {selectedPeriod
                  ? `${format(new Date(selectedPeriod.startDate), "dd/MM/yyyy")} - ${format(new Date(selectedPeriod.endDate), "dd/MM/yyyy")}`
                  : "Aucune période sélectionnée"}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Select
              value={selectedPeriod?.id || ""}
              onValueChange={(value) => {
                const period = periods.find((p) => p.id === value);
                if (period) onSelectPeriod(period);
              }}
            >
              <SelectTrigger className="w-48">
                <SelectValue placeholder="Sélectionner une période" />
              </SelectTrigger>
              <SelectContent>
                {periods.map((period) => (
                  <SelectItem key={period.id} value={period.id}>
                    <div className="flex items-center gap-2">
                      <span>{period.name}</span>
                      {period.isClosed && (
                        <Badge variant="secondary" className="text-xs">
                          Clôturé
                        </Badge>
                      )}
                    </div>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Dialog open={isDialogOpen} onOpenChange={setIsDialogOpen}>
              <DialogTrigger asChild>
                <Button variant="outline" size="icon">
                  <Plus className="h-4 w-4" />
                </Button>
              </DialogTrigger>
              <DialogContent>
                <DialogHeader>
                  <DialogTitle>Nouvelle période comptable</DialogTitle>
                  <DialogDescription>
                    Créez une nouvelle période pour votre exercice comptable
                  </DialogDescription>
                </DialogHeader>
                <div className="space-y-4 py-4">
                  <div className="space-y-2">
                    <Label>Nom de la période</Label>
                    <Input
                      placeholder="Ex: Exercice 2024"
                      value={newPeriod.name}
                      onChange={(e) =>
                        setNewPeriod({ ...newPeriod, name: e.target.value })
                      }
                    />
                  </div>
                  <div className="grid grid-cols-2 gap-4">
                    <div className="space-y-2">
                      <Label>Date de début</Label>
                      <Input
                        type="date"
                        value={newPeriod.startDate}
                        onChange={(e) =>
                          setNewPeriod({ ...newPeriod, startDate: e.target.value })
                        }
                      />
                    </div>
                    <div className="space-y-2">
                      <Label>Date de fin</Label>
                      <Input
                        type="date"
                        value={newPeriod.endDate}
                        onChange={(e) =>
                          setNewPeriod({ ...newPeriod, endDate: e.target.value })
                        }
                      />
                    </div>
                  </div>
                </div>
                <DialogFooter>
                  <Button variant="outline" onClick={() => setIsDialogOpen(false)}>
                    Annuler
                  </Button>
                  <Button onClick={handleCreate}>Créer</Button>
                </DialogFooter>
              </DialogContent>
            </Dialog>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

function FinancialStatementTable({
  title,
  lines,
}: {
  title: string;
  lines: FinancialStatementLine[];
}) {
  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader className="pb-2">
        <CardTitle className="text-base">{title}</CardTitle>
      </CardHeader>
      <CardContent className="p-0">
        <ScrollArea className="h-[400px]">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Code</TableHead>
                <TableHead>Libellé</TableHead>
                <TableHead className="text-right">Montant</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {lines.map((line) => (
                <TableRow
                  key={line.code}
                  className={line.isTotal ? "bg-muted/50 font-semibold" : ""}
                >
                  <TableCell
                    className="text-xs"
                    style={{ paddingLeft: `${line.level * 16 + 16}px` }}
                  >
                    {line.code}
                  </TableCell>
                  <TableCell className={line.isTotal ? "font-semibold" : ""}>
                    {line.label}
                  </TableCell>
                  <TableCell className="text-right">
                    {line.amount !== 0 && formatCurrency(line.amount)}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </ScrollArea>
      </CardContent>
    </Card>
  );
}

const THOT_SYSTEM_PROMPT = `Tu es Thot, un assistant comptable expert français. Tu aides les utilisateurs à:
- Analyser leurs états financiers (bilan, compte de résultat, balance)
- Préparer les clôtures comptables
- Comprendre les écritures comptables
- Donner des conseils de gestion financière
Réponds toujours en français de manière professionnelle mais accessible.`;

function ThotAssistant() {
  const [messages, setMessages] = useState<ThotMessage[]>([
    {
      id: "1",
      role: "assistant",
      content:
        "Bonjour ! Je suis Thot, votre assistant comptable. Je peux vous aider à analyser vos états financiers, préparer la clôture, ou répondre à vos questions comptables.",
      timestamp: new Date(),
    },
  ]);
  const [input, setInput] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [streamingContent, setStreamingContent] = useState("");
  const abortControllerRef = useRef<AbortController | null>(null);
  const threadIdRef = useRef<string | null>(null);

  const handleSend = useCallback(async () => {
    if (!input.trim() || isLoading) return;

    const userMessage: ThotMessage = {
      id: crypto.randomUUID(),
      role: "user",
      content: input,
      timestamp: new Date(),
    };
    setMessages((prev) => [...prev, userMessage]);
    const userInput = input;
    setInput("");
    setIsLoading(true);
    setStreamingContent("");

    // Create abort controller for cancellation
    abortControllerRef.current = new AbortController();
    const { signal } = abortControllerRef.current;

    // Prepare message with accounting context
    const contextualMessage = `[Contexte: Module comptable Orus - Assistant Thot]
${THOT_SYSTEM_PROMPT}

Question de l'utilisateur: ${userInput}`;

    let accumulatedContent = "";
    let messageId: string = crypto.randomUUID();

    try {
      for await (const event of streamChatResponse(
        {
          content: contextualMessage,
          threadId: threadIdRef.current ?? undefined,
        },
        signal,
      )) {
        if (signal.aborted) break;

        switch (event.type) {
          case "system":
            // Capture thread ID for conversation continuity
            if (event.threadId) {
              threadIdRef.current = event.threadId;
            }
            if (event.messageId) {
              messageId = event.messageId;
            }
            break;

          case "textDelta":
            accumulatedContent += event.delta;
            setStreamingContent(accumulatedContent);
            break;

          case "error":
            throw new Error(event.message || "Erreur de l'assistant");

          case "done":
            // Finalize the message
            const aiMessage: ThotMessage = {
              id: messageId,
              role: "assistant",
              content: accumulatedContent || "Je n'ai pas pu générer de réponse.",
              timestamp: new Date(),
            };
            setMessages((prev) => [...prev, aiMessage]);
            setStreamingContent("");
            break;
        }
      }
    } catch (error) {
      // Handle abort or other errors
      if (signal.aborted) {
        // User cancelled - don't show error
        setStreamingContent("");
      } else {
        const errorMessage: ThotMessage = {
          id: crypto.randomUUID(),
          role: "assistant",
          content: `Désolé, une erreur s'est produite: ${error instanceof Error ? error.message : "Erreur inconnue"}. Veuillez réessayer.`,
          timestamp: new Date(),
        };
        setMessages((prev) => [...prev, errorMessage]);
      }
    } finally {
      setIsLoading(false);
      setStreamingContent("");
      abortControllerRef.current = null;
    }
  }, [input, isLoading]);

  const handleCancel = useCallback(() => {
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }
  }, []);

  const handleQuickQuestion = (question: string) => {
    setInput(question);
  };

  return (
    <Card className="bg-card/50 border-border/50 h-full flex flex-col">
      <CardHeader className="pb-2">
        <CardTitle className="text-base flex items-center gap-2">
          <Brain className="h-5 w-5 text-primary" />
          Assistant Thot
        </CardTitle>
        <CardDescription>
          IA comptable pour l'analyse de vos états financiers
        </CardDescription>
      </CardHeader>
      <CardContent className="flex-1 flex flex-col space-y-4">
        {/* Quick questions */}
        <div className="flex flex-wrap gap-2">
          {QUICK_QUESTIONS.map((question) => (
            <Button
              key={question}
              variant="outline"
              size="sm"
              className="text-xs"
              onClick={() => handleQuickQuestion(question)}
            >
              <Sparkles className="h-3 w-3 mr-1" />
              {question}
            </Button>
          ))}
        </div>

        {/* Messages */}
        <ScrollArea className="flex-1 pr-4">
          <div className="space-y-4">
            {messages.map((message) => (
              <div
                key={message.id}
                className={`flex ${message.role === "user" ? "justify-end" : "justify-start"}`}
              >
                <div
                  className={`max-w-[85%] rounded-lg p-3 text-sm ${
                    message.role === "user"
                      ? "bg-primary text-primary-foreground"
                      : "bg-muted"
                  }`}
                >
                  <p className="whitespace-pre-wrap">{message.content}</p>
                </div>
              </div>
            ))}
            {/* Streaming content */}
            {isLoading && streamingContent && (
              <div className="flex justify-start">
                <div className="bg-muted max-w-[85%] rounded-lg p-3 text-sm">
                  <p className="whitespace-pre-wrap">{streamingContent}</p>
                  <span className="inline-block w-2 h-4 bg-primary animate-pulse ml-0.5" />
                </div>
              </div>
            )}
            {/* Loading indicator when no content yet */}
            {isLoading && !streamingContent && (
              <div className="flex justify-start">
                <div className="bg-muted rounded-lg p-3">
                  <div className="flex items-center gap-2">
                    <div className="h-2 w-2 bg-primary rounded-full animate-bounce" />
                    <div className="h-2 w-2 bg-primary rounded-full animate-bounce [animation-delay:100ms]" />
                    <div className="h-2 w-2 bg-primary rounded-full animate-bounce [animation-delay:200ms]" />
                  </div>
                </div>
              </div>
            )}
          </div>
        </ScrollArea>

        {/* Input */}
        <div className="flex items-center gap-2">
          <Button variant="outline" size="icon" disabled={isLoading}>
            <Upload className="h-4 w-4" />
          </Button>
          <Input
            placeholder="Posez votre question comptable..."
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && !e.shiftKey && handleSend()}
            disabled={isLoading}
          />
          {isLoading ? (
            <Button variant="destructive" onClick={handleCancel}>
              <Icons.X className="h-4 w-4" />
            </Button>
          ) : (
            <Button onClick={handleSend} disabled={!input.trim()}>
              <Send className="h-4 w-4" />
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function EntriesSection({ entries }: { entries: OrusAccountingEntry[] }) {
  const [searchQuery, setSearchQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<"all" | "validated" | "pending">("all");

  const filteredEntries = useMemo(() => {
    let filtered = [...entries];

    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(
        (e) =>
          e.account_code.toLowerCase().includes(query) ||
          e.description?.toLowerCase().includes(query)
      );
    }

    if (statusFilter === "validated") {
      filtered = filtered.filter((e) => e.validated);
    } else if (statusFilter === "pending") {
      filtered = filtered.filter((e) => !e.validated);
    }

    return filtered
      .sort((a, b) => new Date(b.entry_date).getTime() - new Date(a.entry_date).getTime())
      .slice(0, 10);
  }, [entries, searchQuery, statusFilter]);

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="text-base">Écritures comptables</CardTitle>
            <CardDescription>Dernières écritures de la période</CardDescription>
          </div>
          <Button variant="outline" size="sm">
            <Download className="mr-1.5 h-4 w-4" />
            Exporter
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        {/* Filters */}
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between mb-4">
          <div className="relative flex-1 max-w-sm">
            <Search className="text-muted-foreground absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2" />
            <Input
              placeholder="Rechercher..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-9 h-9"
            />
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant={statusFilter === "all" ? "default" : "outline"}
              size="sm"
              onClick={() => setStatusFilter("all")}
            >
              Toutes
            </Button>
            <Button
              variant={statusFilter === "validated" ? "default" : "outline"}
              size="sm"
              onClick={() => setStatusFilter("validated")}
            >
              <CheckCircle className="h-3 w-3 mr-1" />
              Validées
            </Button>
            <Button
              variant={statusFilter === "pending" ? "default" : "outline"}
              size="sm"
              onClick={() => setStatusFilter("pending")}
            >
              <Clock className="h-3 w-3 mr-1" />
              En attente
            </Button>
          </div>
        </div>

        {/* Table */}
        <div className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Date</TableHead>
                <TableHead>Compte</TableHead>
                <TableHead>Description</TableHead>
                <TableHead className="text-right">Débit</TableHead>
                <TableHead className="text-right">Crédit</TableHead>
                <TableHead>Statut</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filteredEntries.map((entry) => (
                <TableRow key={entry.id}>
                  <TableCell className="whitespace-nowrap text-sm">
                    {format(new Date(entry.entry_date), "dd/MM/yy", { locale: fr })}
                  </TableCell>
                  <TableCell>
                    <code className="bg-muted rounded px-1.5 py-0.5 text-xs">
                      {entry.account_code}
                    </code>
                  </TableCell>
                  <TableCell className="max-w-xs truncate text-sm">
                    {entry.description || "-"}
                  </TableCell>
                  <TableCell className="text-right text-sm font-medium text-red-600">
                    {entry.debit ? (
                      <AmountDisplay value={entry.debit} currency={entry.currency} />
                    ) : (
                      "-"
                    )}
                  </TableCell>
                  <TableCell className="text-right text-sm font-medium text-green-600">
                    {entry.credit ? (
                      <AmountDisplay value={entry.credit} currency={entry.currency} />
                    ) : (
                      "-"
                    )}
                  </TableCell>
                  <TableCell>
                    <Badge variant={entry.validated ? "default" : "secondary"} className="text-xs">
                      {entry.validated ? "Validée" : "En attente"}
                    </Badge>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>
  );
}

// =====================================================
// MAIN COMPONENT
// =====================================================

export function AccountingOverviewView() {
  // Fetch real data from Orus Supabase
  const { data: periodsData, isLoading: periodsLoading } = useOrusAccountingPeriods();
  const { data: chartOfAccounts } = useOrusChartOfAccounts();
  
  // Convert OrusAccountingPeriod to local AccountingPeriod format
  const periods = useMemo<AccountingPeriod[]>(() => {
    if (!periodsData?.length) {
      return DEMO_PERIODS;
    }
    return periodsData.map((p) => ({
      id: p.id,
      name: p.name,
      startDate: p.start_date,
      endDate: p.end_date,
      isClosed: p.status === "closed",
    }));
  }, [periodsData]);

  const [selectedPeriodId, setSelectedPeriodId] = useState<string | null>(null);
  const selectedPeriod = useMemo(() => {
    if (selectedPeriodId) {
      return periods.find((p) => p.id === selectedPeriodId) || periods[0];
    }
    return periods[0];
  }, [selectedPeriodId, periods]);

  // Fetch entries for the selected period
  const { data: entries, isLoading: entriesLoading, error } = useOrusAccountingEntries(
    selectedPeriod?.id
  );

  const isLoading = periodsLoading || entriesLoading;

  const [activeStatement, setActiveStatement] = useState("bilan");

  const handleCreatePeriod = (_newPeriod: Omit<AccountingPeriod, "id">) => {
    // In a real implementation, this would call a mutation to create the period
    toast.info("Création de période: à implémenter avec la mutation Supabase");
  };

  const handleSelectPeriod = (period: AccountingPeriod) => {
    setSelectedPeriodId(period.id);
  };

  // Calculate summary from entries
  const summary = useMemo(() => {
    if (!entries)
      return { totalDebit: 0, totalCredit: 0, validatedCount: 0, pendingCount: 0 };

    const totalDebit = entries.reduce((sum, e) => sum + (e.debit || 0), 0);
    const totalCredit = entries.reduce((sum, e) => sum + (e.credit || 0), 0);
    const validatedCount = entries.filter((e) => e.validated).length;
    const pendingCount = entries.length - validatedCount;

    return { totalDebit, totalCredit, validatedCount, pendingCount };
  }, [entries]);

  // Generate financial statements from entries and chart of accounts
  const { bilanLines, pnlLines } = useMemo(() => {
    if (!entries?.length || !chartOfAccounts?.length) {
      return { bilanLines: DEMO_BILAN, pnlLines: DEMO_PNL };
    }

    // Group entries by account code
    const balanceByAccount: Record<string, { debit: number; credit: number }> = {};
    entries.forEach((entry) => {
      if (!balanceByAccount[entry.account_code]) {
        balanceByAccount[entry.account_code] = { debit: 0, credit: 0 };
      }
      balanceByAccount[entry.account_code].debit += entry.debit || 0;
      balanceByAccount[entry.account_code].credit += entry.credit || 0;
    });

    // Build bilan from chart of accounts
    const assetAccounts = chartOfAccounts.filter((a) => a.type === "asset");
    const liabilityAccounts = chartOfAccounts.filter((a) => a.type === "liability");
    const equityAccounts = chartOfAccounts.filter((a) => a.type === "equity");
    const incomeAccounts = chartOfAccounts.filter((a) => a.type === "income");
    const expenseAccounts = chartOfAccounts.filter((a) => a.type === "expense");

    const buildLines = (accounts: OrusChartOfAccounts[], sectionLabel: string): FinancialStatementLine[] => {
      const lines: FinancialStatementLine[] = [];
      let total = 0;
      
      accounts.forEach((account) => {
        const balance = balanceByAccount[account.code];
        if (balance) {
          const amount = balance.debit - balance.credit;
          total += amount;
          lines.push({
            code: account.code,
            label: account.name,
            amount: Math.abs(amount),
            level: account.parent_id ? 2 : 1,
          });
        }
      });

      return [
        { code: sectionLabel, label: sectionLabel, amount: total, level: 0, isTotal: true },
        ...lines.sort((a, b) => a.code.localeCompare(b.code)),
      ];
    };

    const bilanLines: FinancialStatementLine[] = [
      ...buildLines(assetAccounts, "ACTIF"),
      ...buildLines([...liabilityAccounts, ...equityAccounts], "PASSIF"),
    ];

    const pnlLines: FinancialStatementLine[] = [
      ...buildLines(incomeAccounts, "PRODUITS"),
      ...buildLines(expenseAccounts, "CHARGES"),
    ];

    // Use demo data if generated lines are empty
    if (bilanLines.length <= 2) {
      return { bilanLines: DEMO_BILAN, pnlLines: DEMO_PNL };
    }

    return { bilanLines, pnlLines };
  }, [entries, chartOfAccounts]);

  if (error) {
    return (
      <div className="flex items-center justify-center py-16">
        <EmptyPlaceholder
          icon={<Icons.AlertCircle className="text-destructive h-10 w-10" />}
          title="Erreur de chargement"
          description={(error as Error).message}
        />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Period selector */}
      <PeriodSelector
        periods={periods}
        selectedPeriod={selectedPeriod}
        onSelectPeriod={handleSelectPeriod}
        onCreatePeriod={handleCreatePeriod}
      />

      {/* Summary cards */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        <Card className="bg-card/50 border-border/50">
          <CardContent className="pt-4">
            <p className="text-xs text-muted-foreground uppercase">Total Débits</p>
            <p className="text-xl font-bold text-red-600 mt-1">
              {isLoading ? (
                <Skeleton className="h-6 w-24" />
              ) : (
                <AmountDisplay value={summary.totalDebit} currency="EUR" />
              )}
            </p>
          </CardContent>
        </Card>
        <Card className="bg-card/50 border-border/50">
          <CardContent className="pt-4">
            <p className="text-xs text-muted-foreground uppercase">Total Crédits</p>
            <p className="text-xl font-bold text-green-600 mt-1">
              {isLoading ? (
                <Skeleton className="h-6 w-24" />
              ) : (
                <AmountDisplay value={summary.totalCredit} currency="EUR" />
              )}
            </p>
          </CardContent>
        </Card>
        <Card className="bg-card/50 border-border/50">
          <CardContent className="pt-4">
            <p className="text-xs text-muted-foreground uppercase">Balance</p>
            <p
              className={`text-xl font-bold mt-1 ${
                summary.totalCredit - summary.totalDebit >= 0
                  ? "text-green-600"
                  : "text-red-600"
              }`}
            >
              {isLoading ? (
                <Skeleton className="h-6 w-24" />
              ) : (
                <AmountDisplay
                  value={summary.totalCredit - summary.totalDebit}
                  currency="EUR"
                />
              )}
            </p>
          </CardContent>
        </Card>
        <Card className="bg-card/50 border-border/50">
          <CardContent className="pt-4">
            <p className="text-xs text-muted-foreground uppercase">Écritures</p>
            <div className="flex items-center gap-2 mt-1">
              {isLoading ? (
                <Skeleton className="h-6 w-16" />
              ) : (
                <>
                  <span className="text-xl font-bold">{entries?.length || 0}</span>
                  <Badge variant="secondary" className="text-xs">
                    {summary.pendingCount} en attente
                  </Badge>
                </>
              )}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* Financial statements + Thot Assistant */}
      <div className="grid grid-cols-1 lg:grid-cols-5 gap-4">
        {/* Financial statements (60%) */}
        <div className="lg:col-span-3 space-y-4">
          <Tabs value={activeStatement} onValueChange={setActiveStatement}>
            <TabsList className="grid w-full grid-cols-3">
              <TabsTrigger value="bilan" className="flex items-center gap-2">
                <FileText className="h-4 w-4" />
                Bilan
              </TabsTrigger>
              <TabsTrigger value="pnl" className="flex items-center gap-2">
                <TrendingUp className="h-4 w-4" />
                Compte de Résultat
              </TabsTrigger>
              <TabsTrigger value="balance" className="flex items-center gap-2">
                <Scale className="h-4 w-4" />
                Balance
              </TabsTrigger>
            </TabsList>
            <TabsContent value="bilan">
              <FinancialStatementTable title="Bilan" lines={bilanLines} />
            </TabsContent>
            <TabsContent value="pnl">
              <FinancialStatementTable title="Compte de Résultat" lines={pnlLines} />
            </TabsContent>
            <TabsContent value="balance">
              <Card className="bg-card/50 border-border/50">
                <CardHeader>
                  <CardTitle className="text-base">Balance Générale</CardTitle>
                  <CardDescription>
                    Balance des comptes pour la période sélectionnée
                  </CardDescription>
                </CardHeader>
                <CardContent>
                  {isLoading ? (
                    <div className="space-y-2">
                      {[1, 2, 3, 4, 5].map((i) => (
                        <Skeleton key={i} className="h-10 w-full" />
                      ))}
                    </div>
                  ) : (
                    <ScrollArea className="h-[400px]">
                      <Table>
                        <TableHeader>
                          <TableRow>
                            <TableHead>Compte</TableHead>
                            <TableHead className="text-right">Débit</TableHead>
                            <TableHead className="text-right">Crédit</TableHead>
                            <TableHead className="text-right">Solde</TableHead>
                          </TableRow>
                        </TableHeader>
                        <TableBody>
                          {entries?.slice(0, 20).map((entry) => (
                            <TableRow key={entry.id}>
                              <TableCell>
                                <code className="bg-muted rounded px-1.5 py-0.5 text-xs">
                                  {entry.account_code}
                                </code>
                              </TableCell>
                              <TableCell className="text-right text-red-600">
                                {entry.debit ? formatCurrency(entry.debit) : "-"}
                              </TableCell>
                              <TableCell className="text-right text-green-600">
                                {entry.credit ? formatCurrency(entry.credit) : "-"}
                              </TableCell>
                              <TableCell className="text-right font-medium">
                                {formatCurrency((entry.credit || 0) - (entry.debit || 0))}
                              </TableCell>
                            </TableRow>
                          ))}
                        </TableBody>
                      </Table>
                    </ScrollArea>
                  )}
                </CardContent>
              </Card>
            </TabsContent>
          </Tabs>
        </div>

        {/* Thot Assistant (40%) */}
        <div className="lg:col-span-2">
          <ThotAssistant />
        </div>
      </div>

      {/* Entries section */}
      {entries && <EntriesSection entries={entries} />}
    </div>
  );
}

export default AccountingOverviewView;
