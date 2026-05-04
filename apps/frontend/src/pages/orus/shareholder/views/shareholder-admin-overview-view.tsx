/**
 * ShareholderAdminOverviewView - Admin overview with complete visibility
 * Shows cap table, pie chart with names, and AUM evolution
 */
import { Card } from "@wealthfolio/ui/components/ui/card";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { AmountDisplay, EmptyPlaceholder } from "@wealthfolio/ui";
import { Icons } from "@wealthfolio/ui/components/ui/icons";

import { useShareholderAdmin } from "@/features/orus-integration";
import { CapTable } from "../components/cap-table";
import { AdminCapitalDistributionChart } from "../components/admin-capital-distribution-chart";
import { AUMEvolutionChart } from "../components/aum-evolution-chart";

// Summary widget for admin - shows total AUM, shareholders, etc.
function AdminSummaryWidget({
  totalAUM,
  shareholderCount,
  totalShares,
  currency,
  isLoading,
}: {
  totalAUM: number;
  shareholderCount: number;
  totalShares: number;
  currency: string;
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
    <Card className="border-primary/20 bg-primary/5 p-3 sm:p-3.5">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <div className="min-w-0">
            <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
              Assets Under Management (AUM)
            </p>
            <p className="text-primary mt-0.5 text-2xl font-bold tracking-tight sm:text-3xl">
              <AmountDisplay value={totalAUM} currency={currency} />
            </p>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-6">
          <div className="text-right">
            <p className="text-foreground text-lg font-semibold">{shareholderCount}</p>
            <p className="text-muted-foreground text-xs">Actionnaires</p>
          </div>
          <div className="text-right">
            <p className="text-foreground text-lg font-semibold">
              {totalShares.toLocaleString()}
            </p>
            <p className="text-muted-foreground text-xs">Parts totales</p>
          </div>
        </div>
      </div>
    </Card>
  );
}

// KPI Cards row
function KPICardsRow({
  shareholders,
  currency,
  isLoading,
}: {
  shareholders: any[];
  currency: string;
  isLoading: boolean;
}) {
  if (isLoading) {
    return (
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {[1, 2, 3, 4].map((i) => (
          <Card key={i} className="p-3 sm:p-4">
            <Skeleton className="mb-2 h-4 w-20" />
            <Skeleton className="h-6 w-24" />
          </Card>
        ))}
      </div>
    );
  }

  // Calculate metrics
  const totalValue = shareholders.reduce((sum, s) => sum + (s.participation.total_value || 0), 0);
  const avgInvestment = shareholders.length > 0 ? totalValue / shareholders.length : 0;

  // Get largest shareholder
  const sorted = [...shareholders].sort(
    (a, b) => b.participation.shares_count - a.participation.shares_count
  );
  const largestShareholder = sorted[0];
  const largestPercent =
    shareholders.length > 0 && totalValue > 0
      ? (((largestShareholder?.participation.total_value || 0) / totalValue) * 100).toFixed(1)
      : "0";

  // Get most recent investment
  const withDates = shareholders.filter((s) => s.participation.investment_date);
  const mostRecent = withDates.sort(
    (a, b) =>
      new Date(b.participation.investment_date!).getTime() -
      new Date(a.participation.investment_date!).getTime()
  )[0];

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <Card className="p-3 sm:p-4">
        <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
          Nombre d'actionnaires
        </p>
        <p className="mt-1 text-xl font-semibold">{shareholders.length}</p>
      </Card>
      <Card className="p-3 sm:p-4">
        <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
          Investissement moyen
        </p>
        <p className="mt-1 text-xl font-semibold">
          <AmountDisplay value={avgInvestment} currency={currency} />
        </p>
      </Card>
      <Card className="p-3 sm:p-4">
        <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
          Plus gros actionnaire
        </p>
        <p className="mt-1 text-xl font-semibold">{largestPercent}%</p>
        <p className="text-muted-foreground text-xs">
          {largestShareholder?.profile?.full_name ||
            largestShareholder?.profile?.email ||
            "N/A"}
        </p>
      </Card>
      <Card className="p-3 sm:p-4">
        <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
          Dernier investissement
        </p>
        <p className="mt-1 text-xl font-semibold">
          {mostRecent ? (
            <AmountDisplay
              value={mostRecent.participation.total_value || 0}
              currency={mostRecent.participation.currency}
            />
          ) : (
            "-"
          )}
        </p>
        {mostRecent && (
          <p className="text-muted-foreground text-xs">
            {mostRecent.profile?.full_name || mostRecent.profile?.email || "Anonyme"}
          </p>
        )}
      </Card>
    </div>
  );
}

export function ShareholderAdminOverviewView() {
  const { shareholders, loading, error } = useShareholderAdmin();

  // Determine currency (use first shareholder's currency or default to EUR)
  const currency = shareholders[0]?.participation.currency || "EUR";

  // Calculate totals
  const totalAUM = shareholders.reduce((sum, s) => sum + (s.participation.total_value || 0), 0);
  const totalShares = shareholders.reduce((sum, s) => sum + s.participation.shares_count, 0);

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

  if (!loading && shareholders.length === 0) {
    return (
      <div className="flex items-center justify-center py-16">
        <EmptyPlaceholder
          icon={<Icons.Users className="text-muted-foreground h-10 w-10" />}
          title="Aucun actionnaire"
          description="Ajoutez des actionnaires depuis l'onglet Administration"
        />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Row 1: Summary Banner */}
      <AdminSummaryWidget
        totalAUM={totalAUM}
        shareholderCount={shareholders.length}
        totalShares={totalShares}
        currency={currency}
        isLoading={loading}
      />

      {/* Row 2: KPI Cards */}
      <KPICardsRow shareholders={shareholders} currency={currency} isLoading={loading} />

      {/* Row 3: AUM Evolution Chart (full width) */}
      <AUMEvolutionChart shareholders={shareholders} currency={currency} />

      {/* Row 4: Cap Table + Pie Chart */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <CapTable shareholders={shareholders} currency={currency} isAdmin={true} />
        </div>
        <div className="lg:col-span-1">
          <AdminCapitalDistributionChart shareholders={shareholders} currency={currency} />
        </div>
      </div>
    </div>
  );
}

export default ShareholderAdminOverviewView;
