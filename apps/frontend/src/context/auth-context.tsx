import { isWeb } from "@/adapters";
import { useOrusAuth } from "@/features/orus-integration/orus-auth-context";
import { setUnauthorizedHandler } from "@/lib/auth-token";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  type ReactNode,
} from "react";

interface AuthContextValue {
  requiresAuth: boolean;
  isAuthenticated: boolean;
  statusLoading: boolean;
  loginLoading: boolean;
  loginError: string | null;
  login: (password: string) => Promise<void>;
  logout: () => void;
  clearError: () => void;
}

const AuthContext = createContext<AuthContextValue | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, loading, signOut } = useOrusAuth();

  useEffect(() => {
    const handler = () => {
      void signOut();
    };
    setUnauthorizedHandler(handler);
    return () => {
      setUnauthorizedHandler(null);
    };
  }, [signOut]);

  const login = useCallback(async (_password: string) => {
    throw new Error("Password login is disabled. Use the Orus login screen.");
  }, []);

  const logout = useCallback(() => {
    void signOut();
  }, [signOut]);

  const clearError = useCallback(() => {}, []);

  const value = useMemo<AuthContextValue>(
    () => ({
      requiresAuth: isWeb,
      isAuthenticated: !isWeb || isAuthenticated,
      statusLoading: isWeb ? loading : false,
      loginLoading: false,
      loginError: null,
      login,
      logout,
      clearError,
    }),
    [
      isAuthenticated,
      loading,
      login,
      logout,
      clearError,
    ],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export const useAuth = () => {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return ctx;
};

export function AuthGate({ children, fallback }: { children: ReactNode; fallback: ReactNode }) {
  const { requiresAuth, isAuthenticated, statusLoading } = useAuth();

  if (statusLoading) {
    return (
      <div className="bg-background text-muted-foreground flex min-h-screen items-center justify-center">
        Checking authentication...
      </div>
    );
  }

  if (requiresAuth && !isAuthenticated) {
    return fallback ? <>{fallback}</> : null;
  }

  return <>{children}</>;
}
