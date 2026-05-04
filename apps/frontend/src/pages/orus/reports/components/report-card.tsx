/**
 * ReportCard - Card component for displaying a generated report
 */
import { useState } from "react";
import { formatDistanceToNow } from "date-fns";
import { fr } from "date-fns/locale";
import {
  Card,
  CardContent,
  CardHeader,
} from "@wealthfolio/ui/components/ui/card";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Switch } from "@wealthfolio/ui/components/ui/switch";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@wealthfolio/ui/components/ui/collapsible";
import {
  ChevronDown,
  ExternalLink,
  RefreshCw,
  XCircle,
  Loader2,
  BarChart3,
  PieChart,
  Calendar,
  Shield,
  Presentation,
  Eye,
  EyeOff,
} from "lucide-react";

import type { ReportHistory } from "@/features/orus-integration/reports-hooks";
import {
  getReportTypeLabel,
  getReportStatusBadgeVariant,
  getReportStatusLabel,
  useCancelReport,
  useRefreshReportStatus,
  useToggleReportVisibility,
} from "@/features/orus-integration/reports-hooks";

interface ReportCardProps {
  report: ReportHistory;
  showVisibilityToggle?: boolean;
}

const REPORT_ICONS: Record<string, React.ReactNode> = {
  performance: <BarChart3 className="h-5 w-5" />,
  allocation: <PieChart className="h-5 w-5" />,
  monthly: <Calendar className="h-5 w-5" />,
  regulatory: <Shield className="h-5 w-5" />,
  client_presentation: <Presentation className="h-5 w-5" />,
};

export function ReportCard({ report, showVisibilityToggle = false }: ReportCardProps) {
  const [isOpen, setIsOpen] = useState(false);
  const cancelReport = useCancelReport();
  const refreshStatus = useRefreshReportStatus();
  const toggleVisibility = useToggleReportVisibility();

  const handleOpenReport = () => {
    if (report.view_only_url) {
      window.open(report.view_only_url, "_blank");
    }
  };

  const handleCancel = () => {
    cancelReport.mutate(report.id);
  };

  const handleRefreshStatus = () => {
    refreshStatus.mutate(report.id);
  };

  const handleToggleVisibility = (checked: boolean) => {
    toggleVisibility.mutate({ reportId: report.id, visible: checked });
  };

  const timeAgo = formatDistanceToNow(new Date(report.created_at), {
    addSuffix: true,
    locale: fr,
  });

  const statusBadge = (
    <Badge
      variant={getReportStatusBadgeVariant(report.status)}
      className={
        report.status === "generating"
          ? "animate-pulse"
          : report.status === "completed"
          ? "bg-emerald-500 hover:bg-emerald-600"
          : ""
      }
    >
      {report.status === "generating" && <Loader2 className="h-3 w-3 mr-1 animate-spin" />}
      {getReportStatusLabel(report.status)}
    </Badge>
  );

  return (
    <Collapsible open={isOpen} onOpenChange={setIsOpen}>
      <Card className="bg-card/50 border-border/50">
        <CollapsibleTrigger asChild>
          <CardHeader className="cursor-pointer hover:bg-muted/30 transition-colors pb-2">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="p-2 rounded-lg bg-primary/10 text-primary">
                  {REPORT_ICONS[report.report_type] || <BarChart3 className="h-5 w-5" />}
                </div>
                <div className="text-left">
                  <h4 className="font-medium text-sm">{report.report_name}</h4>
                  <p className="text-xs text-muted-foreground">
                    {timeAgo} • {getReportTypeLabel(report.report_type)}
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-2">
                {statusBadge}
                <ChevronDown
                  className={`h-4 w-4 text-muted-foreground transition-transform duration-200 ${
                    isOpen ? "rotate-180" : ""
                  }`}
                />
              </div>
            </div>
          </CardHeader>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <CardContent className="pt-0 space-y-3">
            {/* Actions based on status */}
            {report.status === "completed" && (
              <div className="flex items-center gap-2">
                <Button size="sm" onClick={handleOpenReport} disabled={!report.view_only_url}>
                  <ExternalLink className="h-4 w-4 mr-2" />
                  Consulter / Modifier
                </Button>
              </div>
            )}

            {report.status === "failed" && (
              <div className="space-y-2">
                <p className="text-sm text-destructive">{report.error_message}</p>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={handleRefreshStatus}
                  disabled={refreshStatus.isPending}
                >
                  {refreshStatus.isPending ? (
                    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                  ) : (
                    <RefreshCw className="h-4 w-4 mr-2" />
                  )}
                  Vérifier le statut Gamma
                </Button>
              </div>
            )}

            {(report.status === "generating" || report.status === "pending") && (
              <Button
                size="sm"
                variant="destructive"
                onClick={handleCancel}
                disabled={cancelReport.isPending}
              >
                {cancelReport.isPending ? (
                  <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                ) : (
                  <XCircle className="h-4 w-4 mr-2" />
                )}
                Annuler
              </Button>
            )}

            {/* Visibility toggle for admins */}
            {showVisibilityToggle && report.status === "completed" && (
              <div className="flex items-center justify-between pt-2 border-t">
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  {report.visible_to_investors ? (
                    <Eye className="h-4 w-4" />
                  ) : (
                    <EyeOff className="h-4 w-4" />
                  )}
                  <span>Visible pour les investisseurs</span>
                </div>
                <Switch
                  checked={report.visible_to_investors}
                  onCheckedChange={handleToggleVisibility}
                  disabled={toggleVisibility.isPending}
                />
              </div>
            )}
          </CardContent>
        </CollapsibleContent>
      </Card>
    </Collapsible>
  );
}
