/**
 * ShareholderOverviewView - Main overview tab content
 * Styled exactly like the Holdings Insights page
 */
import { Card } from "@wealthfolio/ui/components/ui/card";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { AmountDisplay, EmptyPlaceholder } from "@wealthfolio/ui";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import { AlertCircle, Clock } from "lucide-react";

import { useShareholder, DOCUMENT_TYPES } from "@/features/orus-integration";
import type { OrusShareholderDocumentRequest } from "@/features/orus-integration";
import { PerformanceCard } from "../components/performance-card";
import { CapitalDistributionChart } from "../components/capital-distribution-chart";

type DocumentType = (typeof DOCUMENT_TYPES)[number];

// Summary widget - like CashHoldingsWidget
function ParticipationSummaryWidget({
  totalValue,
  sharesCount,
  ownershipPercent,
  currency,
  isLoading,
}: {
  totalValue: number;
  sharesCount: number;
  ownershipPercent: string;
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
    <Card className="p-3 sm:p-3.5">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2.5">
          <div className="min-w-0">
            <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
              Valeur totale de votre participation
            </p>
            <p className="text-foreground mt-0.5 text-xl font-bold tracking-tight sm:text-2xl">
              <AmountDisplay value={totalValue} currency={currency} />
            </p>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-4">
          <div className="text-right">
            <p className="text-foreground text-sm font-medium">{sharesCount.toLocaleString()} parts</p>
            <p className="text-muted-foreground text-xs">{ownershipPercent}% du capital</p>
          </div>
        </div>
      </div>
    </Card>
  );
}

// Stat card - like the donut chart cards
function StatCard({
  title,
  value,
  subValue,
  currency,
  isHighlighted,
  isLoading,
}: {
  title: string;
  value: string | number;
  subValue?: string;
  currency?: string;
  isHighlighted?: boolean;
  isLoading?: boolean;
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
    <Card className={`p-3 sm:p-4 ${isHighlighted ? "border-primary bg-primary/5" : ""}`}>
      <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">{title}</p>
      <p
        className={`mt-1 text-lg font-semibold tracking-tight ${isHighlighted ? "text-primary" : "text-foreground"}`}
      >
        {currency ? <AmountDisplay value={Number(value)} currency={currency} /> : value}
      </p>
      {subValue && <p className="text-muted-foreground mt-0.5 text-xs">{subValue}</p>}
    </Card>
  );
}

// Pending requests alert widget
function PendingRequestsWidget({
  requests,
}: {
  requests: OrusShareholderDocumentRequest[];
}) {
  if (requests.length === 0) return null;

  return (
    <Card className="border-orange-500/30 bg-orange-500/5 p-3 sm:p-3.5">
      <div className="flex items-center gap-3">
        <AlertCircle className="h-5 w-5 text-orange-500" />
        <div className="flex-1">
          <p className="text-sm font-medium text-orange-600">
            {requests.length} document(s) demandé(s)
          </p>
          <div className="mt-1.5 flex flex-wrap gap-1.5">
            {requests.map((request) => (
              <Badge
                key={request.id}
                variant="outline"
                className="border-orange-500/50 text-orange-600"
              >
                <Clock className="mr-1 h-3 w-3" />
                {DOCUMENT_TYPES.find((t: DocumentType) => t.value === request.document_type)
                  ?.label || request.document_type}
                {request.due_date && (
                  <span className="ml-1 opacity-75">
                    (avant {format(new Date(request.due_date), "dd/MM")})
                  </span>
                )}
              </Badge>
            ))}
          </div>
        </div>
      </div>
    </Card>
  );
}

export function ShareholderOverviewView() {
  const {
    user,
    participation,
    documentRequests,
    allParticipations,
    loading,
  } = useShareholder();

  // Calculate ownership percentage
  const totalShares = allParticipations.reduce(
    (sum: number, p: { shares_count: number }) => sum + p.shares_count,
    0,
  );
  const ownershipPercent =
    participation && totalShares > 0
      ? ((participation.shares_count / totalShares) * 100).toFixed(2)
      : "0";

  const pendingRequests = documentRequests.filter(
    (r: OrusShareholderDocumentRequest) => r.status === "pending",
  );

  if (!participation && !loading) {
    return (
      <div className="flex items-center justify-center py-16">
        <EmptyPlaceholder
          icon={<Icons.Wallet className="text-muted-foreground h-10 w-10" />}
          title="Aucune participation"
          description="Vous n'avez pas encore de participation enregistrée."
        />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Row 1: Summary Banner (full width) - Like Cash Balance */}
      <ParticipationSummaryWidget
        totalValue={participation?.total_value || 0}
        sharesCount={participation?.shares_count || 0}
        ownershipPercent={ownershipPercent}
        currency={participation?.currency || "EUR"}
        isLoading={loading}
      />

      {/* Row 2: 4 stat cards - Like Currency, Accounts, Classes, Regions */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <StatCard
          title="Nombre de parts"
          value={participation?.shares_count.toLocaleString() || "0"}
          subValue={`${ownershipPercent}% du capital`}
          isLoading={loading}
        />
        <StatCard
          title="Valeur par part"
          value={participation?.share_value || 0}
          currency={participation?.currency || "EUR"}
          isLoading={loading}
        />
        <StatCard
          title="Valeur totale"
          value={participation?.total_value || 0}
          currency={participation?.currency || "EUR"}
          isHighlighted
          isLoading={loading}
        />
        <StatCard
          title="Date d'investissement"
          value={
            participation?.investment_date
              ? format(new Date(participation.investment_date), "dd MMM yyyy", { locale: fr })
              : "-"
          }
          isLoading={loading}
        />
      </div>

      {/* Row 3: Performance + Distribution - Like Composition + Sidebar */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-4">
        <div className="col-span-1 lg:col-span-3">
          <PerformanceCard
            initialInvestment={participation?.total_value || 0}
            currency={participation?.currency || "EUR"}
            investmentDate={participation?.investment_date ?? null}
            percentChange={0}
          />
        </div>
        <div className="col-span-1">
          {user && participation && (
            <CapitalDistributionChart
              userCurrency={participation.currency}
              allParticipations={allParticipations}
              currentUserId={user.id}
            />
          )}
        </div>
      </div>

      {/* Row 4: Pending requests alert */}
      <PendingRequestsWidget requests={pendingRequests} />
    </div>
  );
}

export default ShareholderOverviewView;
