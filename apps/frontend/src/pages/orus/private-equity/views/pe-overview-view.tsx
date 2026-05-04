/**
 * PEOverviewView - Private Equity portfolio overview
 */
import { Card } from "@wealthfolio/ui/components/ui/card";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { AmountDisplay, EmptyPlaceholder } from "@wealthfolio/ui";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { useMemo } from "react";
import { TrendingUp, Building2, Briefcase, Users } from "lucide-react";

import { useOrusPECompanies, useOrusPEInvestments } from "@/features/orus-integration";
import type { OrusPECompany, OrusPEInvestment } from "@/features/orus-integration";

// Summary widget
function PortfolioSummaryWidget({
  totalInvested,
  currency,
  companiesCount,
  isLoading,
}: {
  totalInvested: number;
  currency: string;
  companiesCount: number;
  isLoading: boolean;
}) {
  if (isLoading) {
    return (
      <Card className="p-3 sm:p-3.5">
        <div className="flex items-center justify-between gap-3">
          <Skeleton className="h-5 w-32" />
          <Skeleton className="h-9 w-48" />
        </div>
      </Card>
    );
  }

  return (
    <Card className="p-3 sm:p-3.5">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <div className="min-w-0">
            <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
              Portefeuille Private Equity
            </p>
            <p className="text-foreground mt-0.5 text-xl font-bold tracking-tight sm:text-2xl">
              <AmountDisplay value={totalInvested} currency={currency} />
            </p>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-4 text-right">
          <div>
            <p className="text-foreground text-sm font-medium">{companiesCount} société{companiesCount > 1 ? "s" : ""}</p>
            <p className="text-muted-foreground text-xs">en portefeuille</p>
          </div>
        </div>
      </div>
    </Card>
  );
}

// Stat card
function StatCard({
  title,
  value,
  subValue,
  icon: Icon,
  currency,
  isLoading,
  variant,
}: {
  title: string;
  value: string | number;
  subValue?: string;
  icon?: React.ComponentType<{ className?: string }>;
  currency?: string;
  isLoading?: boolean;
  variant?: "default" | "primary";
}) {
  if (isLoading) {
    return (
      <Card className="p-3 sm:p-4">
        <Skeleton className="mb-2 h-4 w-20" />
        <Skeleton className="h-6 w-24" />
      </Card>
    );
  }

  return (
    <Card className={`p-3 sm:p-4 ${variant === "primary" ? "border-primary bg-primary/5" : ""}`}>
      <div className="flex items-center gap-2">
        {Icon && <Icon className="text-muted-foreground h-4 w-4" />}
        <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">{title}</p>
      </div>
      <p className={`mt-1 text-lg font-semibold tracking-tight ${variant === "primary" ? "text-primary" : "text-foreground"}`}>
        {currency ? <AmountDisplay value={Number(value)} currency={currency} /> : value}
      </p>
      {subValue && <p className="text-muted-foreground mt-0.5 text-xs">{subValue}</p>}
    </Card>
  );
}

// Company mini card for overview
function CompanyMiniCard({ company, investment }: { company: OrusPECompany; investment?: OrusPEInvestment }) {
  return (
    <div className="flex items-center justify-between rounded-lg border p-3">
      <div className="flex items-center gap-3">
        <div className="bg-muted flex h-10 w-10 items-center justify-center rounded-lg">
          <Building2 className="text-muted-foreground h-5 w-5" />
        </div>
        <div>
          <p className="text-sm font-medium">{company.name}</p>
          <p className="text-muted-foreground text-xs">
            {company.country || "N/A"} • {company.founded_year || "N/A"}
          </p>
        </div>
      </div>
      <div className="text-right">
        {investment && (
          <>
            <p className="text-sm font-medium">
              <AmountDisplay value={investment.amount} currency={investment.currency} />
            </p>
            <p className="text-muted-foreground text-xs">
              {investment.ownership_percentage?.toFixed(1)}%
            </p>
          </>
        )}
      </div>
    </div>
  );
}

export function PEOverviewView() {
  const { data: companies, isLoading: companiesLoading, error: companiesError } = useOrusPECompanies();
  const { data: investments, isLoading: investmentsLoading, error: investmentsError } = useOrusPEInvestments();

  const isLoading = companiesLoading || investmentsLoading;
  const error = companiesError || investmentsError;

  const summary = useMemo(() => {
    if (!companies || !investments) {
      return { totalInvested: 0, activeCount: 0, cfoManagedCount: 0, avgOwnership: 0 };
    }

    const totalInvested = investments.reduce((sum, inv) => sum + inv.amount, 0);
    const activeCount = companies.filter((c) =>
      investments.some((inv) => inv.company_id === c.id && inv.status === "active")
    ).length;
    const cfoManagedCount = companies.filter((c) => c.is_cfo_managed).length;
    const avgOwnership = investments.length > 0
      ? investments.reduce((sum, inv) => sum + (inv.ownership_percentage || 0), 0) / investments.length
      : 0;

    return { totalInvested, activeCount, cfoManagedCount, avgOwnership };
  }, [companies, investments]);

  // Get latest investment per company for mini cards
  const companiesWithInvestments = useMemo(() => {
    if (!companies || !investments) return [];

    return companies.map((company) => {
      const companyInvestments = investments.filter((inv) => inv.company_id === company.id);
      const latestInvestment = companyInvestments.sort(
        (a, b) => new Date(b.investment_date).getTime() - new Date(a.investment_date).getTime()
      )[0];
      return { company, investment: latestInvestment };
    });
  }, [companies, investments]);

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
          icon={<Briefcase className="text-muted-foreground h-10 w-10" />}
          title="Aucun investissement PE"
          description="Ajoutez votre premier investissement Private Equity dans l'application Orus."
        />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Row 1: Summary Banner */}
      <PortfolioSummaryWidget
        totalInvested={summary.totalInvested}
        currency="EUR"
        companiesCount={companies?.length || 0}
        isLoading={isLoading}
      />

      {/* Row 2: Stats cards */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <StatCard
          title="Total investi"
          value={summary.totalInvested}
          currency="EUR"
          icon={TrendingUp}
          variant="primary"
          isLoading={isLoading}
        />
        <StatCard
          title="Investissements actifs"
          value={summary.activeCount}
          icon={Briefcase}
          isLoading={isLoading}
        />
        <StatCard
          title="Géré par CFO"
          value={summary.cfoManagedCount}
          subValue={`sur ${companies?.length || 0} sociétés`}
          icon={Users}
          isLoading={isLoading}
        />
        <StatCard
          title="Participation moyenne"
          value={`${summary.avgOwnership.toFixed(1)}%`}
          icon={Building2}
          isLoading={isLoading}
        />
      </div>

      {/* Row 3: Top companies */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card className="p-4">
          <p className="text-muted-foreground mb-3 text-xs font-medium uppercase tracking-wider">
            Sociétés en portefeuille
          </p>
          <div className="space-y-2">
            {companiesWithInvestments.slice(0, 5).map(({ company, investment }) => (
              <CompanyMiniCard key={company.id} company={company} investment={investment} />
            ))}
          </div>
        </Card>
        <Card className="p-4">
          <p className="text-muted-foreground mb-3 text-xs font-medium uppercase tracking-wider">
            Répartition géographique
          </p>
          <div className="space-y-3">
            {Object.entries(
              (companies || []).reduce((acc, c) => {
                const country = c.country || "Non spécifié";
                acc[country] = (acc[country] || 0) + 1;
                return acc;
              }, {} as Record<string, number>)
            )
              .sort(([, a], [, b]) => b - a)
              .slice(0, 5)
              .map(([country, count]) => (
                <div key={country}>
                  <div className="mb-1 flex justify-between text-sm">
                    <span>{country}</span>
                    <span className="font-medium">{count}</span>
                  </div>
                  <div className="bg-muted h-2 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-primary transition-all"
                      style={{ width: `${(count / (companies?.length || 1)) * 100}%` }}
                    />
                  </div>
                </div>
              ))}
          </div>
        </Card>
      </div>
    </div>
  );
}

export default PEOverviewView;
