/**
 * AccountingPage - Main accounting page using SwipablePage layout
 * 2 tabs: Trésorerie (Treasury) and Comptabilité (Accounting with Financial Statements)
 */
import { SwipablePage, SwipablePageView } from "@/components/page";
import { Card } from "@wealthfolio/ui/components/ui/card";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { Icons } from "@wealthfolio/ui";
import { Suspense, useMemo } from "react";

import { AccountingTreasuryView, AccountingOverviewView } from "./views";

// Loading skeleton
const DashboardLoader = () => (
  <div className="flex h-full w-full flex-col space-y-4">
    <Card className="p-3 sm:p-3.5">
      <Skeleton className="h-12 w-full" />
    </Card>
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <Skeleton className="h-24" />
      <Skeleton className="h-24" />
      <Skeleton className="h-24" />
      <Skeleton className="h-24" />
    </div>
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
      <Skeleton className="h-48" />
      <Skeleton className="h-48" />
    </div>
  </div>
);

export default function AccountingPage() {
  const views: SwipablePageView[] = useMemo(
    () => [
      {
        value: "treasury",
        label: "Trésorerie",
        icon: Icons.Wallet,
        content: (
          <Suspense fallback={<DashboardLoader />}>
            <AccountingTreasuryView />
          </Suspense>
        ),
      },
      {
        value: "accounting",
        label: "Comptabilité",
        icon: Icons.FileText,
        content: (
          <Suspense fallback={<DashboardLoader />}>
            <AccountingOverviewView />
          </Suspense>
        ),
      },
    ],
    []
  );

  return <SwipablePage views={views} defaultView="treasury" withPadding={true} />;
}
