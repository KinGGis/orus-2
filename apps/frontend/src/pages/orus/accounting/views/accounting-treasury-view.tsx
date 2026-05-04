/**
 * AccountingTreasuryView - Treasury/Cash management view
 * Shows currency balances from Revolut accounts, cash flow charts, and treasury analysis
 */
import { useMemo, useEffect } from "react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui/components/ui/card";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { AmountDisplay } from "@wealthfolio/ui";
import {
  PieChart,
  Pie,
  Cell,
  ResponsiveContainer,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  AreaChart,
  Area,
} from "recharts";
import { Wallet, TrendingUp, TrendingDown, DollarSign, Euro, Coins, RefreshCw, CheckCircle, XCircle, ExternalLink, Loader2 } from "lucide-react";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import { toast } from "sonner";

import { 
  useOrusRevolutAccounts, 
  useOrusRevolutTransactions,
  useRevolutConnection,
  useRevolutSync,
} from "@/features/orus-integration";

// =====================================================
// TYPES
// =====================================================

interface CurrencyBalance {
  currency: string;
  balance: number;
  percentageOfTotal: number;
  icon: React.ReactNode;
  color: string;
}

interface CashFlowData {
  month: string;
  inflows: number;
  outflows: number;
  net: number;
}

interface TrendData {
  month: string;
  balance: number;
}

// =====================================================
// CONSTANTS
// =====================================================

const CURRENCY_CONFIG: Record<string, { icon: React.ReactNode; color: string; label: string }> = {
  EUR: { icon: <Euro className="h-5 w-5" />, color: "#3b82f6", label: "Euro" },
  USD: { icon: <DollarSign className="h-5 w-5" />, color: "#10b981", label: "US Dollar" },
  XOF: { icon: <Coins className="h-5 w-5" />, color: "#f59e0b", label: "CFA Franc" },
  GBP: { icon: <Coins className="h-5 w-5" />, color: "#8b5cf6", label: "British Pound" },
};

const CHART_COLORS = ["#3b82f6", "#10b981", "#f59e0b", "#ef4444", "#8b5cf6", "#ec4899"];

// =====================================================
// HELPER FUNCTIONS
// =====================================================

const formatCurrency = (value: number, currency = "EUR"): string => {
  return new Intl.NumberFormat("fr-FR", {
    style: "currency",
    currency,
    maximumFractionDigits: 0,
  }).format(value);
};

const formatCompactCurrency = (value: number): string => {
  if (Math.abs(value) >= 1000000) {
    return `${(value / 1000000).toFixed(1)}M`;
  }
  if (Math.abs(value) >= 1000) {
    return `${(value / 1000).toFixed(0)}k`;
  }
  return value.toFixed(0);
};

// =====================================================
// COMPONENTS
// =====================================================

function CurrencyBalanceCard({ balance }: { balance: CurrencyBalance }) {
  const config = CURRENCY_CONFIG[balance.currency] || CURRENCY_CONFIG.EUR;
  
  return (
    <Card className="bg-card/50 border-border/50 hover:shadow-md transition-shadow">
      <CardContent className="pt-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div 
              className="p-2.5 rounded-lg"
              style={{ backgroundColor: `${config.color}20`, color: config.color }}
            >
              {config.icon}
            </div>
            <div>
              <p className="text-sm font-medium text-muted-foreground">{config.label}</p>
              <p className="text-2xl font-bold">
                <AmountDisplay value={balance.balance} currency={balance.currency} />
              </p>
            </div>
          </div>
          <Badge variant="outline" className="text-xs">
            {balance.percentageOfTotal.toFixed(1)}%
          </Badge>
        </div>
      </CardContent>
    </Card>
  );
}

function CurrencyDistributionChart({ balances }: { balances: CurrencyBalance[] }) {
  const data = balances.map((b) => ({
    name: b.currency,
    value: b.balance,
    color: CURRENCY_CONFIG[b.currency]?.color || CHART_COLORS[0],
  }));

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <CardTitle className="text-base">Répartition par devise</CardTitle>
        <CardDescription>Distribution de la trésorerie par devise</CardDescription>
      </CardHeader>
      <CardContent className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <PieChart>
            <Pie
              data={data}
              cx="50%"
              cy="50%"
              innerRadius={50}
              outerRadius={80}
              paddingAngle={2}
              dataKey="value"
              label={({ name, percent }) => `${name} ${((percent || 0) * 100).toFixed(0)}%`}
            >
              {data.map((entry, index) => (
                <Cell key={`cell-${index}`} fill={entry.color} />
              ))}
            </Pie>
            <Tooltip 
              formatter={(value) => formatCompactCurrency(value as number)}
              contentStyle={{ 
                backgroundColor: 'hsl(var(--card))', 
                border: '1px solid hsl(var(--border))' 
              }}
            />
          </PieChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}

function CashFlowChart({ data }: { data: CashFlowData[] }) {
  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <CardTitle className="text-base">Flux de trésorerie</CardTitle>
        <CardDescription>Entrées et sorties de cash par mois</CardDescription>
      </CardHeader>
      <CardContent className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={data}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
            <XAxis 
              dataKey="month" 
              tick={{ fontSize: 10 }} 
              stroke="hsl(var(--muted-foreground))" 
            />
            <YAxis 
              tick={{ fontSize: 10 }} 
              stroke="hsl(var(--muted-foreground))"
              tickFormatter={formatCompactCurrency}
            />
            <Tooltip 
              formatter={(value) => formatCompactCurrency(value as number)}
              contentStyle={{ 
                backgroundColor: 'hsl(var(--card))', 
                border: '1px solid hsl(var(--border))' 
              }}
            />
            <Legend />
            <Bar dataKey="inflows" fill="#10b981" name="Entrées" radius={[4, 4, 0, 0]} />
            <Bar dataKey="outflows" fill="#ef4444" name="Sorties" radius={[4, 4, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}

function TreasuryTrendChart({ data }: { data: TrendData[] }) {
  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <CardTitle className="text-base">Évolution de la trésorerie</CardTitle>
        <CardDescription>Solde total sur les 12 derniers mois</CardDescription>
      </CardHeader>
      <CardContent className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={data}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
            <XAxis 
              dataKey="month" 
              tick={{ fontSize: 10 }} 
              stroke="hsl(var(--muted-foreground))" 
            />
            <YAxis 
              tick={{ fontSize: 10 }} 
              stroke="hsl(var(--muted-foreground))"
              tickFormatter={formatCompactCurrency}
            />
            <Tooltip 
              formatter={(value) => formatCompactCurrency(value as number)}
              contentStyle={{ 
                backgroundColor: 'hsl(var(--card))', 
                border: '1px solid hsl(var(--border))' 
              }}
            />
            <Area
              type="monotone"
              dataKey="balance"
              fill="#3b82f6"
              fillOpacity={0.3}
              stroke="#3b82f6"
              strokeWidth={2}
              name="Solde"
            />
          </AreaChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}

function TreasurySkeleton() {
  return (
    <div className="space-y-6">
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Skeleton className="h-28" />
        <Skeleton className="h-28" />
        <Skeleton className="h-28" />
      </div>
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <Skeleton className="h-72" />
        <Skeleton className="h-72" />
      </div>
      <Skeleton className="h-72" />
    </div>
  );
}

// =====================================================
// REVOLUT SYNC PANEL
// =====================================================

function RevolutSyncPanel() {
  const { data: connection, isLoading: connectionLoading, refetch: refetchConnection } = useRevolutConnection();
  const { initiateConnection, syncData } = useRevolutSync();

  // Check for successful connection in URL params
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    if (params.get("revolut_connected") === "true") {
      const expiresAt = params.get("expires_at");
      toast.success("Connexion Revolut réussie", {
        description: expiresAt 
          ? `Token valide jusqu'au ${format(new Date(expiresAt), "PPP", { locale: fr })}`
          : "Connexion établie avec succès",
      });
      // Clean URL
      window.history.replaceState({}, "", window.location.pathname);
      // Refresh connection status
      setTimeout(() => refetchConnection(), 500);
    }
  }, [refetchConnection]);

  const handleConnect = () => {
    toast.info("Connexion à Revolut...", {
      description: "Redirection vers Revolut Business pour autorisation",
    });
    initiateConnection.mutate(undefined, {
      onError: (error) => {
        toast.error("Connexion Revolut échouée", {
          description: error.message || "Impossible de démarrer la connexion Revolut",
        });
      },
    });
  };

  const handleSync = () => {
    toast.info("Synchronisation en cours...");
    syncData.mutate(undefined, {
      onSuccess: (data) => {
        toast.success("Synchronisation réussie", {
          description: `${data.accounts_synced} comptes et ${data.transactions_synced} transactions synchronisés`,
        });
      },
      onError: (error) => {
        toast.error("Erreur de synchronisation", {
          description: error.message || "Impossible de synchroniser les données Revolut",
        });
      },
    });
  };

  const isConnected = connection?.isConnected ?? false;
  const tokenExpiry = connection?.tokenExpiry;

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <img 
              src="https://www.revolut.com/favicon.ico" 
              alt="Revolut" 
              className="h-5 w-5"
            />
            <div>
              <CardTitle className="text-base">Synchronisation Revolut</CardTitle>
              <CardDescription>
                {connectionLoading ? "Vérification de la connexion..." : 
                  isConnected && tokenExpiry 
                    ? `Token expire le ${format(new Date(tokenExpiry), "PPP à HH:mm", { locale: fr })}`
                    : "Connectez votre compte Revolut Business"}
              </CardDescription>
            </div>
          </div>
          <div className="flex items-center gap-2">
            {connectionLoading ? (
              <Badge variant="outline" className="flex items-center gap-1">
                <Loader2 className="h-3 w-3 animate-spin" />
                Chargement...
              </Badge>
            ) : isConnected ? (
              <Badge variant="default" className="flex items-center gap-1 bg-emerald-500">
                <CheckCircle className="h-3 w-3" />
                Connecté
              </Badge>
            ) : (
              <Badge variant="destructive" className="flex items-center gap-1">
                <XCircle className="h-3 w-3spline" />
                Non connecté
              </Badge>
            )}
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div className="flex items-center gap-2">
          {!isConnected ? (
            <Button 
              onClick={handleConnect} 
              disabled={initiateConnection.isPending}
              variant="default"
            >
              {initiateConnection.isPending ? (
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              ) : (
                <ExternalLink className="h-4 w-4 mr-2" />
              )}
              Connecter Revolut
            </Button>
          ) : (
            <>
              <Button
                onClick={handleSync}
                disabled={syncData.isPending}
                variant="default"
              >
                {syncData.isPending ? (
                  <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                ) : (
                  <RefreshCw className="h-4 w-4 mr-2" />
                )}
                Synchroniser
              </Button>
              <Button 
                onClick={handleConnect} 
                disabled={initiateConnection.isPending}
                variant="outline"
              >
                Reconnecter
              </Button>
            </>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

// =====================================================
// MAIN COMPONENT
// =====================================================

export function AccountingTreasuryView() {
  const { data: revolutAccounts, isLoading: accountsLoading, error: accountsError } = useOrusRevolutAccounts();
  const { data: transactions, isLoading: txLoading } = useOrusRevolutTransactions();

  const isLoading = accountsLoading || txLoading;
  const error = accountsError;

  // Calculate currency balances from Revolut accounts
  const currencyBalances = useMemo<CurrencyBalance[]>(() => {
    if (!revolutAccounts?.length) {
      // Fallback demo data if no accounts connected
      return [
        { currency: "EUR", balance: 37761.34, percentageOfTotal: 70.2, icon: <Euro />, color: "#3b82f6" },
        { currency: "GBP", balance: 12585.03, percentageOfTotal: 23.4, icon: <Coins />, color: "#8b5cf6" },
        { currency: "USD", balance: 2931.56, percentageOfTotal: 5.4, icon: <DollarSign />, color: "#10b981" },
        { currency: "XOF", balance: 500.00, percentageOfTotal: 1.0, icon: <Coins />, color: "#f59e0b" },
      ];
    }

    // Group by currency and sum balances
    const balancesByCurrency: Record<string, number> = {};
    revolutAccounts.forEach((account) => {
      const currency = account.currency || "EUR";
      balancesByCurrency[currency] = (balancesByCurrency[currency] || 0) + (account.balance || 0);
    });

    const total = Object.values(balancesByCurrency).reduce((sum, val) => sum + Math.abs(val), 0);

    return Object.entries(balancesByCurrency)
      .map(([currency, balance]) => ({
        currency,
        balance: Math.abs(balance),
        percentageOfTotal: total > 0 ? (Math.abs(balance) / total) * 100 : 0,
        icon: CURRENCY_CONFIG[currency]?.icon || <Coins />,
        color: CURRENCY_CONFIG[currency]?.color || "#6b7280",
      }))
      .sort((a, b) => b.balance - a.balance);
  }, [revolutAccounts]);

  // Calculate cash flow data from transactions
  const cashFlowData = useMemo<CashFlowData[]>(() => {
    if (!transactions?.length) {
      // Demo data
      return [
        { month: "Oct", inflows: 45000, outflows: 32000, net: 13000 },
        { month: "Nov", inflows: 52000, outflows: 48000, net: 4000 },
        { month: "Déc", inflows: 38000, outflows: 35000, net: 3000 },
        { month: "Jan", inflows: 61000, outflows: 42000, net: 19000 },
        { month: "Fév", inflows: 48000, outflows: 51000, net: -3000 },
        { month: "Mar", inflows: 55000, outflows: 45000, net: 10000 },
      ];
    }

    // Group transactions by month
    const monthlyData: Record<string, { inflows: number; outflows: number }> = {};
    transactions
      .filter(tx => tx.completed_at && tx.state?.toLowerCase() === "completed")
      .forEach((tx) => {
        const date = new Date(tx.completed_at!);
        const monthKey = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
        
        if (!monthlyData[monthKey]) {
          monthlyData[monthKey] = { inflows: 0, outflows: 0 };
        }
        
        const amount = tx.amount || 0;
        if (amount > 0) {
          monthlyData[monthKey].inflows += amount;
        } else {
          monthlyData[monthKey].outflows += Math.abs(amount);
        }
      });

    const monthNames = ["Jan", "Fév", "Mar", "Avr", "Mai", "Juin", "Juil", "Août", "Sep", "Oct", "Nov", "Déc"];

    return Object.entries(monthlyData)
      .sort((a, b) => a[0].localeCompare(b[0]))
      .slice(-6)
      .map(([key, data]) => {
        const [, month] = key.split("-");
        return {
          month: monthNames[parseInt(month) - 1],
          inflows: data.inflows,
          outflows: data.outflows,
          net: data.inflows - data.outflows,
        };
      });
  }, [transactions]);

  // Calculate trend data from transactions
  const trendData = useMemo<TrendData[]>(() => {
    if (!transactions?.length) {
      // Demo data
      return [
        { month: "Oct", balance: 40000 },
        { month: "Nov", balance: 44000 },
        { month: "Déc", balance: 47000 },
        { month: "Jan", balance: 51000 },
        { month: "Fév", balance: 49000 },
        { month: "Mar", balance: 53000 },
      ];
    }

    // Calculate cumulative balance by month
    const sortedTx = [...transactions]
      .filter(tx => tx.completed_at && tx.state?.toLowerCase() === "completed" && tx.balance_after !== null)
      .sort((a, b) => new Date(a.completed_at!).getTime() - new Date(b.completed_at!).getTime());

    // Get last transaction balance per month
    const monthlyBalances: Record<string, number> = {};
    sortedTx.forEach((tx) => {
      const date = new Date(tx.completed_at!);
      const monthKey = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
      monthlyBalances[monthKey] = tx.balance_after || 0;
    });

    const monthNames = ["Jan", "Fév", "Mar", "Avr", "Mai", "Juin", "Juil", "Août", "Sep", "Oct", "Nov", "Déc"];

    return Object.entries(monthlyBalances)
      .sort((a, b) => a[0].localeCompare(b[0]))
      .slice(-12)
      .map(([key, balance]) => {
        const [, month] = key.split("-");
        return {
          month: monthNames[parseInt(month) - 1],
          balance: Math.abs(balance),
        };
      });
  }, [transactions]);

  // Calculate summary stats
  const stats = useMemo(() => {
    const totalBalance = currencyBalances.reduce((sum, b) => sum + b.balance, 0);
    const lastMonthCashFlow = cashFlowData[cashFlowData.length - 1];
    const avgMonthlyInflows = cashFlowData.reduce((sum, d) => sum + d.inflows, 0) / (cashFlowData.length || 1);
    const avgMonthlyOutflows = cashFlowData.reduce((sum, d) => sum + d.outflows, 0) / (cashFlowData.length || 1);

    return {
      totalBalance,
      lastMonthNet: lastMonthCashFlow?.net || 0,
      avgMonthlyInflows,
      avgMonthlyOutflows,
      runway: avgMonthlyOutflows > 0 ? Math.round(totalBalance / avgMonthlyOutflows) : 999,
    };
  }, [currencyBalances, cashFlowData]);

  // Get last sync time
  const lastSynced = useMemo(() => {
    if (!revolutAccounts?.length) return null;
    const dates = revolutAccounts
      .filter(a => a.last_synced_at)
      .map(a => new Date(a.last_synced_at!));
    if (!dates.length) return null;
    return format(Math.max(...dates.map(d => d.getTime())), "dd MMM yyyy HH:mm", { locale: fr });
  }, [revolutAccounts]);

  if (isLoading) {
    return <TreasurySkeleton />;
  }

  if (error) {
    return (
      <Card className="bg-card/50 border-border/50">
        <CardContent className="py-16 text-center">
          <Wallet className="h-12 w-12 mx-auto text-muted-foreground mb-4" />
          <p className="text-muted-foreground">Erreur lors du chargement des données</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      {/* Summary header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-bold">Trésorerie</h2>
          <p className="text-muted-foreground text-sm">
            Solde total: <AmountDisplay value={stats.totalBalance} currency="EUR" />
          </p>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            {stats.lastMonthNet >= 0 ? (
              <TrendingUp className="h-5 w-5 text-emerald-500" />
            ) : (
              <TrendingDown className="h-5 w-5 text-red-500" />
            )}
            <span className={`font-medium ${stats.lastMonthNet >= 0 ? "text-emerald-500" : "text-red-500"}`}>
              {formatCurrency(stats.lastMonthNet)} ce mois
            </span>
          </div>
          <Badge variant="outline">Runway: {stats.runway} mois</Badge>
          {lastSynced && (
            <div className="flex items-center gap-1 text-xs text-muted-foreground">
              <RefreshCw className="h-3 w-3" />
              {lastSynced}
            </div>
          )}
        </div>
      </div>

      {/* Currency balance cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {currencyBalances.slice(0, 3).map((balance) => (
          <CurrencyBalanceCard key={balance.currency} balance={balance} />
        ))}
      </div>

      {/* Revolut Sync Panel */}
      <RevolutSyncPanel />

      {/* Charts row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <CurrencyDistributionChart balances={currencyBalances} />
        <CashFlowChart data={cashFlowData} />
      </div>

      {/* Trend chart */}
      <TreasuryTrendChart data={trendData} />
    </div>
  );
}
