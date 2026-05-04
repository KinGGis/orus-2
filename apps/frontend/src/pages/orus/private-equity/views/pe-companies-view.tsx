/**
 * PECompaniesView - Private Equity companies list
 */
import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@wealthfolio/ui/components/ui/card";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Input } from "@wealthfolio/ui/components/ui/input";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { EmptyPlaceholder, AmountDisplay } from "@wealthfolio/ui";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import { Search, Building2, MapPin, Calendar, Users, Briefcase, Play } from "lucide-react";

import { useOrusPECompanies, useOrusPEInvestments } from "@/features/orus-integration";
import type { OrusPECompany, OrusPEInvestment } from "@/features/orus-integration";

function CompanyCard({
  company,
  investments,
  onSimulate,
}: {
  company: OrusPECompany;
  investments: OrusPEInvestment[];
  onSimulate: (companyId: string) => void;
}) {
  const companyInvestments = investments.filter((inv) => inv.company_id === company.id);
  const totalInvested = companyInvestments.reduce((sum, inv) => sum + inv.amount, 0);
  const latestInvestment = companyInvestments.sort(
    (a, b) => new Date(b.investment_date).getTime() - new Date(a.investment_date).getTime()
  )[0];

  return (
    <Card className="overflow-hidden hover:shadow-md transition-shadow">
      <CardHeader className="pb-3">
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-3">
            <div className="bg-muted flex h-12 w-12 items-center justify-center rounded-lg">
              <Building2 className="text-muted-foreground h-6 w-6" />
            </div>
            <div>
              <CardTitle className="text-lg">{company.name}</CardTitle>
              <CardDescription className="line-clamp-1">{company.description || "Pas de description"}</CardDescription>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant={company.is_cfo_managed ? "default" : "secondary"}>
              {company.is_cfo_managed ? "CFO" : "Externe"}
            </Badge>
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Company details */}
        <div className="grid grid-cols-2 gap-3 text-sm">
          <div className="flex items-center gap-2">
            <MapPin className="text-muted-foreground h-4 w-4" />
            <span>{company.country || "N/A"}</span>
          </div>
          <div className="flex items-center gap-2">
            <Calendar className="text-muted-foreground h-4 w-4" />
            <span>{company.founded_year || "N/A"}</span>
          </div>
          <div className="flex items-center gap-2">
            <Users className="text-muted-foreground h-4 w-4" />
            <span>{company.employees_count?.toLocaleString() || "N/A"} employés</span>
          </div>
          <div className="flex items-center gap-2">
            <Briefcase className="text-muted-foreground h-4 w-4" />
            <span className="truncate">{company.business_model || "N/A"}</span>
          </div>
        </div>

        {/* Investment summary */}
        <div className="border-t pt-4">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-muted-foreground text-xs uppercase tracking-wider">Total investi</p>
              <p className="text-xl font-bold">
                <AmountDisplay value={totalInvested} currency={company.currency || "EUR"} />
              </p>
            </div>
            {latestInvestment && (
              <div className="text-right">
                <p className="text-muted-foreground text-xs uppercase tracking-wider">Participation</p>
                <p className="text-lg font-semibold">{latestInvestment.ownership_percentage?.toFixed(1)}%</p>
              </div>
            )}
          </div>
        </div>

        {/* Investment rounds */}
        {companyInvestments.length > 0 && (
          <div className="border-t pt-4 space-y-2">
            <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
              Tours de table ({companyInvestments.length})
            </p>
            {companyInvestments.slice(0, 3).map((inv) => (
              <div key={inv.id} className="flex items-center justify-between text-sm rounded-lg bg-muted/50 p-2">
                <div className="flex items-center gap-2">
                  <Badge variant="outline" className="text-xs">{inv.investment_type || "N/A"}</Badge>
                  <span className="text-muted-foreground">
                    {format(new Date(inv.investment_date), "MMM yyyy", { locale: fr })}
                  </span>
                </div>
                <span className="font-medium">
                  <AmountDisplay value={inv.amount} currency={inv.currency} />
                </span>
              </div>
            ))}
          </div>
        )}

        {/* Simulate button */}
        <div className="border-t pt-4">
          <Button
            variant="outline"
            size="sm"
            className="w-full"
            onClick={() => onSimulate(company.id)}
          >
            <Play className="h-4 w-4 mr-2" />
            Simuler
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

function CompanyCardSkeleton() {
  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-3">
          <Skeleton className="h-12 w-12 rounded-lg" />
          <div className="space-y-2">
            <Skeleton className="h-5 w-32" />
            <Skeleton className="h-4 w-48" />
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid grid-cols-2 gap-3">
          <Skeleton className="h-4 w-20" />
          <Skeleton className="h-4 w-20" />
          <Skeleton className="h-4 w-20" />
          <Skeleton className="h-4 w-20" />
        </div>
        <Skeleton className="h-16 w-full" />
      </CardContent>
    </Card>
  );
}

export function PECompaniesView() {
  const navigate = useNavigate();
  const { data: companies, isLoading: companiesLoading, error: companiesError } = useOrusPECompanies();
  const { data: investments, isLoading: investmentsLoading, error: investmentsError } = useOrusPEInvestments();

  const [searchQuery, setSearchQuery] = useState("");
  const [filter, setFilter] = useState<"all" | "cfo" | "external">("all");

  const isLoading = companiesLoading || investmentsLoading;
  const error = companiesError || investmentsError;

  const handleSimulate = (companyId: string) => {
    navigate(`/private-equity/simulation/${companyId}`);
  };

  const filteredCompanies = useMemo(() => {
    if (!companies) return [];

    let filtered = [...companies];

    // Apply search
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(
        (c) =>
          c.name.toLowerCase().includes(query) ||
          c.description?.toLowerCase().includes(query) ||
          c.country?.toLowerCase().includes(query)
      );
    }

    // Apply filter
    if (filter === "cfo") {
      filtered = filtered.filter((c) => c.is_cfo_managed);
    } else if (filter === "external") {
      filtered = filtered.filter((c) => !c.is_cfo_managed);
    }

    return filtered;
  }, [companies, searchQuery, filter]);

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

  if (!isLoading && !companies?.length) {
    return (
      <div className="flex items-center justify-center py-16">
        <EmptyPlaceholder
          icon={<Building2 className="text-muted-foreground h-10 w-10" />}
          title="Aucune société"
          description="Ajoutez votre première société PE dans l'application Orus."
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
              placeholder="Rechercher une société..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-9"
            />
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant={filter === "all" ? "default" : "outline"}
              size="sm"
              onClick={() => setFilter("all")}
            >
              Toutes
            </Button>
            <Button
              variant={filter === "cfo" ? "default" : "outline"}
              size="sm"
              onClick={() => setFilter("cfo")}
            >
              CFO
            </Button>
            <Button
              variant={filter === "external" ? "default" : "outline"}
              size="sm"
              onClick={() => setFilter("external")}
            >
              Externes
            </Button>
          </div>
        </div>
      </Card>

      {/* Results count */}
      <p className="text-muted-foreground text-sm">
        {filteredCompanies.length} société{filteredCompanies.length > 1 ? "s" : ""} trouvée{filteredCompanies.length > 1 ? "s" : ""}
      </p>

      {/* Companies grid */}
      {isLoading ? (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {[1, 2, 3].map((i) => (
            <CompanyCardSkeleton key={i} />
          ))}
        </div>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {filteredCompanies.map((company) => (
            <CompanyCard 
              key={company.id} 
              company={company} 
              investments={investments || []} 
              onSimulate={handleSimulate}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export default PECompaniesView;
