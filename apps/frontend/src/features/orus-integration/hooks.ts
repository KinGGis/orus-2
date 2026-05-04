/**
 * React Query hooks for accessing Orus data from Supabase
 */
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { orusSupabase } from "./supabase-client";
import type {
  OrusPosition,
  OrusInstrument,
  OrusTrade,
  OrusCashPosition,
  OrusNav,
  OrusPortfolioSummary,
  OrusPECompany,
  OrusPEInvestment,
  OrusAccountingEntry,
  OrusAccountingPeriod,
  OrusChartOfAccounts,
  OrusRevolutAccount,
  OrusRevolutTransaction,
  OrusShareholderParticipation,
} from "./types";

// =====================================================
// POSITIONS HOOKS
// =====================================================

export function useOrusPositions(options?: { date?: string; status?: string }) {
  return useQuery({
    queryKey: ["orus", "positions", options],
    queryFn: async () => {
      let query = orusSupabase.from("positions").select("*").order("date", { ascending: false });

      if (options?.date) {
        query = query.eq("date", options.date);
      }
      if (options?.status) {
        query = query.eq("status", options.status);
      }

      const { data, error } = await query;
      if (error) throw error;
      return data as OrusPosition[];
    },
  });
}

export function useOrusLatestPositions() {
  return useQuery({
    queryKey: ["orus", "positions", "latest"],
    queryFn: async () => {
      // Get the most recent date with positions
      const { data: latestDateResult } = await orusSupabase
        .from("positions")
        .select("date")
        .order("date", { ascending: false })
        .limit(1)
        .single();

      if (!latestDateResult) return [];
      const latestDate = (latestDateResult as { date: string }).date;

      const { data, error } = await orusSupabase
        .from("positions")
        .select("*")
        .eq("date", latestDate)
        .eq("status", "open");

      if (error) throw error;
      return data as OrusPosition[];
    },
  });
}

// =====================================================
// INSTRUMENTS HOOKS
// =====================================================

export function useOrusInstruments() {
  return useQuery({
    queryKey: ["orus", "instruments"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("instruments")
        .select("*")
        .eq("is_active", true)
        .order("symbol");

      if (error) throw error;
      return data as OrusInstrument[];
    },
  });
}

export function useOrusInstrument(id: string) {
  return useQuery({
    queryKey: ["orus", "instruments", id],
    queryFn: async () => {
      const { data, error } = await orusSupabase.from("instruments").select("*").eq("id", id).single();

      if (error) throw error;
      return data as OrusInstrument;
    },
    enabled: !!id,
  });
}

// =====================================================
// TRADES HOOKS
// =====================================================

export function useOrusTrades(options?: { startDate?: string; endDate?: string }) {
  return useQuery({
    queryKey: ["orus", "trades", options],
    queryFn: async () => {
      let query = orusSupabase.from("trades").select("*").order("date", { ascending: false });

      if (options?.startDate) {
        query = query.gte("date", options.startDate);
      }
      if (options?.endDate) {
        query = query.lte("date", options.endDate);
      }

      const { data, error } = await query;
      if (error) throw error;
      return data as OrusTrade[];
    },
  });
}

// =====================================================
// CASH POSITIONS HOOKS
// =====================================================

export function useOrusCashPositions() {
  return useQuery({
    queryKey: ["orus", "cash_positions"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("cash_positions")
        .select("*")
        .order("date", { ascending: false });

      if (error) throw error;
      return data as OrusCashPosition[];
    },
  });
}

export function useOrusLatestCash() {
  return useQuery({
    queryKey: ["orus", "cash_positions", "latest"],
    queryFn: async () => {
      // Get latest cash by currency
      const { data, error } = await orusSupabase.rpc("get_latest_cash_by_currency");

      if (error) {
        // Fallback if RPC doesn't exist
        const { data: fallback, error: fallbackError } = await orusSupabase
          .from("cash_positions")
          .select("*")
          .order("date", { ascending: false })
          .limit(10);

        if (fallbackError) throw fallbackError;
        return fallback as OrusCashPosition[];
      }

      return data as OrusCashPosition[];
    },
  });
}

// =====================================================
// NAV HOOKS
// =====================================================

export function useOrusNavHistory(days: number = 30) {
  return useQuery({
    queryKey: ["orus", "navs", days],
    queryFn: async () => {
      const startDate = new Date();
      startDate.setDate(startDate.getDate() - days);

      const { data, error } = await orusSupabase
        .from("navs")
        .select("*")
        .gte("date", startDate.toISOString().split("T")[0])
        .order("date", { ascending: true });

      if (error) throw error;
      return data as OrusNav[];
    },
  });
}

// =====================================================
// PORTFOLIO SUMMARY HOOKS
// =====================================================

export function useOrusPortfolioSummary() {
  return useQuery({
    queryKey: ["orus", "portfolio_summary"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("portfolio_summary")
        .select("*")
        .order("date", { ascending: false })
        .limit(1)
        .single();

      if (error && error.code !== "PGRST116") throw error; // PGRST116 = no rows
      return data as OrusPortfolioSummary | null;
    },
  });
}

// =====================================================
// PRIVATE EQUITY HOOKS
// =====================================================

export function useOrusPECompanies() {
  return useQuery({
    queryKey: ["orus", "pe_companies"],
    queryFn: async () => {
      const { data, error } = await orusSupabase.from("pe_companies").select("*").order("name");

      if (error) throw error;
      return data as OrusPECompany[];
    },
  });
}

export function useOrusPECompany(id: string) {
  return useQuery({
    queryKey: ["orus", "pe_companies", id],
    queryFn: async () => {
      const { data, error } = await orusSupabase.from("pe_companies").select("*").eq("id", id).single();

      if (error) throw error;
      return data as OrusPECompany;
    },
    enabled: !!id,
  });
}

export function useOrusPEInvestments(companyId?: string) {
  return useQuery({
    queryKey: ["orus", "pe_investments", companyId],
    queryFn: async () => {
      let query = orusSupabase.from("pe_investments").select("*").order("investment_date", { ascending: false });

      if (companyId) {
        query = query.eq("company_id", companyId);
      }

      const { data, error } = await query;
      if (error) throw error;
      return data as OrusPEInvestment[];
    },
  });
}

// =====================================================
// ACCOUNTING HOOKS
// =====================================================

export function useOrusAccountingEntries(periodId?: string) {
  return useQuery({
    queryKey: ["orus", "accounting_entries", periodId],
    queryFn: async () => {
      let query = orusSupabase.from("accounting_entries").select("*").order("entry_date", { ascending: false });

      if (periodId) {
        query = query.eq("period_id", periodId);
      }

      const { data, error } = await query;
      if (error) throw error;
      return data as OrusAccountingEntry[];
    },
  });
}

export function useOrusAccountingPeriods() {
  return useQuery({
    queryKey: ["orus", "accounting_periods"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("accounting_periods")
        .select("*")
        .order("start_date", { ascending: false });

      if (error) throw error;
      return data as OrusAccountingPeriod[];
    },
  });
}

export function useOrusChartOfAccounts() {
  return useQuery({
    queryKey: ["orus", "chart_of_accounts"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("chart_of_accounts")
        .select("*")
        .eq("is_active", true)
        .order("code");

      if (error) throw error;
      return data as OrusChartOfAccounts[];
    },
  });
}

// =====================================================
// REVOLUT HOOKS
// =====================================================

export function useOrusRevolutAccounts() {
  return useQuery({
    queryKey: ["orus", "revolut_accounts"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("revolut_accounts")
        .select("*")
        .order("name");

      if (error) throw error;
      return data as OrusRevolutAccount[];
    },
  });
}

export function useOrusRevolutTransactions(accountId?: string) {
  return useQuery({
    queryKey: ["orus", "revolut_transactions", accountId],
    queryFn: async () => {
      let query = orusSupabase
        .from("revolut_transactions")
        .select("*")
        .order("completed_at", { ascending: false })
        .limit(500);

      if (accountId) {
        query = query.eq("revolut_account_id", accountId);
      }

      const { data, error } = await query;
      if (error) throw error;
      return data as OrusRevolutTransaction[];
    },
  });
}

// =====================================================
// SHAREHOLDER HOOKS
// =====================================================

export function useOrusShareholderParticipations() {
  return useQuery({
    queryKey: ["orus", "shareholder_participations"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("shareholder_participations")
        .select(`
          *,
          profiles:user_id (
            full_name,
            email
          )
        `)
        .order("total_value", { ascending: false });

      if (error) throw error;
      return data as OrusShareholderParticipation[];
    },
  });
}

// =====================================================
// AUTH HOOKS
// =====================================================

export function useOrusAuth() {
  const queryClient = useQueryClient();

  const signIn = useMutation({
    mutationFn: async ({ email, password }: { email: string; password: string }) => {
      const { data, error } = await orusSupabase.auth.signInWithPassword({
        email,
        password,
      });
      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus"] });
    },
  });

  const signOut = useMutation({
    mutationFn: async () => {
      const { error } = await orusSupabase.auth.signOut();
      if (error) throw error;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus"] });
    },
  });

  const session = useQuery({
    queryKey: ["orus", "session"],
    queryFn: async () => {
      const {
        data: { session },
      } = await orusSupabase.auth.getSession();
      return session;
    },
    staleTime: 5 * 60 * 1000, // 5 minutes
  });

  return {
    session: session.data,
    isLoading: session.isLoading,
    isAuthenticated: !!session.data,
    signIn,
    signOut,
  };
}

// =====================================================
// REVOLUT SYNC HOOKS
// =====================================================

interface RevolutSyncLog {
  id: string;
  sync_type: string;
  status: string;
  started_at: string;
  completed_at: string | null;
  accounts_synced: number | null;
  transactions_synced: number | null;
  error_message: string | null;
}

export function useRevolutConnection() {
  return useQuery({
    queryKey: ["orus", "revolut_connection"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("revolut_auth_tokens")
        .select("expires_at, created_at")
        .order("created_at", { ascending: false })
        .limit(1)
        .maybeSingle();

      if (error) throw error;

      if (data && new Date(data.expires_at) > new Date()) {
        return {
          isConnected: true,
          tokenExpiry: data.expires_at,
        };
      }

      return {
        isConnected: false,
        tokenExpiry: null,
      };
    },
    staleTime: 60 * 1000, // 1 minute
  });
}

export function useRevolutSyncLogs() {
  return useQuery({
    queryKey: ["orus", "revolut_sync_logs"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("revolut_sync_log")
        .select("*")
        .order("started_at", { ascending: false })
        .limit(10);

      if (error) throw error;
      return data as RevolutSyncLog[];
    },
  });
}

const ORUS_SUPABASE_URL = "https://bujfwfqmfsgaibnmuvml.supabase.co";

export function useRevolutSync() {
  const queryClient = useQueryClient();

  const initiateConnection = useMutation({
    mutationFn: async (redirectUri?: string) => {
      const appRedirectUri = redirectUri || `${window.location.origin}/orus/accounting`;
      
      const res = await fetch(
        `${ORUS_SUPABASE_URL}/functions/v1/revolut-auth?app_redirect_uri=${encodeURIComponent(appRedirectUri)}`
      );
      const data = await res.json();

      if (!res.ok || !data?.authorization_url) {
        throw new Error(data?.error || "URL d'autorisation Revolut introuvable");
      }

      // Redirect to Revolut authorization
      window.location.href = data.authorization_url as string;
      return data;
    },
  });

  const syncData = useMutation({
    mutationFn: async () => {
      const { data, error } = await orusSupabase.functions.invoke("revolut-sync");

      if (error) throw error;
      return data as { accounts_synced: number; transactions_synced: number };
    },
    onSuccess: () => {
      // Invalidate all Revolut-related queries
      queryClient.invalidateQueries({ queryKey: ["orus", "revolut_accounts"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "revolut_transactions"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "revolut_connection"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "revolut_sync_logs"] });
    },
  });

  return {
    initiateConnection,
    syncData,
  };
}
