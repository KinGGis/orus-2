/**
 * Private Equity Hooks
 * Complete CRUD operations and portfolio metrics calculations
 * Based on Orus usePrivateEquity.ts
 */
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { orusSupabase } from "./supabase-client";
import { toast } from "sonner";

// =====================================================
// TYPES
// =====================================================

export interface PECompany {
  id: string;
  position_id: string | null;
  name: string;
  sector: string | null;
  country: string | null;
  business_model: string;
  is_cfo_managed: boolean;
  currency: string;
  fiscal_year_end: number;
  description: string | null;
  website: string | null;
  founded_year: number | null;
  employees_count: number | null;
  created_at: string;
  updated_at: string;
}

export interface PEInvestment {
  id: string;
  company_id: string;
  investment_date: string;
  investment_amount: number;
  valuation_at_entry: number | null;
  ownership_percentage: number | null;
  round_name: string | null;
  current_valuation: number | null;
  realized_value: number;
  unrealized_value: number | null;
  exit_date: string | null;
  exit_value: number | null;
  investment_type: string;
  notes: string | null;
  created_at?: string;
  updated_at?: string;
}

export interface PEPortfolioMetrics {
  totalInvested: number;
  totalCurrentValue: number;
  totalRealizedValue: number;
  totalUnrealizedValue: number;
  tvpi: number;
  moic: number;
  dpi: number;
  rvpi: number;
  irr: number;
  companiesCount: number;
  cfoManagedCount: number;
  activeInvestmentsCount: number;
}

export interface PEPosition {
  company: PECompany;
  investment: PEInvestment | null;
  investments: PEInvestment[];
  totalInvested: number;
  totalValue: number;
  moic: number;
  unrealizedGain: number;
  role: string;
}

// =====================================================
// COMPANY HOOKS
// =====================================================

export function usePECompanies() {
  return useQuery({
    queryKey: ["orus", "pe", "companies"],
    queryFn: async (): Promise<PECompany[]> => {
      const { data, error } = await (orusSupabase as any)
        .from("pe_companies")
        .select("*")
        .order("name");

      if (error) throw error;
      return data || [];
    },
  });
}

export function usePECompany(companyId: string) {
  return useQuery({
    queryKey: ["orus", "pe", "company", companyId],
    queryFn: async (): Promise<PECompany | null> => {
      const { data, error } = await (orusSupabase as any)
        .from("pe_companies")
        .select("*")
        .eq("id", companyId)
        .single();

      if (error) {
        if (error.code === "PGRST116") return null;
        throw error;
      }
      return data;
    },
    enabled: !!companyId,
  });
}

export function useCreatePECompany() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (company: Partial<PECompany>) => {
      const { data, error } = await (orusSupabase as any)
        .from("pe_companies")
        .insert({
          ...company,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        })
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      toast.success("Société créée avec succès");
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "companies"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "positions"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "metrics"] });
    },
    onError: (error) => {
      console.error("Error creating PE company:", error);
      toast.error("Erreur lors de la création de la société");
    },
  });
}

export function useUpdatePECompany() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ id, ...updates }: Partial<PECompany> & { id: string }) => {
      const { data, error } = await (orusSupabase as any)
        .from("pe_companies")
        .update({
          ...updates,
          updated_at: new Date().toISOString(),
        })
        .eq("id", id)
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: (data) => {
      toast.success("Société mise à jour");
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "companies"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "company", data.id] });
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "positions"] });
    },
    onError: (error) => {
      console.error("Error updating PE company:", error);
      toast.error("Erreur lors de la mise à jour");
    },
  });
}

export function useDeletePECompany() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (companyId: string) => {
      const { error } = await (orusSupabase as any)
        .from("pe_companies")
        .delete()
        .eq("id", companyId);

      if (error) throw error;
    },
    onSuccess: () => {
      toast.success("Société supprimée");
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "companies"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "positions"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "metrics"] });
    },
    onError: (error) => {
      console.error("Error deleting PE company:", error);
      toast.error("Erreur lors de la suppression");
    },
  });
}

// =====================================================
// INVESTMENT HOOKS
// =====================================================

export function usePEInvestments(companyId?: string) {
  return useQuery({
    queryKey: ["orus", "pe", "investments", companyId],
    queryFn: async (): Promise<PEInvestment[]> => {
      let query = (orusSupabase as any)
        .from("pe_investments")
        .select("*");

      if (companyId) {
        query = query.eq("company_id", companyId);
      }

      const { data, error } = await query.order("investment_date", { ascending: false });
      if (error) throw error;
      return data || [];
    },
  });
}

export function useCreatePEInvestment() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (investment: Partial<PEInvestment>) => {
      const { data, error } = await (orusSupabase as any)
        .from("pe_investments")
        .insert({
          ...investment,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        })
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      toast.success("Investissement créé");
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "investments"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "metrics"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "positions"] });
    },
    onError: (error) => {
      console.error("Error creating PE investment:", error);
      toast.error("Erreur lors de la création de l'investissement");
    },
  });
}

export function useUpdatePEInvestment() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ id, ...updates }: Partial<PEInvestment> & { id: string }) => {
      const { data, error } = await (orusSupabase as any)
        .from("pe_investments")
        .update({
          ...updates,
          updated_at: new Date().toISOString(),
        })
        .eq("id", id)
        .select()
        .single();

      if (error) throw error;
      return data;
    },
    onSuccess: () => {
      toast.success("Investissement mis à jour");
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "investments"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "metrics"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "positions"] });
    },
    onError: (error) => {
      console.error("Error updating PE investment:", error);
      toast.error("Erreur lors de la mise à jour");
    },
  });
}

export function useDeletePEInvestment() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (investmentId: string) => {
      const { error } = await (orusSupabase as any)
        .from("pe_investments")
        .delete()
        .eq("id", investmentId);

      if (error) throw error;
    },
    onSuccess: () => {
      toast.success("Investissement supprimé");
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "investments"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "metrics"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "pe", "positions"] });
    },
    onError: (error) => {
      console.error("Error deleting PE investment:", error);
      toast.error("Erreur lors de la suppression");
    },
  });
}

// =====================================================
// PORTFOLIO METRICS HOOK
// =====================================================

export function usePEPortfolioMetrics() {
  return useQuery({
    queryKey: ["orus", "pe", "metrics"],
    queryFn: async (): Promise<PEPortfolioMetrics> => {
      const { data: companies, error: companiesError } = await (orusSupabase as any)
        .from("pe_companies")
        .select("*");

      if (companiesError) throw companiesError;

      const { data: investments, error: investmentsError } = await (orusSupabase as any)
        .from("pe_investments")
        .select("*");

      if (investmentsError) throw investmentsError;

      if (!investments || investments.length === 0) {
        return {
          totalInvested: 0,
          totalCurrentValue: 0,
          totalRealizedValue: 0,
          totalUnrealizedValue: 0,
          tvpi: 0,
          moic: 0,
          dpi: 0,
          rvpi: 0,
          irr: 0,
          companiesCount: companies?.length || 0,
          cfoManagedCount: companies?.filter((c: PECompany) => c.is_cfo_managed).length || 0,
          activeInvestmentsCount: 0,
        };
      }

      const totalInvested = investments.reduce(
        (sum: number, inv: PEInvestment) => sum + (inv.investment_amount || 0),
        0
      );
      const totalRealizedValue = investments.reduce(
        (sum: number, inv: PEInvestment) => sum + (inv.realized_value || 0),
        0
      );
      const totalUnrealizedValue = investments.reduce(
        (sum: number, inv: PEInvestment) => sum + (inv.unrealized_value || 0),
        0
      );
      const totalCurrentValue = totalRealizedValue + totalUnrealizedValue;

      const tvpi = totalInvested > 0 ? totalCurrentValue / totalInvested : 0;
      const moic = tvpi;
      const dpi = totalInvested > 0 ? totalRealizedValue / totalInvested : 0;
      const rvpi = totalInvested > 0 ? totalUnrealizedValue / totalInvested : 0;

      // Calculate IRR using simplified weighted average annualized return
      let weightedReturn = 0;
      let totalWeight = 0;

      investments.forEach((inv: PEInvestment) => {
        if (inv.investment_amount > 0) {
          const currentValue = (inv.realized_value || 0) + (inv.unrealized_value || 0);
          const years =
            (Date.now() - new Date(inv.investment_date).getTime()) /
            (365.25 * 24 * 60 * 60 * 1000);
          if (years > 0) {
            const annualizedReturn = Math.pow(currentValue / inv.investment_amount, 1 / years) - 1;
            weightedReturn += annualizedReturn * inv.investment_amount;
            totalWeight += inv.investment_amount;
          }
        }
      });

      const irr = totalWeight > 0 ? (weightedReturn / totalWeight) * 100 : 0;

      const activeInvestmentsCount = investments.filter(
        (inv: PEInvestment) => !inv.exit_date
      ).length;

      return {
        totalInvested,
        totalCurrentValue,
        totalRealizedValue,
        totalUnrealizedValue,
        tvpi,
        moic,
        dpi,
        rvpi,
        irr,
        companiesCount: companies?.length || 0,
        cfoManagedCount: companies?.filter((c: PECompany) => c.is_cfo_managed).length || 0,
        activeInvestmentsCount,
      };
    },
  });
}

// =====================================================
// POSITIONS HOOK (Combined Company + Investments)
// =====================================================

export function usePEPositions() {
  return useQuery({
    queryKey: ["orus", "pe", "positions"],
    queryFn: async (): Promise<PEPosition[]> => {
      const { data: companies, error: companiesError } = await (orusSupabase as any)
        .from("pe_companies")
        .select("*")
        .order("name");

      if (companiesError) throw companiesError;
      if (!companies || companies.length === 0) return [];

      const { data: investments, error: investmentsError } = await (orusSupabase as any)
        .from("pe_investments")
        .select("*")
        .order("investment_date", { ascending: false });

      if (investmentsError) throw investmentsError;

      return companies.map((company: PECompany) => {
        const companyInvestments =
          investments?.filter((inv: PEInvestment) => inv.company_id === company.id) || [];
        const latestInvestment = companyInvestments[0] || null;

        const totalInvested = companyInvestments.reduce(
          (sum: number, inv: PEInvestment) => sum + (inv.investment_amount || 0),
          0
        );
        const totalValue = companyInvestments.reduce(
          (sum: number, inv: PEInvestment) =>
            sum + (inv.realized_value || 0) + (inv.unrealized_value || 0),
          0
        );

        const moic = totalInvested > 0 ? totalValue / totalInvested : 0;
        const unrealizedGain = totalValue - totalInvested;
        const role = company.is_cfo_managed ? "CFO" : "Board/Investor";

        return {
          company,
          investment: latestInvestment,
          investments: companyInvestments,
          totalInvested,
          totalValue,
          moic,
          unrealizedGain,
          role,
        };
      });
    },
  });
}

// =====================================================
// SIMULATION HOOK
// =====================================================

export interface PESimulationParams {
  entryValuation: number;
  investmentAmount: number;
  ownershipPercent: number;
  holdingPeriodYears: number;
  exitMultiple: number;
  dilutionPercent: number;
}

export interface PESimulationResult {
  exitValuation: number;
  grossReturn: number;
  netReturn: number;
  irr: number;
  moic: number;
  finalOwnership: number;
}

export function usePESimulation() {
  const simulate = (params: PESimulationParams): PESimulationResult => {
    const {
      entryValuation,
      investmentAmount,
      ownershipPercent,
      holdingPeriodYears,
      exitMultiple,
      dilutionPercent,
    } = params;

    // Calculate exit valuation
    const exitValuation = entryValuation * exitMultiple;

    // Apply dilution to ownership
    const finalOwnership = ownershipPercent * (1 - dilutionPercent / 100);

    // Calculate returns
    const exitProceeds = exitValuation * (finalOwnership / 100);
    const grossReturn = exitProceeds - investmentAmount;
    const netReturn = grossReturn; // Could add fees here

    // Calculate MOIC
    const moic = investmentAmount > 0 ? exitProceeds / investmentAmount : 0;

    // Calculate IRR
    const irr =
      holdingPeriodYears > 0 && investmentAmount > 0
        ? (Math.pow(exitProceeds / investmentAmount, 1 / holdingPeriodYears) - 1) * 100
        : 0;

    return {
      exitValuation,
      grossReturn,
      netReturn,
      irr,
      moic,
      finalOwnership,
    };
  };

  return { simulate };
}
