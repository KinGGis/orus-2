/**
 * CapitalDistributionChart - Pie chart showing ownership distribution
 * Based on Orus CapitalDistributionChart.tsx
 */
import { useMemo } from "react";
import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip } from "recharts";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@wealthfolio/ui/components/ui/card";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { formatCurrency } from "@/lib/format";

interface CapitalDistributionChartProps {
  userCurrency: string;
  allParticipations: Array<{
    user_id: string;
    shares_count: number;
    total_value: number;
  }>;
  currentUserId: string;
}

const COLORS = ["hsl(var(--primary))", "hsl(var(--muted))"];

export function CapitalDistributionChart({
  userCurrency,
  allParticipations,
  currentUserId,
}: CapitalDistributionChartProps) {
  const chartData = useMemo(() => {
    const totalShares = allParticipations.reduce((sum, p) => sum + p.shares_count, 0);
    const userParticipation = allParticipations.find((p) => p.user_id === currentUserId);
    const otherShares = totalShares - (userParticipation?.shares_count || 0);

    const userPercent = totalShares > 0 ? ((userParticipation?.shares_count || 0) / totalShares) * 100 : 0;
    const otherPercent = 100 - userPercent;

    return [
      {
        name: "Ma participation",
        value: userParticipation?.shares_count || 0,
        percent: userPercent,
        totalValue: userParticipation?.total_value || 0,
      },
      {
        name: "Autres investisseurs",
        value: otherShares,
        percent: otherPercent,
        totalValue: allParticipations
          .filter((p) => p.user_id !== currentUserId)
          .reduce((sum, p) => sum + p.total_value, 0),
      },
    ].filter((d) => d.value > 0);
  }, [allParticipations, currentUserId]);

  const totalShares = allParticipations.reduce((sum, p) => sum + p.shares_count, 0);

  if (allParticipations.length === 0) {
    return null;
  }

  return (
    <Card>
      <CardHeader className="pb-2 sm:pb-6">
        <CardTitle className="flex items-center gap-2 text-base sm:text-lg">
          <Icons.BarChart className="h-4 w-4 text-primary sm:h-5 sm:w-5" />
          Répartition du capital
        </CardTitle>
        <CardDescription className="text-xs sm:text-sm">
          Votre part dans le fonds ({totalShares.toLocaleString()} parts au total)
        </CardDescription>
      </CardHeader>
      <CardContent className="pt-0">
        <div className="h-[180px] sm:h-[220px]">
          <ResponsiveContainer width="100%" height="100%">
            <PieChart>
              <Pie
                data={chartData}
                cx="50%"
                cy="50%"
                innerRadius={50}
                outerRadius={75}
                paddingAngle={2}
                dataKey="value"
                label={false}
                labelLine={false}
              >
                {chartData.map((_entry, index) => (
                  <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
                ))}
              </Pie>
              <Tooltip
                content={({ active, payload }) => {
                  if (active && payload && payload.length) {
                    const data = payload[0].payload;
                    return (
                      <div className="rounded-lg border bg-background p-3 shadow-lg">
                        <p className="font-medium">{data.name}</p>
                        <p className="text-muted-foreground text-sm">
                          {data.value.toLocaleString()} parts ({data.percent.toFixed(1)}%)
                        </p>
                        <p className="text-sm font-semibold">{formatCurrency(data.totalValue, userCurrency)}</p>
                      </div>
                    );
                  }
                  return null;
                }}
              />
            </PieChart>
          </ResponsiveContainer>
        </div>

        {/* Legend */}
        <div className="mt-4 space-y-2">
          {chartData.map((entry, index) => (
            <div key={entry.name} className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2">
                <div
                  className="h-3 w-3 rounded-full"
                  style={{ backgroundColor: COLORS[index % COLORS.length] }}
                />
                <span>{entry.name}</span>
              </div>
              <div className="flex items-center gap-4">
                <span className="text-muted-foreground">{entry.value.toLocaleString()} parts</span>
                <span className="font-medium">{entry.percent.toFixed(1)}%</span>
              </div>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

export default CapitalDistributionChart;
