/**
 * Private Equity Page - SwipablePage with tabs
 */
import { Suspense, useMemo } from "react";
import { Briefcase, Building2, TrendingUp } from "lucide-react";
import { Card } from "@wealthfolio/ui/components/ui/card";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { SwipablePage, type SwipablePageView } from "@/components/page";
import { PEOverviewView, PECompaniesView, PEInvestmentsView } from "./views";

// Loading skeleton matching insights page style
const DashboardLoader = () => (
  <div className="flex h-full w-full flex-col space-y-4">
    <Card className="p-3 sm:p-3.5">
      <Skeleton className="h-12 w-full" />
    </Card>
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <Skeleton className="h-32" />
      <Skeleton className="h-32" />
      <Skeleton className="h-32" />
      <Skeleton className="h-32" />
    </div>
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-4">
      <Skeleton className="col-span-3 h-64" />
      <Skeleton className="h-64" />
    </div>
  </div>
);

export default function PrivateEquityPage() {
  const views: SwipablePageView[] = useMemo(
    () => [
      {
        value: "overview",
        label: "Vue d'ensemble",
        icon: Briefcase,
        content: (
          <Suspense fallback={<DashboardLoader />}>
            <PEOverviewView />
          </Suspense>
        ),
      },
      {
        value: "companies",
        label: "Sociétés",
        icon: Building2,
        content: (
          <Suspense fallback={<DashboardLoader />}>
            <PECompaniesView />
          </Suspense>
        ),
      },
      {
        value: "investments",
        label: "Investissements",
        icon: TrendingUp,
        content: (
          <Suspense fallback={<DashboardLoader />}>
            <PEInvestmentsView />
          </Suspense>
        ),
      },
    ],
    []
  );

  return (
    <SwipablePage
      title="Private Equity"
      views={views}
      defaultView="overview"
    />
  );
}
