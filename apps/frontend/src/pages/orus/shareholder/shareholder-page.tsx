/**
 * ShareholderPage - Main shareholder page using SwipablePage layout
 * Matches the Insights page structure with tabs navigation
 */
import { SwipablePage, SwipablePageView } from "@/components/page";
import { Card } from "@wealthfolio/ui/components/ui/card";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { Icons, EmptyPlaceholder } from "@wealthfolio/ui";
import { Suspense, useMemo } from "react";

import { useShareholder, useOrusAuthWithRole } from "@/features/orus-integration";
import {
  ShareholderOverviewView,
  ShareholderAdminOverviewView,
  ShareholderDocumentsView,
  ShareholderProfileView,
} from "./views";
import { ShareholderAdminPanel } from "./components/shareholder-admin-panel";

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

// Empty state for no participation
function NoParticipationState() {
  return (
    <div className="flex items-center justify-center py-16">
      <EmptyPlaceholder
        icon={<Icons.Wallet className="text-muted-foreground h-10 w-10" />}
        title="Aucune participation"
        description="Vous n'avez pas encore de participation enregistrée. Contactez l'administrateur pour plus d'informations."
      />
    </div>
  );
}

// User Shareholder Page
function ShareholderUserPage() {
  const { participation, loading, error } = useShareholder();

  // Define views for SwipablePage
  const views: SwipablePageView[] = useMemo(
    () => [
      {
        value: "overview",
        label: "Vue d'ensemble",
        icon: Icons.PieChart,
        content: (
          <Suspense fallback={<DashboardLoader />}>
            {!participation ? <NoParticipationState /> : <ShareholderOverviewView />}
          </Suspense>
        ),
      },
      {
        value: "documents",
        label: "Documents",
        icon: Icons.FileText,
        content: (
          <Suspense fallback={<DashboardLoader />}>
            {!participation ? <NoParticipationState /> : <ShareholderDocumentsView />}
          </Suspense>
        ),
      },
      {
        value: "profile",
        label: "Mon profil",
        icon: Icons.User,
        content: (
          <Suspense fallback={<DashboardLoader />}>
            {!participation ? <NoParticipationState /> : <ShareholderProfileView />}
          </Suspense>
        ),
      },
    ],
    [participation],
  );

  if (loading) {
    return (
      <div className="p-4">
        <DashboardLoader />
      </div>
    );
  }

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

  return <SwipablePage views={views} defaultView="overview" withPadding={true} />;
}

// Admin Shareholder Page (includes admin panel)
function ShareholderAdminPage() {
  const { loading } = useShareholder();

  const views: SwipablePageView[] = useMemo(
    () => [
      {
        value: "overview",
        label: "Vue d'ensemble",
        icon: Icons.PieChart,
        content: (
          <Suspense fallback={<DashboardLoader />}>
            <ShareholderAdminOverviewView />
          </Suspense>
        ),
      },
      {
        value: "documents",
        label: "Documents",
        icon: Icons.FileText,
        content: (
          <Suspense fallback={<DashboardLoader />}>
            <ShareholderDocumentsView />
          </Suspense>
        ),
      },
      {
        value: "profile",
        label: "Mon profil",
        icon: Icons.User,
        content: (
          <Suspense fallback={<DashboardLoader />}>
            <ShareholderProfileView />
          </Suspense>
        ),
      },
      {
        value: "admin",
        label: "Administration",
        icon: Icons.Settings,
        content: (
          <Suspense fallback={<DashboardLoader />}>
            <ShareholderAdminPanel />
          </Suspense>
        ),
      },
    ],
    [],
  );

  if (loading) {
    return (
      <div className="p-4">
        <DashboardLoader />
      </div>
    );
  }

  return <SwipablePage views={views} defaultView="overview" withPadding={true} />;
}

// Main entry point - routes based on role
export default function ShareholderPage() {
  const { loading, isAdmin } = useOrusAuthWithRole();

  if (loading) {
    return (
      <div className="p-4">
        <DashboardLoader />
      </div>
    );
  }

  if (isAdmin) {
    return <ShareholderAdminPage />;
  }

  return <ShareholderUserPage />;
}
