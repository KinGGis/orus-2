/**
 * AccountingEntriesView - Journal entries table tab
 */
import { useMemo, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@wealthfolio/ui/components/ui/card";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Input } from "@wealthfolio/ui/components/ui/input";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { EmptyPlaceholder, AmountDisplay } from "@wealthfolio/ui";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@wealthfolio/ui/components/ui/table";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import { Search, Download } from "lucide-react";

import { useOrusAccountingEntries } from "@/features/orus-integration";

function EntriesTableSkeleton() {
  return (
    <div className="space-y-2">
      {[1, 2, 3, 4, 5].map((i) => (
        <Skeleton key={i} className="h-12 w-full" />
      ))}
    </div>
  );
}

export function AccountingEntriesView() {
  const { data: entries, isLoading, error } = useOrusAccountingEntries();
  const [searchQuery, setSearchQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<"all" | "validated" | "pending">("all");

  const filteredEntries = useMemo(() => {
    if (!entries) return [];

    let filtered = [...entries];

    // Apply search filter
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(
        (e) =>
          e.account_code.toLowerCase().includes(query) ||
          e.description?.toLowerCase().includes(query)
      );
    }

    // Apply status filter
    if (statusFilter === "validated") {
      filtered = filtered.filter((e) => e.validated);
    } else if (statusFilter === "pending") {
      filtered = filtered.filter((e) => !e.validated);
    }

    // Sort by date descending
    return filtered.sort((a, b) => new Date(b.entry_date).getTime() - new Date(a.entry_date).getTime());
  }, [entries, searchQuery, statusFilter]);

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

  if (!isLoading && !entries?.length) {
    return (
      <div className="flex items-center justify-center py-16">
        <EmptyPlaceholder
          icon={<Icons.Receipt className="text-muted-foreground h-10 w-10" />}
          title="Aucune écriture"
          description="Aucune écriture comptable n'a été trouvée."
        />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Filters bar */}
      <Card className="p-3">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="relative flex-1 max-w-sm">
            <Search className="text-muted-foreground absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2" />
            <Input
              placeholder="Rechercher..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-9"
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
              Validées
            </Button>
            <Button
              variant={statusFilter === "pending" ? "default" : "outline"}
              size="sm"
              onClick={() => setStatusFilter("pending")}
            >
              En attente
            </Button>
          </div>
        </div>
      </Card>

      {/* Entries table */}
      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="text-base">Écritures comptables</CardTitle>
              <CardDescription>
                {filteredEntries.length} écriture{filteredEntries.length > 1 ? "s" : ""} trouvée{filteredEntries.length > 1 ? "s" : ""}
              </CardDescription>
            </div>
            <Button variant="outline" size="sm">
              <Download className="mr-1.5 h-4 w-4" />
              Exporter
            </Button>
          </div>
        </CardHeader>
        <CardContent className="p-0">
          {isLoading ? (
            <div className="p-4">
              <EntriesTableSkeleton />
            </div>
          ) : (
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
                      <TableCell className="font-medium whitespace-nowrap">
                        {format(new Date(entry.entry_date), "dd MMM yyyy", { locale: fr })}
                      </TableCell>
                      <TableCell>
                        <code className="bg-muted rounded px-1.5 py-0.5 text-sm">{entry.account_code}</code>
                      </TableCell>
                      <TableCell className="max-w-xs truncate">{entry.description || "-"}</TableCell>
                      <TableCell className="text-right font-medium text-red-600">
                        {entry.debit ? <AmountDisplay value={entry.debit} currency={entry.currency} /> : "-"}
                      </TableCell>
                      <TableCell className="text-right font-medium text-green-600">
                        {entry.credit ? <AmountDisplay value={entry.credit} currency={entry.currency} /> : "-"}
                      </TableCell>
                      <TableCell>
                        <Badge variant={entry.validated ? "default" : "secondary"}>
                          {entry.validated ? "Validée" : "En attente"}
                        </Badge>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

export default AccountingEntriesView;
