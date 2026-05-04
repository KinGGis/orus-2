/**
 * Orus Authentication Context
 * Provides authentication state and methods for Orus integration
 * Supports: Email/Password, Google OAuth, Password Reset
 */
import {
  createContext,
  useContext,
  useEffect,
  useState,
  useCallback,
  type ReactNode,
} from "react";
import { User, Session, AuthError } from "@supabase/supabase-js";
import { orusSupabase } from "./supabase-client";
import { useQueryClient } from "@tanstack/react-query";

// =====================================================
// TYPES
// =====================================================

export type OrusUserRole = "superadmin" | "admin" | "analyst" | "trader" | "portfolio_manager" | "viewer";

export interface OrusUserProfile {
  id: string;
  email: string;
  full_name: string | null;
  avatar_url: string | null;
  phone: string | null;
  company: string | null;
  job_title: string | null;
  preferred_currency: string;
  preferred_language: string;
  onboarding_completed: boolean;
  onboarding_completed_at: string | null;
  created_at: string;
  updated_at: string;
}

interface OrusAuthContextValue {
  // State
  user: User | null;
  session: Session | null;
  profile: OrusUserProfile | null;
  userRole: OrusUserRole | null;
  loading: boolean;
  isAuthenticated: boolean;
  needsOnboarding: boolean;
  
  // Email/Password Auth
  signIn: (email: string, password: string) => Promise<{ error: AuthError | null }>;
  signUp: (email: string, password: string, fullName: string) => Promise<{ error: AuthError | null }>;
  signOut: () => Promise<{ error: AuthError | null }>;
  resetPassword: (email: string) => Promise<{ error: AuthError | null }>;
  updatePassword: (newPassword: string) => Promise<{ error: AuthError | null }>;
  
  // OAuth
  signInWithGoogle: () => Promise<{ error: AuthError | null }>;
  
  // Profile
  updateProfile: (data: Partial<OrusUserProfile>) => Promise<{ error: Error | null }>;
  completeOnboarding: () => Promise<{ error: Error | null }>;
  refreshProfile: () => Promise<void>;
  
  // Utils
  refreshSession: () => Promise<void>;
}

const OrusAuthContext = createContext<OrusAuthContextValue | undefined>(undefined);

// =====================================================
// PROVIDER
// =====================================================

export function OrusAuthProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();
  const [user, setUser] = useState<User | null>(null);
  const [session, setSession] = useState<Session | null>(null);
  const [profile, setProfile] = useState<OrusUserProfile | null>(null);
  const [userRole, setUserRole] = useState<OrusUserRole | null>(null);
  const [loading, setLoading] = useState(true);

  // Fetch user profile from profiles table
  const fetchProfile = useCallback(async (userId: string) => {
    try {
      const { data, error } = await orusSupabase
        .from("profiles")
        .select("*")
        .eq("id", userId)
        .maybeSingle();

      if (error) {
        console.warn("[OrusAuth] Error fetching profile:", error.message);
        setProfile(null);
        return null;
      }

      // Cast data to any to handle dynamic Supabase schema
      const profileRow = data as Record<string, unknown> | null;
      
      const profileData: OrusUserProfile = {
        id: (profileRow?.id as string) || userId,
        email: (profileRow?.email as string) || "",
        full_name: (profileRow?.full_name as string | null) || null,
        avatar_url: (profileRow?.avatar_url as string | null) || null,
        phone: (profileRow?.phone as string | null) || null,
        company: (profileRow?.company as string | null) || null,
        job_title: (profileRow?.job_title as string | null) || null,
        preferred_currency: (profileRow?.preferred_currency as string) || "EUR",
        preferred_language: (profileRow?.preferred_language as string) || "fr",
        onboarding_completed: (profileRow?.onboarding_completed as boolean) || false,
        onboarding_completed_at: (profileRow?.onboarding_completed_at as string | null) || null,
        created_at: (profileRow?.created_at as string) || new Date().toISOString(),
        updated_at: (profileRow?.updated_at as string) || new Date().toISOString(),
      };

      setProfile(profileData);
      return profileData;
    } catch (error) {
      console.error("[OrusAuth] Error in fetchProfile:", error);
      setProfile(null);
      return null;
    }
  }, []);

  // Fetch user role from user_roles table
  const fetchUserRole = useCallback(async (userId: string) => {
    try {
      const { data, error } = await orusSupabase
        .from("user_roles")
        .select("role")
        .eq("user_id", userId)
        .maybeSingle();

      if (error) {
        console.warn("[OrusAuth] Error fetching user role:", error.message);
        setUserRole(null);
        return;
      }

      // Cast data to handle dynamic Supabase schema
      const roleRow = data as Record<string, unknown> | null;
      setUserRole((roleRow?.role as OrusUserRole) || null);
    } catch (error) {
      console.error("[OrusAuth] Error in fetchUserRole:", error);
      setUserRole(null);
    }
  }, []);

  // Initialize auth state
  useEffect(() => {
    let mounted = true;

    // Set up auth state listener
    const { data: { subscription } } = orusSupabase.auth.onAuthStateChange(
      async (event, currentSession) => {
        if (!mounted) return;

        console.log("[OrusAuth] Auth state changed:", event);
        setSession(currentSession);
        setUser(currentSession?.user ?? null);

        if (currentSession?.user) {
          // Use setTimeout to avoid potential deadlocks with Supabase
          setTimeout(() => {
            if (mounted) {
              fetchUserRole(currentSession.user.id);
              fetchProfile(currentSession.user.id);
            }
          }, 0);
        } else {
          setUserRole(null);
          setProfile(null);
        }

        // Invalidate queries on auth change
        if (event === "SIGNED_IN" || event === "SIGNED_OUT" || event === "TOKEN_REFRESHED") {
          queryClient.invalidateQueries({ queryKey: ["orus"] });
        }
      }
    );

    // Check for existing session
    const initializeSession = async () => {
      try {
        const { data: { session: existingSession } } = await orusSupabase.auth.getSession();
        
        if (!mounted) return;

        setSession(existingSession);
        setUser(existingSession?.user ?? null);

        if (existingSession?.user) {
          await Promise.all([
            fetchUserRole(existingSession.user.id),
            fetchProfile(existingSession.user.id),
          ]);
        }
      } catch (error) {
        console.error("[OrusAuth] Error getting session:", error);
      } finally {
        if (mounted) {
          setLoading(false);
        }
      }
    };

    initializeSession();

    return () => {
      mounted = false;
      subscription.unsubscribe();
    };
  }, [fetchUserRole, fetchProfile, queryClient]);

  // =====================================================
  // AUTH METHODS
  // =====================================================

  const signIn = useCallback(async (email: string, password: string) => {
    const { error } = await orusSupabase.auth.signInWithPassword({
      email,
      password,
    });
    return { error };
  }, []);

  const signUp = useCallback(async (email: string, password: string, fullName: string) => {
    const redirectUrl = `${window.location.origin}/auth/orus/callback`;

    const { error } = await orusSupabase.auth.signUp({
      email,
      password,
      options: {
        emailRedirectTo: redirectUrl,
        data: {
          full_name: fullName,
        },
      },
    });
    return { error };
  }, []);

  const signOut = useCallback(async () => {
    try {
      const { error } = await orusSupabase.auth.signOut();

      if (error) {
        console.warn("[OrusAuth] Server signOut failed, forcing local:", error.message);
        await orusSupabase.auth.signOut({ scope: "local" });
      }

      // Clear state
      setUser(null);
      setSession(null);
      setUserRole(null);
      setProfile(null);

      // Clear queries
      queryClient.invalidateQueries({ queryKey: ["orus"] });

      return { error: null };
    } catch (e) {
      console.error("[OrusAuth] SignOut error:", e);
      setUser(null);
      setSession(null);
      setUserRole(null);
      setProfile(null);
      return { error: null };
    }
  }, [queryClient]);

  const resetPassword = useCallback(async (email: string) => {
    const redirectUrl = `${window.location.origin}/reset-password`;

    const { error } = await orusSupabase.auth.resetPasswordForEmail(email, {
      redirectTo: redirectUrl,
    });
    return { error };
  }, []);

  const updatePassword = useCallback(async (newPassword: string) => {
    const { error } = await orusSupabase.auth.updateUser({
      password: newPassword,
    });
    return { error };
  }, []);

  const signInWithGoogle = useCallback(async () => {
    const redirectUrl = `${window.location.origin}/auth/orus/callback`;

    const { error } = await orusSupabase.auth.signInWithOAuth({
      provider: "google",
      options: {
        redirectTo: redirectUrl,
        queryParams: {
          access_type: "offline",
          prompt: "consent",
        },
      },
    });
    return { error };
  }, []);

  // Update user profile
  const updateProfile = useCallback(async (data: Partial<OrusUserProfile>) => {
    if (!user) {
      console.error("[OrusAuth] updateProfile: User not authenticated");
      return { error: new Error("User not authenticated") };
    }

    try {
      console.log("[OrusAuth] updateProfile: Starting update for user", user.id);
      console.log("[OrusAuth] updateProfile: Data to update", data);
      
      // Use upsert to ensure profile exists (for existing users)
      const upsertData = {
        id: user.id,
        email: user.email || "",
        ...data,
        updated_at: new Date().toISOString(),
      };
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const { error } = await (orusSupabase as any)
        .from("profiles")
        .upsert(upsertData, { onConflict: "id" });

      if (error) {
        console.error("[OrusAuth] updateProfile: Error", error);
        return { error: new Error(error.message) };
      }

      console.log("[OrusAuth] updateProfile: Success");
      // Refresh profile
      await fetchProfile(user.id);
      return { error: null };
    } catch (err) {
      console.error("[OrusAuth] updateProfile: Exception", err);
      return { error: err instanceof Error ? err : new Error("Unknown error") };
    }
  }, [user, fetchProfile]);

  // Complete onboarding
  const completeOnboarding = useCallback(async () => {
    if (!user) {
      console.error("[OrusAuth] completeOnboarding: User not authenticated");
      return { error: new Error("User not authenticated") };
    }

    try {
      console.log("[OrusAuth] completeOnboarding: Starting for user", user.id);
      
      // Use upsert to ensure profile exists (for existing users)
      const upsertData = {
        id: user.id,
        email: user.email || "",
        onboarding_completed: true,
        onboarding_completed_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
      console.log("[OrusAuth] completeOnboarding: Upsert data", upsertData);
      
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const { error, data } = await (orusSupabase as any)
        .from("profiles")
        .upsert(upsertData, { onConflict: "id" })
        .select();

      console.log("[OrusAuth] completeOnboarding: Response", { error, data });

      if (error) {
        console.error("[OrusAuth] completeOnboarding: Error", error);
        return { error: new Error(error.message) };
      }

      console.log("[OrusAuth] completeOnboarding: Success, refreshing profile");
      // Refresh profile
      await fetchProfile(user.id);
      return { error: null };
    } catch (err) {
      console.error("[OrusAuth] completeOnboarding: Exception", err);
      return { error: err instanceof Error ? err : new Error("Unknown error") };
    }
  }, [user, fetchProfile]);

  // Refresh profile
  const refreshProfile = useCallback(async () => {
    if (user) {
      await fetchProfile(user.id);
    }
  }, [user, fetchProfile]);

  const refreshSession = useCallback(async () => {
    const { data: { session: newSession } } = await orusSupabase.auth.refreshSession();
    if (newSession) {
      setSession(newSession);
      setUser(newSession.user);
    }
  }, []);

  // =====================================================
  // CONTEXT VALUE
  // =====================================================

  const value: OrusAuthContextValue = {
    user,
    session,
    profile,
    userRole,
    loading,
    isAuthenticated: !!session,
    needsOnboarding: !!session && profile !== null && !profile.onboarding_completed,
    signIn,
    signUp,
    signOut,
    resetPassword,
    updatePassword,
    signInWithGoogle,
    updateProfile,
    completeOnboarding,
    refreshProfile,
    refreshSession,
  };

  return (
    <OrusAuthContext.Provider value={value}>
      {children}
    </OrusAuthContext.Provider>
  );
}

// =====================================================
// HOOK
// =====================================================

export function useOrusAuth() {
  const context = useContext(OrusAuthContext);
  if (!context) {
    throw new Error("useOrusAuth must be used within an OrusAuthProvider");
  }
  return context;
}

// =====================================================
// PROTECTED ROUTE COMPONENT
// =====================================================

interface OrusProtectedRouteProps {
  children: ReactNode;
  requiredRole?: OrusUserRole | OrusUserRole[];
  fallbackPath?: string;
}

export function OrusProtectedRoute({
  children,
  requiredRole,
  fallbackPath = "/login",
}: OrusProtectedRouteProps) {
  const { isAuthenticated, loading, userRole } = useOrusAuth();

  if (loading) {
    return (
      <div className="flex h-screen items-center justify-center">
        <div className="flex flex-col items-center gap-4">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
          <p className="text-muted-foreground text-sm">Vérification de l'authentification...</p>
        </div>
      </div>
    );
  }

  if (!isAuthenticated) {
    // Redirect to login
    window.location.href = fallbackPath;
    return null;
  }

  // Check role if required
  if (requiredRole) {
    const allowedRoles = Array.isArray(requiredRole) ? requiredRole : [requiredRole];
    if (userRole && !allowedRoles.includes(userRole)) {
      return (
        <div className="flex h-screen items-center justify-center">
          <div className="text-center">
            <h2 className="text-lg font-semibold">Accès refusé</h2>
            <p className="text-muted-foreground">
              Vous n'avez pas les permissions nécessaires pour accéder à cette page.
            </p>
          </div>
        </div>
      );
    }
  }

  return <>{children}</>;
}

// =====================================================
// AUTH GUARD FOR LAYOUT
// =====================================================

interface OrusAuthGuardProps {
  children: ReactNode;
}

/**
 * Auth guard that redirects to login if not authenticated
 */
export function OrusAuthGuard({ children }: OrusAuthGuardProps) {
  const { isAuthenticated, loading, profile } = useOrusAuth();

  if (loading) {
    return (
      <div className="flex h-screen items-center justify-center bg-background">
        <div className="flex flex-col items-center gap-4">
          <div className="flex h-16 w-16 items-center justify-center rounded-full bg-primary/10">
            <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
          </div>
          <div className="text-center">
            <h2 className="text-lg font-semibold">Orus</h2>
            <p className="text-muted-foreground text-sm">Chargement...</p>
          </div>
        </div>
      </div>
    );
  }

  if (!isAuthenticated) {
    window.location.href = "/login";
    return null;
  }

  // Profile is still loading
  if (profile === null) {
    return (
      <div className="flex h-screen items-center justify-center bg-background">
        <div className="flex flex-col items-center gap-4">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
          <p className="text-muted-foreground text-sm">Chargement du profil...</p>
        </div>
      </div>
    );
  }

  return <>{children}</>;
}
