/**
 * Admin Capital Distribution Chart - Pie chart showing all shareholders
 * Non-anonymized version with individual shareholder names
 */
import { useMemo } from "react";
import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip } from "recharts";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui/components/ui/card";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { formatCurrency } from "@/lib/format";
import type { OrusShareholderWithProfile } from "@/features/orus-integration";

interface AdminCapitalDistributionChartProps {
  shareholders: OrusShareholderWithProfile[];
  currency: string;
}

// Color palette for multiple shareholders
const COLORS = [
  "hsl(160, 84%, 39%)", // Primary green
  "hsl(217, 91%, 60%)", // Blue
  "hsl(262, 83%, 58%)", // Purple
  "hsl(24, 95%, 53%)",  // Orange
  "hsl(349, 89%, 60%)", // Red
  "hsl(186, 94%, 41%)", // Cyan
  "hsl(45, 93%, 47%)",  // Yellow
  "hsl(310, 60%, 50%)", // Pink
];

interface ChartDataPoint {
  name: string;
  value: number;
  percent: number;
  totalValue: number;
  color: string;
}

export function AdminCapitalDistributionChart({
  shareholders,
  currency,
}: AdminCapitalDistributionChartProps) {
  const chartData = useMemo(() => {
    const totalShares = shareholders.reduce((sum, s) => sum + s.participation.shares_count, 0);

    // Sort by shares count descending
    const sorted = [...shareholders].sort(
      (a, b) => b.participation.shares_count - a.participation.shares_count
    );

    return sorted.map((s, index): ChartDataPoint => ({
      name: s.profile?.full_name || s.profile?.email?.split("@")[0] || `Actionnaire ${index + 1}`,
      value: s.participation.shares_count,
      percent: totalShares > 0 ? (s.participation.shares_count / totalShares) * 100 : 0,
      totalValue: s.participation.total_value || 0,
      color: COLORS[index % COLORS.length],
    }));
  }, [shareholders]);

  const totalShares = shareholders.reduce((sum, s) => sum + s.participation.shares_count, 0);
  const totalValue = shareholders.reduce((sum, s) => sum + (s.participation.total_value || 0), 0);

  if (shareholders.length === 0) {
    return null;
  }

  return (
    <Card>
      <CardHeader className="pb-2 sm:pb-6">
        <CardTitle className="flex items-center gap-2 text-base sm:text-lg">
          <Icons.PieChart className="h-4 w-4 text-primary sm:h-5 sm:w-5" />
          Répartition du capital
        </CardTitle>
        <CardDescription className="text-xs sm:text-sm">
          {shareholders.length} actionnaire{shareholders.length > 1 ? "s" : ""} •{" "}
          {totalShares.toLocaleString()} parts • {formatCurrency(totalValue, currency)}
        </CardDescription>
      </CardHeader>
      <CardContent className="pt-0">
        <div className="h-[250px]">
          <ResponsiveContainer width="100%" height="100%">
            <PieChart>
              <Pie
                data={chartData}
                cx="50%"
                cy="50%"
                innerRadius={60}
                outerRadius={90}
                paddingAngle={2}
                dataKey="value"
                label={({ percent }) => (percent && percent > 5 ? `${percent.toFixed(0)}%` : "")}
                labelLine={false}
              >
                {chartData.map((entry, index) => (
                  <Cell key={`cell-${index}`} fill={entry.color} />
                ))}
              </Pie>
              <Tooltip
                content={({ active, payload }) => {
                  if (active && payload && payload.length) {
                    const data = payload[0].payload as ChartDataPoint;
                    return (
                      <div className="rounded-lg border bg-background p-3 shadow-lg">
                        <div className="flex items-center gap-2">
                          <div
                            className="h-3 w-3 rounded-full"
                            style={{ backgroundColor: data.color }}
                          />
                          <p className="font-medium">{data.name}</p>
                        </div>
                        <p className="text-muted-foreground text-sm">
                          {data.value.toLocaleString()} parts ({data.percent.toFixed(1)}%)
                        </p>
                        <p className="text-primary text-sm font-semibold">
                          {formatCurrency(data.totalValue, currency)}
                        </p>
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
        <div className="mt-4 max-h-[150px] space-y-2 overflow-y-auto">
          {chartData.map((entry) => (
            <div key={entry.name} className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2">
                <div
                  className="h-3 w-3 flex-shrink-0 rounded-full"
                  style={{ backgroundColor: entry.color }}
                />
                <span className="truncate">{entry.name}</span>
              </div>
              <div className="flex items-center gap-4 flex-shrink-0">
                <span className="text-muted-foreground">
                  {entry.value.toLocaleString()} parts
                </span>
                <span className="font-medium w-14 text-right">{entry.percent.toFixed(1)}%</span>
              </div>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

export default AdminCapitalDistributionChart;
