/**
 * Accounting Hooks
 * Complete accounting operations: periods, entries, reports, AI analysis
 * Based on Orus useAccounting.ts
 */
import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { orusSupabase } from "./supabase-client";
import { toast } from "sonner";

// =====================================================
// TYPES
// =====================================================

export interface AccountingPeriod {
  id: string;
  name: string;
  start_date: string;
  end_date: string;
  entity_id: string;
  status: "draft" | "validated" | "closed";
  currency: string;
  notes?: string;
  created_by?: string;
  created_at: string;
  updated_at: string;
}

export interface ChartOfAccount {
  id: string;
  code: string;
  name: string;
  type: "asset" | "liability" | "equity" | "income" | "expense";
  parent_id?: string;
  description?: string;
  hmrc_box?: string;
  is_active: boolean;
  sort_order: number;
}

export interface AccountingEntry {
  id: string;
  period_id: string;
  entry_date: string;
  account_code: string;
  description: string;
  debit: number;
  credit: number;
  currency: string;
  source_type?: string;
  source_id?: string;
  ai_generated: boolean;
  ai_explanation?: string;
  validated: boolean;
  validated_by?: string;
  validated_at?: string;
  created_at: string;
}

export interface AccountingReport {
  id: string;
  period_id: string;
  report_type: "balance_sheet" | "profit_loss" | "trial_balance";
  generated_at: string;
  generated_by?: string;
  data_snapshot: Record<string, any>;
  ai_notes?: string;
  ai_analysis?: Record<string, any>;
  file_path?: string;
  file_format?: string;
  status: "draft" | "final" | "archived";
}

export interface TrialBalanceEntry {
  account_code: string;
  account_name: string;
  account_type: string;
  debit_total: number;
  credit_total: number;
  balance: number;
}

export interface AccountingSummary {
  totalDebits: number;
  totalCredits: number;
  entriesCount: number;
  validatedCount: number;
  pendingCount: number;
  balance: number;
  byAccountType: Record<string, { debit: number; credit: number }>;
}

// =====================================================
// PERIODS HOOKS
// =====================================================

export function useAccountingPeriods() {
  return useQuery({
    queryKey: ["orus", "accounting", "periods"],
    queryFn: async () => {
      const { data, error } = await (orusSupabase as any)
        .from("accounting_periods")
        .select("*")
        .order("start_date", { ascending: false });

      if (error) throw error;
      return data as AccountingPeriod[];
    },
  });
}

export function useAccountingPeriod(periodId: string | null) {
  return useQuery({
    queryKey: ["orus", "accounting", "period", periodId],
    queryFn: async () => {
      if (!periodId) return null;

      const { data, error } = await (orusSupabase as any)
        .from("accounting_periods")
        .select("*")
        .eq("id", periodId)
        .single();

      if (error) throw error;
      return data as AccountingPeriod;
    },
    enabled: !!periodId,
  });
}

export function useCreateAccountingPeriod() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (
      period: Omit<AccountingPeriod, "id" | "created_at" | "updated_at">
    ) => {
      const { data, error } = await (orusSupabase as any)
        .from("accounting_periods")
        .insert({
          ...period,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        })
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      toast.success("Période créée avec succès");
      queryClient.invalidateQueries({ queryKey: ["orus", "accounting", "periods"] });
    },
    onError: (error) => {
      console.error("Error creating accounting period:", error);
      toast.error("Erreur lors de la création de la période");
    },
  });
}

export function useUpdateAccountingPeriod() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      periodId,
      ...updates
    }: Partial<AccountingPeriod> & { periodId: string }) => {
      const { data, error } = await (orusSupabase as any)
        .from("accounting_periods")
        .update({
          ...updates,
          updated_at: new Date().toISOString(),
        })
        .eq("id", periodId)
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      toast.success("Période mise à jour");
      queryClient.invalidateQueries({ queryKey: ["orus", "accounting", "periods"] });
    },
    onError: (error) => {
      console.error("Error updating accounting period:", error);
      toast.error("Erreur lors de la mise à jour");
    },
  });
}

export function useUpdatePeriodStatus() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      periodId,
      status,
    }: {
      periodId: string;
      status: AccountingPeriod["status"];
    }) => {
      const { data, error } = await (orusSupabase as any)
        .from("accounting_periods")
        .update({ status, updated_at: new Date().toISOString() })
        .eq("id", periodId)
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      toast.success("Statut mis à jour");
      queryClient.invalidateQueries({ queryKey: ["orus", "accounting", "periods"] });
    },
    onError: (error) => {
      console.error("Error updating period status:", error);
      toast.error("Erreur lors de la mise à jour du statut");
    },
  });
}

// =====================================================
// CHART OF ACCOUNTS HOOKS
// =====================================================

export function useChartOfAccounts() {
  return useQuery({
    queryKey: ["orus", "accounting", "chart_of_accounts"],
    queryFn: async () => {
      const { data, error } = await (orusSupabase as any)
        .from("chart_of_accounts")
        .select("*")
        .eq("is_active", true)
        .order("code");

      if (error) throw error;
      return data as ChartOfAccount[];
    },
  });
}

export function useCreateChartOfAccount() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (account: Partial<ChartOfAccount>) => {
      const { data, error } = await (orusSupabase as any)
        .from("chart_of_accounts")
        .insert(account)
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      toast.success("Compte créé");
      queryClient.invalidateQueries({
        queryKey: ["orus", "accounting", "chart_of_accounts"],
      });
    },
    onError: (error) => {
      console.error("Error creating chart of account:", error);
      toast.error("Erreur lors de la création du compte");
    },
  });
}

// =====================================================
// ENTRIES HOOKS
// =====================================================

export function useAccountingEntries(periodId: string | null) {
  return useQuery({
    queryKey: ["orus", "accounting", "entries", periodId],
    queryFn: async () => {
      if (!periodId) return [];

      const { data, error } = await (orusSupabase as any)
        .from("accounting_entries")
        .select("*")
        .eq("period_id", periodId)
        .order("entry_date", { ascending: false });

      if (error) throw error;
      return data as AccountingEntry[];
    },
    enabled: !!periodId,
  });
}

export function useCreateAccountingEntry() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (entry: Partial<AccountingEntry>) => {
      const { data, error } = await (orusSupabase as any)
        .from("accounting_entries")
        .insert({
          ...entry,
          created_at: new Date().toISOString(),
        })
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      toast.success("Écriture créée");
      queryClient.invalidateQueries({ queryKey: ["orus", "accounting", "entries"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "accounting", "summary"] });
    },
    onError: (error) => {
      console.error("Error creating accounting entry:", error);
      toast.error("Erreur lors de la création de l'écriture");
    },
  });
}

export function useUpdateAccountingEntry() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      entryId,
      updates,
    }: {
      entryId: string;
      updates: Partial<AccountingEntry>;
    }) => {
      const { data, error } = await (orusSupabase as any)
        .from("accounting_entries")
        .update(updates)
        .eq("id", entryId)
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      toast.success("Écriture mise à jour");
      queryClient.invalidateQueries({ queryKey: ["orus", "accounting", "entries"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "accounting", "summary"] });
    },
    onError: (error) => {
      console.error("Error updating accounting entry:", error);
      toast.error("Erreur lors de la mise à jour");
    },
  });
}

export function useDeleteAccountingEntry() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (entryId: string) => {
      const { error } = await (orusSupabase as any)
        .from("accounting_entries")
        .delete()
        .eq("id", entryId);

      if (error) throw error;
    },
    onSuccess: () => {
      toast.success("Écriture supprimée");
      queryClient.invalidateQueries({ queryKey: ["orus", "accounting", "entries"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "accounting", "summary"] });
    },
    onError: (error) => {
      console.error("Error deleting accounting entry:", error);
      toast.error("Erreur lors de la suppression");
    },
  });
}

export function useValidateEntry() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      entryId,
      validatedBy,
    }: {
      entryId: string;
      validatedBy: string;
    }) => {
      const { data, error } = await (orusSupabase as any)
        .from("accounting_entries")
        .update({
          validated: true,
          validated_by: validatedBy,
          validated_at: new Date().toISOString(),
        })
        .eq("id", entryId)
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      toast.success("Écriture validée");
      queryClient.invalidateQueries({ queryKey: ["orus", "accounting", "entries"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "accounting", "summary"] });
    },
    onError: (error) => {
      console.error("Error validating entry:", error);
      toast.error("Erreur lors de la validation");
    },
  });
}

// =====================================================
// REPORTS HOOKS
// =====================================================

export function useAccountingReports(periodId: string | null) {
  return useQuery({
    queryKey: ["orus", "accounting", "reports", periodId],
    queryFn: async () => {
      if (!periodId) return [];

      const { data, error } = await (orusSupabase as any)
        .from("accounting_reports")
        .select("*")
        .eq("period_id", periodId)
        .order("generated_at", { ascending: false });

      if (error) throw error;
      return data as AccountingReport[];
    },
    enabled: !!periodId,
  });
}

// =====================================================
// SUMMARY HOOK
// =====================================================

export function useAccountingSummary(periodId: string | null) {
  const { data: entries } = useAccountingEntries(periodId);
  const { data: chartOfAccounts } = useChartOfAccounts();

  const summary: AccountingSummary | null =
    entries && entries.length > 0
      ? {
          totalDebits: entries.reduce((sum, e) => sum + (e.debit || 0), 0),
          totalCredits: entries.reduce((sum, e) => sum + (e.credit || 0), 0),
          entriesCount: entries.length,
          validatedCount: entries.filter((e) => e.validated).length,
          pendingCount: entries.filter((e) => !e.validated).length,
          balance:
            entries.reduce((sum, e) => sum + (e.debit || 0), 0) -
            entries.reduce((sum, e) => sum + (e.credit || 0), 0),
          byAccountType: entries.reduce(
            (acc, entry) => {
              const account = chartOfAccounts?.find(
                (a) => a.code === entry.account_code
              );
              const type = account?.type || "other";
              if (!acc[type]) {
                acc[type] = { debit: 0, credit: 0 };
              }
              acc[type].debit += entry.debit || 0;
              acc[type].credit += entry.credit || 0;
              return acc;
            },
            {} as Record<string, { debit: number; credit: number }>
          ),
        }
      : null;

  return { summary, entries, chartOfAccounts };
}

// =====================================================
// TRIAL BALANCE HOOK
// =====================================================

export function useTrialBalance(periodId: string | null) {
  const { data: entries } = useAccountingEntries(periodId);
  const { data: chartOfAccounts } = useChartOfAccounts();

  const trialBalance: TrialBalanceEntry[] | null =
    entries && chartOfAccounts
      ? chartOfAccounts
          .map((account) => {
            const accountEntries = entries.filter(
              (e) => e.account_code === account.code
            );
            const debitTotal = accountEntries.reduce(
              (sum, e) => sum + (e.debit || 0),
              0
            );
            const creditTotal = accountEntries.reduce(
              (sum, e) => sum + (e.credit || 0),
              0
            );
            return {
              account_code: account.code,
              account_name: account.name,
              account_type: account.type,
              debit_total: debitTotal,
              credit_total: creditTotal,
              balance: debitTotal - creditTotal,
            };
          })
          .filter((entry) => entry.debit_total > 0 || entry.credit_total > 0)
      : null;

  return { trialBalance };
}

// =====================================================
// MAIN ACCOUNTING HOOK (Combined Interface)
// =====================================================

export function useAccounting() {
  const queryClient = useQueryClient();
  const [isGenerating, setIsGenerating] = useState(false);
  const [selectedPeriodId, setSelectedPeriodId] = useState<string | null>(null);

  const periodsQuery = useAccountingPeriods();
  const chartQuery = useChartOfAccounts();
  const entriesQuery = useAccountingEntries(selectedPeriodId);
  const reportsQuery = useAccountingReports(selectedPeriodId);
  const { summary } = useAccountingSummary(selectedPeriodId);
  const { trialBalance } = useTrialBalance(selectedPeriodId);

  const createPeriod = useCreateAccountingPeriod();
  const updatePeriodStatus = useUpdatePeriodStatus();
  const createEntry = useCreateAccountingEntry();
  const updateEntry = useUpdateAccountingEntry();
  const deleteEntry = useDeleteAccountingEntry();
  const validateEntry = useValidateEntry();

  // Generate accounting entries via Edge Function
  const generateEntries = async (periodId: string, startDate: string, endDate: string) => {
    setIsGenerating(true);
    try {
      const { data, error } = await orusSupabase.functions.invoke(
        "generate-accounting-report",
        {
          body: { action: "generate_entries", periodId, startDate, endDate },
        }
      );

      if (error) throw error;

      queryClient.invalidateQueries({
        queryKey: ["orus", "accounting", "entries", periodId],
      });
      toast.success(
        `${data.entriesGenerated} écritures comptables ont été générées.`
      );
      return data;
    } catch (error: any) {
      toast.error(error.message || "Erreur lors de la génération");
      throw error;
    } finally {
      setIsGenerating(false);
    }
  };

  // Validate all entries for a period
  const validateAllEntries = async (periodId: string) => {
    try {
      const { data, error } = await orusSupabase.functions.invoke(
        "generate-accounting-report",
        {
          body: { action: "validate_entries", periodId },
        }
      );

      if (error) throw error;

      queryClient.invalidateQueries({
        queryKey: ["orus", "accounting", "entries", periodId],
      });
      toast.success(
        `${data.validated} écritures validées. ${data.issues?.length || 0} problèmes détectés.`
      );
      return data;
    } catch (error: any) {
      toast.error(error.message || "Erreur lors de la validation");
      throw error;
    }
  };

  // Generate financial report
  const generateReport = async (
    periodId: string,
    reportType: "balance_sheet" | "profit_loss" | "trial_balance"
  ) => {
    setIsGenerating(true);
    try {
      const { data, error } = await orusSupabase.functions.invoke(
        "generate-accounting-report",
        {
          body: { action: "generate_report", periodId, reportType },
        }
      );

      if (error) throw error;

      queryClient.invalidateQueries({
        queryKey: ["orus", "accounting", "reports", periodId],
      });
      toast.success("Le rapport financier a été généré avec succès.");
      return data;
    } catch (error: any) {
      toast.error(error.message || "Erreur lors de la génération du rapport");
      throw error;
    } finally {
      setIsGenerating(false);
    }
  };

  // Get AI analysis
  const getAIAnalysis = async (periodId: string) => {
    try {
      const { data, error } = await orusSupabase.functions.invoke(
        "generate-accounting-report",
        {
          body: { action: "ai_analysis", periodId },
        }
      );

      if (error) throw error;
      return data;
    } catch (error: any) {
      toast.error(error.message || "Erreur lors de l'analyse AI");
      throw error;
    }
  };

  return {
    // Data
    periods: periodsQuery.data || [],
    chartOfAccounts: chartQuery.data || [],
    entries: entriesQuery.data || [],
    reports: reportsQuery.data || [],
    summary,
    trialBalance,
    selectedPeriodId,
    setSelectedPeriodId,

    // Loading states
    isLoading:
      periodsQuery.isLoading || chartQuery.isLoading || entriesQuery.isLoading,
    isGenerating,

    // Mutations
    createPeriod,
    updatePeriodStatus,
    createEntry,
    updateEntry,
    deleteEntry,
    validateEntry,

    // Functions
    generateEntries,
    validateAllEntries,
    generateReport,
    getAIAnalysis,
  };
}
