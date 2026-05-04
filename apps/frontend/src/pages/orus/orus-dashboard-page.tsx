/**
 * Orus Dashboard Page
 * Displays data from the existing Orus Supabase database
 */
import { Card, CardContent, CardDescription, CardHeader, CardTitle, Skeleton } from "@wealthfolio/ui";
import { formatCurrency, formatNumber, formatPercent } from "@/lib/format";
import {
  useOrusLatestPositions,
  useOrusPortfolioSummary,
  useOrusLatestCash,
  useOrusNavHistory,
  useOrusAuth,
} from "@/features/orus-integration";

export default function OrusDashboardPage() {
  const { isAuthenticated, isLoading: authLoading } = useOrusAuth();
  const { data: positions, isLoading: positionsLoading, error: positionsError } = useOrusLatestPositions();
  const { data: summary, isLoading: summaryLoading } = useOrusPortfolioSummary();
  const { data: cash, isLoading: cashLoading } = useOrusLatestCash();
  // NAV history for future chart implementation
  useOrusNavHistory(30);

  // Calculate totals
  const totalMarketValue = positions?.reduce((acc, p) => acc + (p.market_value || 0), 0) || 0;
  const totalCash = cash?.reduce((acc, c) => acc + c.amount, 0) || 0;
  const totalAUM = totalMarketValue + totalCash;
  const positionCount = positions?.length || 0;

  // Group positions by asset class
  const positionsByAssetClass = positions?.reduce(
    (acc, p) => {
      const assetClass = p.asset_class || "Other";
      if (!acc[assetClass]) {
        acc[assetClass] = { count: 0, value: 0 };
      }
      acc[assetClass].count++;
      acc[assetClass].value += p.market_value || 0;
      return acc;
    },
    {} as Record<string, { count: number; value: number }>,
  );

  if (authLoading) {
    return (
      <div className="p-6">
        <Skeleton className="h-8 w-48 mb-4" />
        <Skeleton className="h-64 w-full" />
      </div>
    );
  }

  if (!isAuthenticated) {
    return (
      <div className="p-6">
        <Card>
          <CardHeader>
            <CardTitle>Orus Data</CardTitle>
            <CardDescription>Connect to view your Orus portfolio data</CardDescription>
          </CardHeader>
          <CardContent>
            <p className="text-muted-foreground">
              Please sign in to Orus to access your existing portfolio data. Go to Settings → Orus Integration to
              configure your connection.
            </p>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      <div>
        <h1 className="text-2xl font-bold">Orus Portfolio</h1>
        <p className="text-muted-foreground">Data from your existing Orus database</p>
      </div>

      {/* Summary Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card>
          <CardHeader className="pb-2">
            <CardDescription>Total AUM</CardDescription>
          </CardHeader>
          <CardContent>
            {summaryLoading ? (
              <Skeleton className="h-8 w-24" />
            ) : (
              <div className="text-2xl font-bold">{formatCurrency(summary?.total_aum || totalAUM, "EUR")}</div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardDescription>Invested</CardDescription>
          </CardHeader>
          <CardContent>
            {positionsLoading ? (
              <Skeleton className="h-8 w-24" />
            ) : (
              <div className="text-2xl font-bold">{formatCurrency(totalMarketValue, "EUR")}</div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardDescription>Cash</CardDescription>
          </CardHeader>
          <CardContent>
            {cashLoading ? (
              <Skeleton className="h-8 w-24" />
            ) : (
              <div className="text-2xl font-bold">{formatCurrency(totalCash, "EUR")}</div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardDescription>Positions</CardDescription>
          </CardHeader>
          <CardContent>
            {positionsLoading ? (
              <Skeleton className="h-8 w-24" />
            ) : (
              <div className="text-2xl font-bold">{positionCount}</div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* P&L Summary */}
      {summary && (
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <Card>
            <CardHeader className="pb-2">
              <CardDescription>Daily P&L</CardDescription>
            </CardHeader>
            <CardContent>
              <div className={`text-xl font-bold ${(summary.daily_pnl || 0) >= 0 ? "text-green-600" : "text-red-600"}`}>
                {formatCurrency(summary.daily_pnl || 0, "EUR")}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="pb-2">
              <CardDescription>MTD P&L</CardDescription>
            </CardHeader>
            <CardContent>
              <div className={`text-xl font-bold ${(summary.mtd_pnl || 0) >= 0 ? "text-green-600" : "text-red-600"}`}>
                {formatCurrency(summary.mtd_pnl || 0, "EUR")}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="pb-2">
              <CardDescription>YTD P&L</CardDescription>
            </CardHeader>
            <CardContent>
              <div className={`text-xl font-bold ${(summary.ytd_pnl || 0) >= 0 ? "text-green-600" : "text-red-600"}`}>
                {formatCurrency(summary.ytd_pnl || 0, "EUR")}
              </div>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Asset Class Breakdown */}
      <Card>
        <CardHeader>
          <CardTitle>Asset Allocation</CardTitle>
          <CardDescription>Breakdown by asset class</CardDescription>
        </CardHeader>
        <CardContent>
          {positionsLoading ? (
            <div className="space-y-2">
              <Skeleton className="h-4 w-full" />
              <Skeleton className="h-4 w-3/4" />
              <Skeleton className="h-4 w-1/2" />
            </div>
          ) : positionsByAssetClass && Object.keys(positionsByAssetClass).length > 0 ? (
            <div className="space-y-3">
              {Object.entries(positionsByAssetClass)
                .sort((a, b) => b[1].value - a[1].value)
                .map(([assetClass, { count, value }]) => (
                  <div key={assetClass} className="flex items-center justify-between">
                    <div>
                      <span className="font-medium">{assetClass}</span>
                      <span className="text-muted-foreground ml-2">({count} positions)</span>
                    </div>
                    <div className="text-right">
                      <div className="font-medium">{formatCurrency(value, "EUR")}</div>
                      <div className="text-sm text-muted-foreground">
                        {formatPercent(totalMarketValue > 0 ? value / totalMarketValue : 0)}
                      </div>
                    </div>
                  </div>
                ))}
            </div>
          ) : (
            <p className="text-muted-foreground">No positions found</p>
          )}
        </CardContent>
      </Card>

      {/* Positions Table */}
      <Card>
        <CardHeader>
          <CardTitle>Positions</CardTitle>
          <CardDescription>Current open positions from Orus</CardDescription>
        </CardHeader>
        <CardContent>
          {positionsLoading ? (
            <div className="space-y-2">
              {[...Array(5)].map((_, i) => (
                <Skeleton key={i} className="h-12 w-full" />
              ))}
            </div>
          ) : positionsError ? (
            <p className="text-red-500">Error loading positions: {positionsError.message}</p>
          ) : positions && positions.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b text-left text-sm text-muted-foreground">
                    <th className="pb-2">Instrument</th>
                    <th className="pb-2 text-right">Quantity</th>
                    <th className="pb-2 text-right">Price</th>
                    <th className="pb-2 text-right">Market Value</th>
                    <th className="pb-2 text-right">Daily %</th>
                  </tr>
                </thead>
                <tbody>
                  {positions.slice(0, 20).map((position) => (
                    <tr key={position.id} className="border-b last:border-0">
                      <td className="py-3">
                        <div className="font-medium">{position.instrument || "Unknown"}</div>
                        <div className="text-sm text-muted-foreground">{position.asset_class}</div>
                      </td>
                      <td className="py-3 text-right">{formatNumber(position.quantity)}</td>
                      <td className="py-3 text-right">{formatCurrency(position.price || 0, position.currency || "EUR")}</td>
                      <td className="py-3 text-right">
                        {formatCurrency(position.market_value || 0, position.currency || "EUR")}
                      </td>
                      <td className="py-3 text-right">
                        <span
                          className={
                            (position.daily_variation_percent || 0) >= 0 ? "text-green-600" : "text-red-600"
                          }
                        >
                          {formatPercent((position.daily_variation_percent || 0) / 100)}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {positions.length > 20 && (
                <p className="text-sm text-muted-foreground mt-4">
                  Showing 20 of {positions.length} positions
                </p>
              )}
            </div>
          ) : (
            <p className="text-muted-foreground">No open positions found</p>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
