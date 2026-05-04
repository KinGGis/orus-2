/**
 * Admin Hooks for Orus Integration
 * Manages users, permissions, invitations, and integrations (IBKR, Revolut, Market Data)
 */
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { orusSupabase } from "./supabase-client";
import { toast } from "sonner";
import { useCallback, useEffect, useState } from "react";

// =====================================================
// TYPES
// =====================================================

export type UserRole = "superadmin" | "admin" | "analyst" | "trader" | "portfolio_manager" | "viewer";

export interface UserWithRole {
  user_id: string;
  email: string;
  full_name: string;
  role: UserRole;
  created_at: string;
}

export interface UserInvitation {
  id: string;
  email: string;
  role: UserRole;
  status: "pending" | "accepted" | "expired" | "cancelled";
  invited_by?: string;
  invited_by_email?: string;
  created_at: string;
  expires_at?: string;
}

export interface Permission {
  resource: string;
  action: string;
  allowed: boolean;
}

export interface IBKRStatus {
  connected: boolean;
  account_id?: string;
  cash?: number;
  net_liq?: number;
  gross_position_value?: number;
}

export interface IBKRPosition {
  symbol: string;
  qty: number;
  avg_cost: number;
  market_value: number;
  pnl: number;
}

export interface MarketDataConfig {
  id?: string;
  is_enabled: boolean;
  api_key?: string;
  provider?: string;
  cron_schedule?: string;
  last_run_at?: string;
  next_run_at?: string;
  python_service_url?: string;
}

export interface RevolutStatus {
  connected: boolean;
  last_sync?: string;
}

// =====================================================
// USERS HOOKS
// =====================================================

export function useOrusUsers() {
  return useQuery({
    queryKey: ["orus", "admin", "users"],
    queryFn: async () => {
      // First check current user
      const { data: { user } } = await orusSupabase.auth.getUser();
      console.log("[Admin] Current user:", user?.id, user?.email);

      // Fetch profiles
      const { data: profiles, error: profilesError } = await orusSupabase
        .from("profiles")
        .select("id, email, full_name, created_at");

      console.log("[Admin] Profiles fetched:", profiles?.length, "error:", profilesError?.message);
      if (profilesError) {
        console.error("[Admin] Profiles error:", profilesError);
        throw profilesError;
      }

      // Fetch all roles (may be limited by RLS to only own role for non-admins)
      const { data: roles, error: rolesError } = await orusSupabase
        .from("user_roles")
        .select("user_id, role, created_at");

      console.log("[Admin] Roles fetched:", roles?.length, "error:", rolesError?.message);
      // Don't throw on roles error - user might not have admin access to see all roles
      if (rolesError) {
        console.warn("[Admin] Roles query limited by RLS:", rolesError.message);
      }

      // Combine
      const usersWithRoles: UserWithRole[] = (profiles || []).map((profile) => {
        const userRole = roles?.find((r) => r.user_id === profile.id);
        return {
          user_id: profile.id,
          email: profile.email || "",
          full_name: profile.full_name || "",
          role: (userRole?.role as UserRole) || "viewer",
          created_at: profile.created_at || userRole?.created_at || "",
        };
      });

      console.log("[Admin] Combined users:", usersWithRoles.length);
      return usersWithRoles;
    },
  });
}

export function useUpdateUserRole() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ userId, role }: { userId: string; role: UserRole }) => {
      // Check if role exists
      const { data: existingRole, error: checkError } = await orusSupabase
        .from("user_roles")
        .select("id")
        .eq("user_id", userId)
        .maybeSingle();

      if (checkError) throw checkError;

      if (existingRole) {
        const { error } = await orusSupabase.from("user_roles").update({ role }).eq("user_id", userId);
        if (error) throw error;
      } else {
        const { error } = await orusSupabase.from("user_roles").insert({ user_id: userId, role });
        if (error) throw error;
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "users"] });
      toast.success("Rôle mis à jour");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

export function useDeleteUser() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (userId: string) => {
      const response = await orusSupabase.functions.invoke("delete-user", {
        body: { userId },
      });

      if (response.error) throw response.error;
      if (response.data?.error) throw new Error(response.data.error);

      return response.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "users"] });
      toast.success("Utilisateur supprimé");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

// =====================================================
// INVITATIONS HOOKS
// =====================================================

export function useInvitations() {
  return useQuery({
    queryKey: ["orus", "admin", "invitations"],
    queryFn: async () => {
      console.log("[Admin] Fetching invitations...");
      
      const { data, error } = await orusSupabase
        .from("user_invitations")
        .select(`
          id,
          email,
          role,
          status,
          invited_by,
          created_at,
          expires_at,
          profiles:invited_by (email)
        `)
        .order("created_at", { ascending: false });

      console.log("[Admin] Invitations fetched:", data?.length, "error:", error?.message);
      
      if (error) {
        console.error("[Admin] Invitations error:", error);
        // Return empty array instead of throwing for RLS issues
        return [] as UserInvitation[];
      }

      return (data || []).map((inv) => ({
        ...inv,
        invited_by_email: (inv.profiles as { email: string } | null)?.email,
      })) as UserInvitation[];
    },
  });
}

export function useSendInvitation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ email, role }: { email: string; role: UserRole }) => {
      // Use the invite-user Edge Function which handles Supabase Auth admin operations
      const response = await orusSupabase.functions.invoke("invite-user", {
        body: { email, role },
      });

      if (response.error) {
        throw new Error(response.error.message);
      }

      if (response.data?.error) {
        throw new Error(response.data.error);
      }

      return response.data;
    },
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "invitations"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "users"] });
      
      if (data?.password_reset_sent) {
        toast.success(data.message || "Lien de récupération envoyé");
      } else if (data?.assigned_directly) {
        toast.success(data.message || "Rôle assigné directement");
      } else {
        toast.success("Invitation envoyée");
      }
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

export function useCancelInvitation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (invitationId: string) => {
      const { error } = await orusSupabase
        .from("user_invitations")
        .update({ status: "cancelled" })
        .eq("id", invitationId);

      if (error) throw error;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "invitations"] });
      toast.success("Invitation annulée");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

export function useResendInvitation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (invitationId: string) => {
      // Get invitation details
      const { data: invitation, error: fetchError } = await orusSupabase
        .from("user_invitations")
        .select("*")
        .eq("id", invitationId)
        .single();

      if (fetchError) throw fetchError;

      // Update expiry
      const { error: updateError } = await orusSupabase
        .from("user_invitations")
        .update({
          status: "pending",
          expires_at: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString(),
        })
        .eq("id", invitationId);

      if (updateError) throw updateError;

      // Resend magic link
      const { error: authError } = await orusSupabase.auth.signInWithOtp({
        email: invitation.email,
        options: {
          data: {
            invited_by: invitation.invited_by,
            role: invitation.role,
            invitation_id: invitation.id,
          },
        },
      });

      if (authError) throw authError;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "invitations"] });
      toast.success("Invitation renvoyée");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

// =====================================================
// PERMISSIONS HOOKS
// =====================================================

export function useRolePermissions(role: UserRole) {
  return useQuery({
    queryKey: ["orus", "admin", "permissions", role],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("role_permissions")
        .select("resource, action, allowed")
        .eq("role", role);

      if (error) throw error;
      return data as Permission[];
    },
    enabled: !!role,
  });
}

export function useUpdatePermission() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      role,
      resource,
      action,
      allowed,
    }: {
      role: UserRole;
      resource: string;
      action: string;
      allowed: boolean;
    }) => {
      const { error } = await orusSupabase.from("role_permissions").upsert(
        {
          role,
          resource,
          action,
          allowed,
        },
        { onConflict: "role,resource,action" }
      );

      if (error) throw error;
    },
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "permissions", variables.role] });
      toast.success("Permission mise à jour");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

// =====================================================
// IBKR HOOKS
// =====================================================

export function useIBKRStatus() {
  return useQuery({
    queryKey: ["orus", "admin", "ibkr", "status"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("ibkr_connections")
        .select("*")
        .order("created_at", { ascending: false })
        .limit(1)
        .maybeSingle();

      if (error) throw error;

      if (!data) {
        return { connected: false } as IBKRStatus;
      }

      return {
        connected: data.status === "connected",
        account_id: data.account_id,
        cash: data.cash,
        net_liq: data.net_liq,
        gross_position_value: data.gross_position_value,
      } as IBKRStatus;
    },
  });
}

export function useIBKRPositions() {
  return useQuery({
    queryKey: ["orus", "admin", "ibkr", "positions"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("ibkr_positions")
        .select("*")
        .order("symbol");

      if (error) throw error;

      return (data || []).map((p) => ({
        symbol: p.symbol,
        qty: p.quantity,
        avg_cost: p.avg_cost,
        market_value: p.market_value,
        pnl: p.unrealized_pnl,
      })) as IBKRPosition[];
    },
  });
}

export function useSyncIBKR() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => {
      const response = await orusSupabase.functions.invoke("sync-ibkr");

      if (response.error) throw response.error;
      return response.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "ibkr"] });
      toast.success("Synchronisation IBKR réussie");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

export function useConnectIBKR() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => {
      const response = await orusSupabase.functions.invoke("connect-ibkr");

      if (response.error) throw response.error;
      return response.data;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "ibkr"] });
      toast.success("Connexion IBKR initiée");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

export function useDisconnectIBKR() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => {
      const { error } = await orusSupabase
        .from("ibkr_connections")
        .update({ status: "disconnected" })
        .eq("status", "connected");

      if (error) throw error;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "ibkr"] });
      toast.success("Déconnexion IBKR réussie");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

// =====================================================
// MARKET DATA HOOKS
// =====================================================

export function useMarketDataConfig() {
  return useQuery({
    queryKey: ["orus", "admin", "market-data", "config"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("market_data_sync_config")
        .select("*")
        .maybeSingle();

      if (error) throw error;

      return (data || {
        is_enabled: false,
        provider: "alpha_vantage",
      }) as MarketDataConfig;
    },
  });
}

export function useUpdateMarketDataConfig() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (config: Partial<MarketDataConfig>) => {
      // Get existing record or use default ID
      const { data: existing } = await orusSupabase
        .from("market_data_sync_config")
        .select("id")
        .maybeSingle();

      const recordId = existing?.id || "00000000-0000-0000-0000-000000000001";

      const { error } = await orusSupabase.from("market_data_sync_config").upsert(
        {
          id: recordId,
          ...config,
          updated_at: new Date().toISOString(),
        },
        { onConflict: "id" }
      );

      if (error) throw error;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "market-data"] });
      toast.success("Configuration sauvegardée");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

// =====================================================
// REVOLUT HOOKS (Admin)
// =====================================================

export function useRevolutStatus() {
  return useQuery({
    queryKey: ["orus", "admin", "revolut", "status"],
    queryFn: async () => {
      const { data, error } = await orusSupabase
        .from("revolut_auth_tokens")
        .select("*")
        .order("created_at", { ascending: false })
        .limit(1)
        .maybeSingle();

      if (error) throw error;

      if (!data) {
        return { connected: false } as RevolutStatus;
      }

      const isValid = new Date(data.expires_at) > new Date();

      return {
        connected: isValid,
        last_sync: data.updated_at,
      } as RevolutStatus;
    },
  });
}

export function useConnectRevolut() {
  return useMutation({
    mutationFn: async () => {
      const redirectUri = `${window.location.origin}/administration`;
      const response = await fetch(
        `https://bujfwfqmfsgaibnmuvml.supabase.co/functions/v1/revolut-auth?app_redirect_uri=${encodeURIComponent(redirectUri)}`,
        { method: "GET" }
      );

      if (!response.ok) {
        throw new Error("Failed to initiate Revolut connection");
      }

      const data = await response.json();
      if (data.authorization_url) {
        window.location.href = data.authorization_url;
      }
      return data;
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

export function useDisconnectRevolut() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => {
      const { error } = await orusSupabase.from("revolut_auth_tokens").delete().neq("id", "");

      if (error) throw error;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "revolut"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "revolut"] });
      toast.success("Déconnexion Revolut réussie");
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

export function useSyncRevolut() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async () => {
      const response = await orusSupabase.functions.invoke("revolut-sync");

      if (response.error) throw response.error;
      return response.data;
    },
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["orus", "admin", "revolut"] });
      queryClient.invalidateQueries({ queryKey: ["orus", "revolut"] });
      toast.success("Synchronisation Revolut réussie", {
        description: `${data?.accounts_synced || 0} comptes, ${data?.transactions_synced || 0} transactions`,
      });
    },
    onError: (error: Error) => {
      toast.error(`Erreur: ${error.message}`);
    },
  });
}

// =====================================================
// PERMISSIONS HOOK (Local usage)
// =====================================================

export function usePermissions() {
  const [permissions, setPermissions] = useState<Permission[]>([]);
  const [loading, setLoading] = useState(true);
  const [userRole, setUserRole] = useState<UserRole | null>(null);

  useEffect(() => {
    const fetchPermissions = async () => {
      try {
        const {
          data: { user },
        } = await orusSupabase.auth.getUser();

        if (!user) {
          setPermissions([]);
          setLoading(false);
          return;
        }

        // Get user role
        const { data: roleData } = await orusSupabase
          .from("user_roles")
          .select("role")
          .eq("user_id", user.id)
          .maybeSingle();

        const role = (roleData?.role as UserRole) || "viewer";
        setUserRole(role);

        // Get permissions for role
        const { data: perms } = await orusSupabase
          .from("role_permissions")
          .select("resource, action, allowed")
          .eq("role", role);

        setPermissions(perms || []);
      } catch (error) {
        console.error("Error fetching permissions:", error);
        setPermissions([]);
      } finally {
        setLoading(false);
      }
    };

    fetchPermissions();
  }, []);

  const hasPermission = useCallback(
    (resource: string, action: string): boolean => {
      if (userRole === "superadmin") return true;
      const permission = permissions.find((p) => p.resource === resource && p.action === action);
      return permission?.allowed ?? false;
    },
    [permissions, userRole]
  );

  const canView = useCallback((resource: string) => hasPermission(resource, "view"), [hasPermission]);
  const canCreate = useCallback((resource: string) => hasPermission(resource, "create"), [hasPermission]);
  const canEdit = useCallback((resource: string) => hasPermission(resource, "edit"), [hasPermission]);
  const canDelete = useCallback((resource: string) => hasPermission(resource, "delete"), [hasPermission]);
  const canManageUsers = useCallback(() => hasPermission("admin", "manage-users"), [hasPermission]);
  const canManagePermissions = useCallback(() => hasPermission("admin", "manage-permissions"), [hasPermission]);

  // Module visibility based on permissions
  const canAccessModule = useCallback(
    (moduleKey: string): boolean => {
      // Superadmin can see everything
      if (userRole === "superadmin") return true;
      // Map module key to resource
      return hasPermission(moduleKey, "view");
    },
    [hasPermission, userRole]
  );

  return {
    permissions,
    loading,
    userRole,
    hasPermission,
    canView,
    canCreate,
    canEdit,
    canDelete,
    canManageUsers,
    canManagePermissions,
    canAccessModule,
  };
}

// =====================================================
// MODULE VISIBILITY HOOK (for navigation filtering)
// =====================================================

interface ModuleVisibility {
  privateEquity: boolean;
  accounting: boolean;
  shareholder: boolean;
  reports: boolean;
  admin: boolean;
}

export function useModuleVisibility(): { modules: ModuleVisibility; loading: boolean; userRole: UserRole | null } {
  const { canAccessModule, loading, userRole } = usePermissions();

  const modules: ModuleVisibility = {
    privateEquity: canAccessModule("private-equity"),
    accounting: canAccessModule("accounting"),
    shareholder: canAccessModule("shareholder"),
    reports: canAccessModule("reports"),
    admin: canAccessModule("admin"),
  };

  return { modules, loading, userRole };
}

// =====================================================
// ROLE LABELS
// =====================================================

export const ROLE_OPTIONS: { value: UserRole; label: string }[] = [
  { value: "superadmin", label: "Super Admin" },
  { value: "admin", label: "Admin" },
  { value: "portfolio_manager", label: "Portfolio Manager" },
  { value: "trader", label: "Trader" },
  { value: "analyst", label: "Analyste" },
  { value: "viewer", label: "Lecteur" },
];

export function getRoleLabel(role: string): string {
  const option = ROLE_OPTIONS.find((o) => o.value === role);
  return option?.label || role;
}

export function getRoleBadgeVariant(
  role: string
): "default" | "destructive" | "secondary" | "outline" {
  switch (role) {
    case "superadmin":
      return "destructive";
    case "admin":
      return "default";
    case "portfolio_manager":
      return "secondary";
    default:
      return "outline";
  }
}

// =====================================================
// RESOURCES FOR PERMISSIONS
// =====================================================

export const PERMISSION_RESOURCES = [
  { resource: "admin", label: "Administration", actions: ["view", "manage-users", "manage-permissions"] },
  { resource: "ai_assistant", label: "Assistant IA", actions: ["view", "create"] },
  { resource: "treasury", label: "Trésorerie", actions: ["view", "edit", "sync"] },
  { resource: "dashboard", label: "Tableau de bord", actions: ["view"] },
  { resource: "positions", label: "Positions", actions: ["view", "create", "edit", "delete"] },
  { resource: "stocks", label: "Stock Management", actions: ["view", "create", "edit", "delete"] },
  { resource: "cash", label: "Cash", actions: ["view", "create", "edit"] },
  { resource: "debt", label: "Debt Management", actions: ["view", "create", "edit", "delete"] },
  { resource: "private_equity", label: "Private Equity", actions: ["view", "create", "edit", "delete"] },
  { resource: "risk_journal", label: "Risk & Journal", actions: ["view", "create", "edit"] },
  { resource: "reports", label: "Rapports", actions: ["view", "generate", "share"] },
  { resource: "settings", label: "Paramètres", actions: ["view", "edit"] },
];
