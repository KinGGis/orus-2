/**
 * AUM Evolution Chart - Shows cumulative AUM over time
 * Line chart displaying the evolution of total shareholder investments
 */
import { useMemo } from "react";
import {
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Area,
  AreaChart,
} from "recharts";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui/components/ui/card";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { formatCurrency } from "@/lib/format";
import { format, parseISO } from "date-fns";
import { fr } from "date-fns/locale";
import type { OrusShareholderWithProfile } from "@/features/orus-integration";

interface AUMEvolutionChartProps {
  shareholders: OrusShareholderWithProfile[];
  currency: string;
}

interface DataPoint {
  date: string;
  dateLabel: string;
  cumulativeAUM: number;
  newInvestment: number;
  shareholderName: string;
}

export function AUMEvolutionChart({ shareholders, currency }: AUMEvolutionChartProps) {
  const chartData = useMemo(() => {
    // Get all investment events sorted by date
    const events = shareholders
      .filter((s) => s.participation.investment_date)
      .map((s) => ({
        date: s.participation.investment_date!,
        value: s.participation.total_value || 0,
        name: s.profile?.full_name || s.profile?.email || "Anonyme",
      }))
      .sort((a, b) => new Date(a.date).getTime() - new Date(b.date).getTime());

    if (events.length === 0) return [];

    // Build cumulative data points
    let cumulative = 0;
    const dataPoints: DataPoint[] = [];

    events.forEach((event) => {
      cumulative += event.value;
      dataPoints.push({
        date: event.date,
        dateLabel: format(parseISO(event.date), "MMM yyyy", { locale: fr }),
        cumulativeAUM: cumulative,
        newInvestment: event.value,
        shareholderName: event.name,
      });
    });

    // Add current date as last point if different from last event
    const lastEventDate = events[events.length - 1].date;
    const today = new Date().toISOString().split("T")[0];
    if (lastEventDate !== today) {
      dataPoints.push({
        date: today,
        dateLabel: format(new Date(), "MMM yyyy", { locale: fr }),
        cumulativeAUM: cumulative,
        newInvestment: 0,
        shareholderName: "",
      });
    }

    return dataPoints;
  }, [shareholders]);

  const totalAUM = useMemo(() => {
    return shareholders.reduce((sum, s) => sum + (s.participation.total_value || 0), 0);
  }, [shareholders]);

  const firstInvestmentDate = useMemo(() => {
    const dates = shareholders
      .filter((s) => s.participation.investment_date)
      .map((s) => new Date(s.participation.investment_date!));
    if (dates.length === 0) return null;
    return new Date(Math.min(...dates.map((d) => d.getTime())));
  }, [shareholders]);

  if (chartData.length === 0) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base sm:text-lg">
            <Icons.TrendingUp className="h-4 w-4 text-primary sm:h-5 sm:w-5" />
            Évolution de l'AUM
          </CardTitle>
          <CardDescription>Aucune donnée d'investissement disponible</CardDescription>
        </CardHeader>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base sm:text-lg">
          <Icons.TrendingUp className="h-4 w-4 text-primary sm:h-5 sm:w-5" />
          Évolution de l'AUM
        </CardTitle>
        <CardDescription>
          Entrées cumulées des actionnaires
          {firstInvestmentDate && (
            <span> depuis {format(firstInvestmentDate, "MMMM yyyy", { locale: fr })}</span>
          )}
        </CardDescription>
      </CardHeader>
      <CardContent>
        {/* Current AUM display */}
        <div className="mb-4 flex items-baseline gap-2">
          <span className="text-3xl font-bold text-green-500">
            {formatCurrency(totalAUM, currency)}
          </span>
          <span className="text-muted-foreground text-sm">AUM actuel</span>
        </div>

        {/* Chart */}
        <div className="h-[250px]">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={chartData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
              <defs>
                <linearGradient id="aumGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#22c55e" stopOpacity={0.4} />
                  <stop offset="95%" stopColor="#22c55e" stopOpacity={0.05} />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--muted-foreground) / 0.2)" />
              <XAxis
                dataKey="dateLabel"
                tick={{ fontSize: 12, fill: "hsl(var(--muted-foreground))" }}
                tickLine={false}
                axisLine={false}
              />
              <YAxis
                tickFormatter={(value) => formatCurrency(value, currency)}
                tick={{ fontSize: 12, fill: "hsl(var(--muted-foreground))" }}
                tickLine={false}
                axisLine={false}
                width={80}
              />
              <Tooltip
                content={({ active, payload }) => {
                  if (active && payload && payload.length) {
                    const data = payload[0].payload as DataPoint;
                    return (
                      <div className="rounded-lg border border-border bg-popover p-3 shadow-lg">
                        <p className="font-medium text-popover-foreground">{data.dateLabel}</p>
                        <p className="text-lg font-bold text-green-500">
                          {formatCurrency(data.cumulativeAUM, currency)}
                        </p>
                        {data.newInvestment > 0 && (
                          <p className="text-muted-foreground text-sm">
                            +{formatCurrency(data.newInvestment, currency)} ({data.shareholderName})
                          </p>
                        )}
                      </div>
                    );
                  }
                  return null;
                }}
              />
              <Area
                type="monotone"
                dataKey="cumulativeAUM"
                stroke="#22c55e"
                strokeWidth={2}
                fill="url(#aumGradient)"
                dot={{ r: 4, fill: "#22c55e", stroke: "#22c55e" }}
                activeDot={{ r: 6, fill: "#22c55e", stroke: "#fff", strokeWidth: 2 }}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      </CardContent>
    </Card>
  );
}

export default AUMEvolutionChart;
