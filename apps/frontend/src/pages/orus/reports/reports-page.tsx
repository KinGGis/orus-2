/**
 * ReportsPage - Main reports page with generation cards and history
 */
import { useState, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui/components/ui/card";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Input } from "@wealthfolio/ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui/components/ui/select";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import {
  BarChart3,
  PieChart,
  Calendar,
  Shield,
  Presentation,
  Settings,
  Search,
  ChevronLeft,
  ChevronRight,
  FileText,
  Loader2,
} from "lucide-react";

import {
  useReportsHistory,
  useGenerateReport,
  type DateRangeParams,
} from "@/features/orus-integration/reports-hooks";
import { ReportCard } from "./components/report-card";
import { DateRangeDialog } from "./components/date-range-dialog";

// =====================================================
// TYPES
// =====================================================

type ReportType = "performance" | "allocation" | "monthly" | "regulatory" | "client_presentation";

interface ReportTypeCard {
  type: ReportType;
  title: string;
  description: string;
  icon: React.ReactNode;
  needsDateRange: boolean;
}

// =====================================================
// CONSTANTS
// =====================================================

const REPORT_TYPE_CARDS: ReportTypeCard[] = [
  {
    type: "performance",
    title: "Rapport de Performance",
    description: "Analyse détaillée de la performance du portefeuille",
    icon: <BarChart3 className="h-8 w-8" />,
    needsDateRange: true,
  },
  {
    type: "allocation",
    title: "Rapport d'Allocation",
    description: "Répartition des actifs par classe et secteur",
    icon: <PieChart className="h-8 w-8" />,
    needsDateRange: false,
  },
  {
    type: "monthly",
    title: "Rapport Mensuel",
    description: "Synthèse mensuelle des opérations et positions",
    icon: <Calendar className="h-8 w-8" />,
    needsDateRange: false,
  },
  {
    type: "regulatory",
    title: "Rapport Réglementaire",
    description: "Documents de conformité et rapports légaux",
    icon: <Shield className="h-8 w-8" />,
    needsDateRange: false,
  },
  {
    type: "client_presentation",
    title: "Présentation Client",
    description: "Présentation interactive via Gamma.app",
    icon: <Presentation className="h-8 w-8" />,
    needsDateRange: false,
  },
];

const ITEMS_PER_PAGE = 6;

// =====================================================
// COMPONENTS
// =====================================================

function ReportTypeCardComponent({
  card,
  onGenerate,
  isGenerating,
}: {
  card: ReportTypeCard;
  onGenerate: (type: ReportType) => void;
  isGenerating: boolean;
}) {
  return (
    <Card className="bg-card/50 border-border/50 hover:shadow-md transition-shadow">
      <CardHeader className="pb-2">
        <div className="flex items-center gap-3">
          <div className="p-3 rounded-lg bg-primary/10 text-primary">{card.icon}</div>
          <div>
            <CardTitle className="text-base">{card.title}</CardTitle>
            <CardDescription className="text-xs">{card.description}</CardDescription>
          </div>
        </div>
      </CardHeader>
      <CardContent className="pt-2">
        <Button
          onClick={() => onGenerate(card.type)}
          disabled={isGenerating}
          className="w-full"
          size="sm"
        >
          {isGenerating ? (
            <Loader2 className="h-4 w-4 mr-2 animate-spin" />
          ) : (
            <FileText className="h-4 w-4 mr-2" />
          )}
          Générer
        </Button>
      </CardContent>
    </Card>
  );
}

function ReportsHistorySkeleton() {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      {Array.from({ length: 6 }).map((_, i) => (
        <Skeleton key={i} className="h-24" />
      ))}
    </div>
  );
}

// =====================================================
// MAIN PAGE COMPONENT
// =====================================================

export default function ReportsPage() {
  const navigate = useNavigate();
  const generateReport = useGenerateReport();

  // Filters state
  const [searchTerm, setSearchTerm] = useState("");
  const [statusFilter, setStatusFilter] = useState("all");
  const [typeFilter, setTypeFilter] = useState("all");
  const [currentPage, setCurrentPage] = useState(1);

  // Date range dialog state
  const [dateRangeDialogOpen, setDateRangeDialogOpen] = useState(false);
  const [selectedReportType, setSelectedReportType] = useState<ReportType | null>(null);

  // Fetch reports history
  const { data: reports, isLoading: reportsLoading } = useReportsHistory({
    search: searchTerm || undefined,
    status: statusFilter !== "all" ? statusFilter : undefined,
    type: typeFilter !== "all" ? typeFilter : undefined,
  });

  // Pagination
  const totalPages = Math.ceil((reports?.length || 0) / ITEMS_PER_PAGE);
  const paginatedReports = useMemo(() => {
    if (!reports) return [];
    const start = (currentPage - 1) * ITEMS_PER_PAGE;
    return reports.slice(start, start + ITEMS_PER_PAGE);
  }, [reports, currentPage]);

  // Handlers
  const handleGenerateClick = (type: ReportType) => {
    const card = REPORT_TYPE_CARDS.find((c) => c.type === type);
    if (card?.needsDateRange) {
      setSelectedReportType(type);
      setDateRangeDialogOpen(true);
    } else {
      // Generate directly with all-time period
      generateReport.mutate({
        report_type: type,
        date_range: { period_type: "all-time" },
      });
    }
  };

  const handleGenerateWithDateRange = (dateRange: DateRangeParams) => {
    if (selectedReportType) {
      generateReport.mutate({
        report_type: selectedReportType,
        date_range: dateRange,
      });
      setDateRangeDialogOpen(false);
      setSelectedReportType(null);
    }
  };

  return (
    <div className="container mx-auto py-6 space-y-8">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Rapports</h1>
          <p className="text-muted-foreground">Générez et exportez des rapports détaillés</p>
        </div>
        <Button variant="outline" onClick={() => navigate("/reports/config")}>
          <Settings className="h-4 w-4 mr-2" />
          Configuration
        </Button>
      </div>

      {/* Report Type Cards Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5 gap-4">
        {REPORT_TYPE_CARDS.map((card) => (
          <ReportTypeCardComponent
            key={card.type}
            card={card}
            onGenerate={handleGenerateClick}
            isGenerating={generateReport.isPending}
          />
        ))}
      </div>

      {/* History Section */}
      <Card className="bg-card/50 border-border/50">
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle>Historique Complet</CardTitle>
              <CardDescription>{reports?.length || 0} rapports au total</CardDescription>
            </div>
          </div>

          {/* Filters */}
          <div className="flex flex-wrap gap-4 pt-4">
            <div className="flex-1 min-w-[200px]">
              <div className="relative">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                <Input
                  placeholder="Rechercher un rapport..."
                  value={searchTerm}
                  onChange={(e) => {
                    setSearchTerm(e.target.value);
                    setCurrentPage(1);
                  }}
                  className="pl-10"
                />
              </div>
            </div>
            <Select
              value={typeFilter}
              onValueChange={(value) => {
                setTypeFilter(value);
                setCurrentPage(1);
              }}
            >
              <SelectTrigger className="w-[180px]">
                <SelectValue placeholder="Type" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">Tous les types</SelectItem>
                <SelectItem value="performance">Performance</SelectItem>
                <SelectItem value="allocation">Allocation</SelectItem>
                <SelectItem value="monthly">Mensuel</SelectItem>
                <SelectItem value="regulatory">Réglementaire</SelectItem>
                <SelectItem value="client_presentation">Présentation Client</SelectItem>
              </SelectContent>
            </Select>
            <Select
              value={statusFilter}
              onValueChange={(value) => {
                setStatusFilter(value);
                setCurrentPage(1);
              }}
            >
              <SelectTrigger className="w-[150px]">
                <SelectValue placeholder="Statut" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">Tous les statuts</SelectItem>
                <SelectItem value="completed">Terminé</SelectItem>
                <SelectItem value="failed">Échec</SelectItem>
                <SelectItem value="generating">En cours</SelectItem>
                <SelectItem value="pending">En attente</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </CardHeader>

        <CardContent>
          {reportsLoading ? (
            <ReportsHistorySkeleton />
          ) : paginatedReports.length === 0 ? (
            <div className="text-center py-12 text-muted-foreground">
              <FileText className="h-12 w-12 mx-auto mb-4 opacity-50" />
              <p>Aucun rapport trouvé</p>
            </div>
          ) : (
            <>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {paginatedReports.map((report) => (
                  <ReportCard key={report.id} report={report} showVisibilityToggle />
                ))}
              </div>

              {/* Pagination */}
              {totalPages > 1 && (
                <div className="flex items-center justify-center gap-4 mt-6">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                    disabled={currentPage === 1}
                  >
                    <ChevronLeft className="h-4 w-4 mr-1" />
                    Précédent
                  </Button>
                  <span className="text-sm text-muted-foreground">
                    Page {currentPage} sur {totalPages}
                  </span>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
                    disabled={currentPage === totalPages}
                  >
                    Suivant
                    <ChevronRight className="h-4 w-4 ml-1" />
                  </Button>
                </div>
              )}
            </>
          )}
        </CardContent>
      </Card>

      {/* Date Range Dialog */}
      <DateRangeDialog
        open={dateRangeDialogOpen}
        onOpenChange={setDateRangeDialogOpen}
        onGenerate={handleGenerateWithDateRange}
        isGenerating={generateReport.isPending}
      />
    </div>
  );
}
