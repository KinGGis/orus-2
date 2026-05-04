/**
 * PEInvestmentsView - Private Equity investments list
 */
import { useMemo, useState } from "react";
import { Card } from "@wealthfolio/ui/components/ui/card";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Input } from "@wealthfolio/ui/components/ui/input";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { EmptyPlaceholder, AmountDisplay } from "@wealthfolio/ui";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import { Search, TrendingUp, Download, Calendar, Building2, Percent } from "lucide-react";

import { useOrusPECompanies, useOrusPEInvestments } from "@/features/orus-integration";
import type { OrusPECompany, OrusPEInvestment } from "@/features/orus-integration";

interface InvestmentWithCompany extends OrusPEInvestment {
  company?: OrusPECompany;
}

function InvestmentRow({ investment, company }: { investment: OrusPEInvestment; company?: OrusPECompany }) {
  return (
    <div className="flex items-center justify-between rounded-lg border p-4 hover:bg-muted/50 transition-colors">
      <div className="flex items-start gap-4 min-w-0">
        <div className="bg-muted flex h-10 w-10 items-center justify-center rounded-lg shrink-0">
          <Building2 className="text-muted-foreground h-5 w-5" />
        </div>
        <div className="min-w-0">
          <p className="font-medium truncate">{company?.name || "N/A"}</p>
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Calendar className="h-3 w-3" />
            <span>{format(new Date(investment.investment_date), "d MMM yyyy", { locale: fr })}</span>
            <Badge variant="outline" className="text-xs ml-1">{investment.investment_type || "N/A"}</Badge>
          </div>
        </div>
      </div>
      <div className="flex items-center gap-6 shrink-0">
        <div className="text-right">
          <p className="font-semibold">
            <AmountDisplay value={investment.amount} currency={investment.currency} />
          </p>
          {investment.ownership_percentage && (
            <div className="flex items-center justify-end gap-1 text-sm text-muted-foreground">
              <Percent className="h-3 w-3" />
              <span>{investment.ownership_percentage.toFixed(2)}%</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function InvestmentRowSkeleton() {
  return (
    <div className="flex items-center justify-between rounded-lg border p-4">
      <div className="flex items-center gap-4">
        <Skeleton className="h-10 w-10 rounded-lg" />
        <div className="space-y-2">
          <Skeleton className="h-4 w-32" />
          <Skeleton className="h-3 w-24" />
        </div>
      </div>
      <Skeleton className="h-5 w-20" />
    </div>
  );
}

export function PEInvestmentsView() {
  const { data: companies, isLoading: companiesLoading, error: companiesError } = useOrusPECompanies();
  const { data: investments, isLoading: investmentsLoading, error: investmentsError } = useOrusPEInvestments();

  const [searchQuery, setSearchQuery] = useState("");
  const [sortBy, setSortBy] = useState<"date" | "amount">("date");
  const [sortOrder, setSortOrder] = useState<"asc" | "desc">("desc");

  const isLoading = companiesLoading || investmentsLoading;
  const error = companiesError || investmentsError;

  const companiesMap = useMemo(() => {
    if (!companies) return new Map<string, OrusPECompany>();
    return new Map(companies.map((c) => [c.id, c]));
  }, [companies]);

  const sortedAndFilteredInvestments = useMemo(() => {
    if (!investments) return [];

    // Enrich with company data
    let enriched: InvestmentWithCompany[] = investments.map((inv) => ({
      ...inv,
      company: companiesMap.get(inv.company_id),
    }));

    // Apply search
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      enriched = enriched.filter(
        (inv) =>
          inv.company?.name.toLowerCase().includes(query) ||
          inv.investment_type?.toLowerCase().includes(query)
      );
    }

    // Apply sort
    enriched.sort((a, b) => {
      let comparison = 0;
      if (sortBy === "date") {
        comparison = new Date(a.investment_date).getTime() - new Date(b.investment_date).getTime();
      } else {
        comparison = a.amount - b.amount;
      }
      return sortOrder === "asc" ? comparison : -comparison;
    });

    return enriched;
  }, [investments, companiesMap, searchQuery, sortBy, sortOrder]);

  const totalValue = useMemo(() => {
    return sortedAndFilteredInvestments.reduce((sum, inv) => sum + inv.amount, 0);
  }, [sortedAndFilteredInvestments]);

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

  if (!isLoading && !investments?.length) {
    return (
      <div className="flex items-center justify-center py-16">
        <EmptyPlaceholder
          icon={<TrendingUp className="text-muted-foreground h-10 w-10" />}
          title="Aucun investissement"
          description="Ajoutez votre premier investissement PE dans l'application Orus."
        />
      </div>
    );
  }

  const toggleSort = (field: "date" | "amount") => {
    if (sortBy === field) {
      setSortOrder(sortOrder === "asc" ? "desc" : "asc");
    } else {
      setSortBy(field);
      setSortOrder("desc");
    }
  };

  return (
    <div className="space-y-4">
      {/* Filters bar */}
      <Card className="p-3">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="relative flex-1 max-w-sm">
            <Search className="text-muted-foreground absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2" />
            <Input
              placeholder="Rechercher un investissement..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-9"
            />
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant={sortBy === "date" ? "default" : "outline"}
              size="sm"
              onClick={() => toggleSort("date")}
            >
              Date {sortBy === "date" && (sortOrder === "desc" ? "↓" : "↑")}
            </Button>
            <Button
              variant={sortBy === "amount" ? "default" : "outline"}
              size="sm"
              onClick={() => toggleSort("amount")}
            >
              Montant {sortBy === "amount" && (sortOrder === "desc" ? "↓" : "↑")}
            </Button>
            <Button variant="outline" size="sm" className="hidden sm:flex">
              <Download className="h-4 w-4 mr-1" />
              Exporter
            </Button>
          </div>
        </div>
      </Card>

      {/* Summary */}
      <div className="flex items-center justify-between">
        <p className="text-muted-foreground text-sm">
          {sortedAndFilteredInvestments.length} investissement{sortedAndFilteredInvestments.length > 1 ? "s" : ""} trouvé{sortedAndFilteredInvestments.length > 1 ? "s" : ""}
        </p>
        <p className="text-sm font-medium">
          Total: <AmountDisplay value={totalValue} currency="EUR" />
        </p>
      </div>

      {/* Investments list */}
      {isLoading ? (
        <div className="space-y-3">
          {[1, 2, 3, 4, 5].map((i) => (
            <InvestmentRowSkeleton key={i} />
          ))}
        </div>
      ) : (
        <div className="space-y-3">
          {sortedAndFilteredInvestments.map((investment) => (
            <InvestmentRow key={investment.id} investment={investment} company={investment.company} />
          ))}
        </div>
      )}
    </div>
  );
}

export default PEInvestmentsView;
