import { isWeb } from "@/adapters";
import { AuthProvider } from "@/context/auth-context";
import { WealthfolioConnectProvider } from "@/features/wealthfolio-connect";
import { DeviceSyncProvider } from "@/features/devices-sync";
import { OrusAuthProvider } from "@/features/orus-integration/orus-auth-context";
import { SettingsProvider } from "@/lib/settings-provider";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { TooltipProvider } from "@wealthfolio/ui";
import { useState } from "react";
import { PrivacyProvider } from "./context/privacy-context";
import { AppRoutes } from "./routes";

function App() {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            refetchOnWindowFocus: false,
            staleTime: 5 * 60 * 1000,
            retry: false,
          },
        },
      }),
  );

  // Make QueryClient available globally for addons
  window.__wealthfolio_query_client__ = queryClient;

  const routedContent = <AppRoutes />;

  return (
    <QueryClientProvider client={queryClient}>
      <OrusAuthProvider>
        <AuthProvider>
          <WealthfolioConnectProvider>
            <DeviceSyncProvider>
              <PrivacyProvider>
                <SettingsProvider>
                  <TooltipProvider>{routedContent}</TooltipProvider>
                </SettingsProvider>
              </PrivacyProvider>
            </DeviceSyncProvider>
          </WealthfolioConnectProvider>
        </AuthProvider>
      </OrusAuthProvider>
    </QueryClientProvider>
  );
}

export default App;
