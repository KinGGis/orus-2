/**
 * Reports Hooks for Orus Integration
 * Manages report generation, configuration, and history
 */
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { orusSupabase } from "./supabase-client";
import { toast } from "sonner";

// =====================================================
// TYPES
// =====================================================

export interface ReportConfiguration {
  id: string;
  report_type: "performance" | "allocation" | "monthly" | "regulatory" | "client_presentation";
  name: string;
  description?: string;
  is_enabled: boolean;
  frequency: "daily" | "weekly" | "monthly" | "quarterly";
  schedule_time: string;
  schedule_day?: number;
  recipients: string[];
  n8n_webhook_url?: string;
  created_by?: string;
  created_at: string;
  last_run_at?: string;
  next_run_at?: string;
}

export interface ReportHistory {
  id: string;
  config_id?: string;
  report_type: string;
  report_name: string;
  status: "pending" | "generating" | "completed" | "failed";
  triggered_by: "manual" | "scheduled" | "api";
  generation_started_at?: string;
  generation_completed_at?: string;
  file_path?: string;
  view_only_url?: string;
  file_size?: number;
  file_format?: string;
  error_message?: string;
  n8n_execution_id?: string;
  created_at: string;
  visible_to_investors: boolean;
}

export interface DateRangeParams {
  period_type: "quarterly" | "semi-annual" | "annual" | "all-time" | "custom";
  start_date?: string;
  end_date?: string;
}

export interface ReportFilters {
  search?: string;
  status?: string;
  type?: string;
}

// =====================================================
// CONSTANTS
// =====================================================

const GENERATION_TIMEOUT_MS = 10 * 60 * 1000; // 10 minutes timeout

// =====================================================
// REPORT CONFIGURATIONS HOOKS
// =====================================================

export function useReportConfigs() {
  return useQuery({
    queryKey: ["orus", "report-configurations"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("report_configurations")
        .select("*")
        .order("created_at", { ascending: false });

      if (error) throw error;
      return data as ReportConfiguration[];
    },
  });
}

export function useCreateReportConfig() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (config: Omit<ReportConfiguration, "id" | "created_at">) => {
      const {
        data: { user },
      } = await orusSupabase.auth.getUser();

      const { data, error } = await orusSupabase
        .from("report_configurations")
        .insert({
          ...config,
          created_by: user?.id,
        })
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "report-configurations"] });
      toast.success("Configuration créée");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

export function useUpdateReportConfig() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (config: Partial<ReportConfiguration> & { id: string }) => {
      const { id, ...updateData } = config;

      const { data, error } = await orusSupabase
        .from("report_configurations")
        .update(updateData)
        .eq("id", id)
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "report-configurations"] });
      toast.success("Configuration mise à jour");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

export function useDeleteReportConfig() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (configId: string) => {
      const { error } = await orusSupabase.from("report_configurations").delete().eq("id", configId);

      if (error) throw error;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "report-configurations"] });
      toast.success("Configuration supprimée");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

// =====================================================
// REPORT HISTORY HOOKS
// =====================================================

export function useReportsHistory(filters?: ReportFilters) {
  const queryClient = useQueryClient();

  return useQuery({
    queryKey: ["orus", "reports-history", filters],
    queryFn: async () => {
      let query = orusSupabase
        .from("reports_history")
        .select("*")
        .order("created_at", { ascending: false });

      if (filters?.type && filters.type !== "all") {
        query = query.eq("report_type", filters.type);
      }
      if (filters?.status && filters.status !== "all") {
        query = query.eq("status", filters.status);
      }
      if (filters?.search) {
        query = query.ilike("report_name", `%${filters.search}%`);
      }

      const { data, error } = await query.limit(100);

      if (error) throw error;

      const reports = data as ReportHistory[];

      // Auto-detect and mark stuck reports as failed (generating for more than 10 minutes)
      const now = Date.now();
      const stuckReports = reports.filter((report) => {
        if (report.status !== "pending" && report.status !== "generating") return false;
        const createdAt = new Date(report.created_at).getTime();
        return now - createdAt > GENERATION_TIMEOUT_MS;
      });

      // Mark stuck reports as failed in the background
      if (stuckReports.length > 0) {
        for (const report of stuckReports) {
          await orusSupabase
            .from("reports_history")
            .update({
              status: "failed",
              error_message: "Délai de génération dépassé (timeout)",
              generation_completed_at: new Date().toISOString(),
            })
            .eq("id", report.id)
            .eq("status", report.status);
        }

        // Invalidate to get fresh data
        queryClient.invalidateQueries({ queryKey: ["orus", "reports-history"] });

        // Return updated reports immediately for better UX
        return reports.map((report) => {
          if (stuckReports.find((r) => r.id === report.id)) {
            return {
              ...report,
              status: "failed" as const,
              error_message: "Délai de génération dépassé (timeout)",
            };
          }
          return report;
        });
      }

      return reports;
    },
    refetchInterval: (query) => {
      // Refetch every 5 seconds if there are pending/generating reports
      const hasPendingReports = query.state.data?.some(
        (report) => report.status === "pending" || report.status === "generating"
      );
      return hasPendingReports ? 5000 : false;
    },
  });
}

// =====================================================
// REPORT GENERATION HOOKS
// =====================================================

export function useGenerateReport() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (params: {
      report_type: "performance" | "allocation" | "monthly" | "regulatory" | "client_presentation";
      date_range?: DateRangeParams;
      name?: string;
    }) => {
      const {
        data: { session },
      } = await orusSupabase.auth.getSession();

      if (!session) {
        throw new Error("Not authenticated");
      }

      // Generate report name if not provided
      const reportName =
        params.name ||
        `Rapport ${getReportTypeLabel(params.report_type)} - ${new Date().toLocaleDateString("fr-FR")}`;

      // First, insert into reports_history
      const { data: historyEntry, error: historyError } = await orusSupabase
        .from("reports_history")
        .insert({
          report_type: params.report_type,
          report_name: reportName,
          status: "pending",
          triggered_by: "manual",
          visible_to_investors: false,
        })
        .select()
        .single();

      if (historyError) throw historyError;

      // Then invoke the edge function
      const response = await orusSupabase.functions.invoke("generate-gamma-presentation", {
        body: {
          report_type: params.report_type,
          date_range: params.date_range,
          name: reportName,
          report_id: historyEntry.id,
          triggered_by: "manual",
        },
      });

      // Extract detailed error message
      let errorMessage: string | null = null;
      if (response.error) {
        // Try to get the actual error from the response data if available
        if (response.data?.error) {
          errorMessage = response.data.error;
        } else if (response.error.message) {
          // Parse JSON error if it's a stringified JSON
          try {
            const parsed = JSON.parse(response.error.message);
            errorMessage = parsed.error || parsed.message || response.error.message;
          } catch {
            errorMessage = response.error.message;
          }
        } else {
          errorMessage = "Erreur inconnue lors de la génération";
        }
      }

      if (errorMessage) {
        // Update history with detailed error
        await orusSupabase
          .from("reports_history")
          .update({
            status: "failed",
            error_message: errorMessage,
            generation_completed_at: new Date().toISOString(),
          })
          .eq("id", historyEntry.id);

        throw new Error(errorMessage);
      }

      return response.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "reports-history"] });
      toast.success("Génération du rapport lancée", {
        description: "Le rapport sera disponible dans quelques instants",
      });
    },
    onError: (error: Error) => {
      console.error("Error generating report:", error);
      // Try to display more specific error messages
      let errorDetails = error.message;
      if (errorDetails.includes("GAMMA_API_KEY")) {
        errorDetails = "Clé API Gamma non configurée. Contactez l'administrateur.";
      } else if (errorDetails.includes("non-2xx")) {
        errorDetails = "Erreur serveur. Vérifiez les logs Supabase ou réessayez plus tard.";
      }
      toast.error(`Erreur de génération`, {
        description: errorDetails,
        duration: 8000,
      });
    },
  });
}

export function useCancelReport() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (reportId: string) => {
      const { error } = await orusSupabase
        .from("reports_history")
        .update({
          status: "failed",
          error_message: "Génération annulée par l'utilisateur",
          generation_completed_at: new Date().toISOString(),
        })
        .eq("id", reportId);

      if (error) throw error;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "reports-history"] });
      toast.success("Génération du rapport annulée");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

export function useRefreshReportStatus() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (reportId: string) => {
      const response = await orusSupabase.functions.invoke("refresh-report-status", {
        body: { report_id: reportId },
      });

      if (response.error) {
        // Extract detailed error message
        let errorMessage = "Erreur lors de la vérification";
        if (response.data?.error) {
          errorMessage = response.data.error;
        } else if (response.error.message) {
          try {
            const parsed = JSON.parse(response.error.message);
            errorMessage = parsed.error || parsed.message || response.error.message;
          } catch {
            errorMessage = response.error.message;
          }
        }
        throw new Error(errorMessage);
      }

      return response.data;
    },
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["orus", "reports-history"] });
      if (data?.updated_status === "completed") {
        toast.success("Le rapport est prêt !");
      } else if (data?.updated_status === "generating") {
        toast.info("Le rapport est encore en cours de génération");
      } else {
        toast.info(`Statut: ${data?.gamma_status || "vérifié"}`);
      }
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

export function useToggleReportVisibility() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ reportId, visible }: { reportId: string; visible: boolean }) => {
      const { error } = await orusSupabase
        .from("reports_history")
        .update({ visible_to_investors: visible })
        .eq("id", reportId);

      if (error) throw error;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "reports-history"] });
      toast.success("Visibilité mise à jour");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

// =====================================================
// DOWNLOAD REPORT
// =====================================================

export async function downloadReport(filePath: string, fileName: string) {
  try {
    const { data, error } = await orusSupabase.storage.from("reports").download(filePath);

    if (error) throw error;

    // Create download link
    const url = URL.createObjectURL(data);
    const a = document.createElement("a");
    a.href = url;
    a.download = fileName;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    toast.success("Téléchargement démarré");
  } catch (error: unknown) {
    console.error("Error downloading report:", error);
    const message = error instanceof Error ? error.message : "Erreur inconnue";
    toast.error(`Erreur de téléchargement: ${message}`);
  }
}

// =====================================================
// HELPERS
// =====================================================

export function getReportTypeLabel(type: string): string {
  const labels: Record<string, string> = {
    performance: "Performance",
    allocation: "Allocation",
    monthly: "Mensuel",
    regulatory: "Réglementaire",
    client_presentation: "Présentation Client",
  };
  return labels[type] || type;
}

export function getReportStatusBadgeVariant(
  status: string
): "default" | "destructive" | "secondary" | "outline" {
  switch (status) {
    case "completed":
      return "default";
    case "failed":
      return "destructive";
    case "generating":
      return "secondary";
    case "pending":
      return "outline";
    default:
      return "outline";
  }
}

export function getReportStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    completed: "Terminé",
    failed: "Échec",
    generating: "En cours",
    pending: "En attente",
  };
  return labels[status] || status;
}
