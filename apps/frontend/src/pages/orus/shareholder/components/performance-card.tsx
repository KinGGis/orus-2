/**
 * PerformanceCard - Shows investment performance for shareholders
 * Enhanced with MOIC, Annualized Return, Time since investment
 */
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@wealthfolio/ui/components/ui/card";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Minus, Calendar, TrendingUp } from "lucide-react";
import { formatCurrency } from "@/lib/format";
import { format, differenceInDays, differenceInMonths, differenceInYears } from "date-fns";
import { fr } from "date-fns/locale";
import { useMemo } from "react";

interface PerformanceCardProps {
  initialInvestment: number;
  currency: string;
  investmentDate: string | null;
  // Performance metrics (could come from NAV or be calculated)
  percentChange?: number;
  // Optional current value (if not provided, calculated from percentChange)
  currentValue?: number;
}

export function PerformanceCard({
  initialInvestment,
  currency,
  investmentDate,
  percentChange = 0,
  currentValue: providedCurrentValue,
}: PerformanceCardProps) {
  // Calculate current value based on performance
  const currentValue = providedCurrentValue ?? initialInvestment * (1 + percentChange / 100);
  const absoluteChange = currentValue - initialInvestment;

  const isPositive = absoluteChange > 0;
  const isNegative = absoluteChange < 0;

  // Calculate MOIC (Multiple on Invested Capital)
  const moic = useMemo(() => {
    if (initialInvestment <= 0) return 0;
    return currentValue / initialInvestment;
  }, [currentValue, initialInvestment]);

  // Calculate duration and annualized return
  const { duration, durationLabel, annualizedReturn } = useMemo(() => {
    if (!investmentDate) {
      return { duration: 0, durationLabel: "-", annualizedReturn: 0 };
    }

    const startDate = new Date(investmentDate);
    const today = new Date();
    const days = differenceInDays(today, startDate);
    const months = differenceInMonths(today, startDate);
    const years = differenceInYears(today, startDate);

    let label: string;
    if (years >= 1) {
      const remainingMonths = months - years * 12;
      label = remainingMonths > 0 ? `${years} an${years > 1 ? "s" : ""} ${remainingMonths} mois` : `${years} an${years > 1 ? "s" : ""}`;
    } else if (months >= 1) {
      label = `${months} mois`;
    } else {
      label = `${days} jour${days > 1 ? "s" : ""}`;
    }

    // Calculate annualized return using CAGR formula
    const yearFraction = days / 365;
    let annualized = 0;
    if (yearFraction > 0 && initialInvestment > 0) {
      annualized = (Math.pow(currentValue / initialInvestment, 1 / yearFraction) - 1) * 100;
    }

    return { duration: days, durationLabel: label, annualizedReturn: annualized };
  }, [investmentDate, currentValue, initialInvestment]);

  const actualPercentChange = useMemo(() => {
    if (initialInvestment <= 0) return 0;
    return ((currentValue - initialInvestment) / initialInvestment) * 100;
  }, [currentValue, initialInvestment]);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          {isPositive ? (
            <Icons.TrendingUp className="h-5 w-5 text-success" />
          ) : isNegative ? (
            <Icons.TrendingDown className="h-5 w-5 text-destructive" />
          ) : (
            <Minus className="h-5 w-5 text-muted-foreground" />
          )}
          Performance de votre investissement
        </CardTitle>
        <CardDescription>
          Évolution depuis{" "}
          {investmentDate
            ? format(new Date(investmentDate), "MMMM yyyy", { locale: fr })
            : "votre investissement initial"}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-2 gap-4 md:grid-cols-3 lg:grid-cols-6">
          {/* Initial Investment */}
          <div className="space-y-1">
            <p className="text-muted-foreground text-sm">Investissement initial</p>
            <p className="text-xl font-semibold">{formatCurrency(initialInvestment, currency)}</p>
          </div>

          {/* Current Value */}
          <div className="space-y-1">
            <p className="text-muted-foreground text-sm">Valeur actuelle estimée</p>
            <p
              className={`text-xl font-bold ${
                isPositive ? "text-success" : isNegative ? "text-destructive" : "text-foreground"
              }`}
            >
              {formatCurrency(currentValue, currency)}
            </p>
          </div>

          {/* Absolute Change */}
          <div className="space-y-1">
            <p className="text-muted-foreground text-sm">Plus/Moins-value</p>
            <div className="flex items-center gap-1">
              {isPositive ? (
                <Icons.ArrowUp className="h-4 w-4 text-success" />
              ) : isNegative ? (
                <Icons.ArrowDown className="h-4 w-4 text-destructive" />
              ) : null}
              <p
                className={`text-xl font-semibold ${
                  isPositive ? "text-success" : isNegative ? "text-destructive" : "text-foreground"
                }`}
              >
                {isPositive ? "+" : ""}
                {formatCurrency(absoluteChange, currency)}
              </p>
            </div>
          </div>

          {/* Percent Return */}
          <div className="space-y-1">
            <p className="text-muted-foreground text-sm">Rendement total</p>
            <div className="flex items-center gap-1">
              {isPositive ? (
                <Icons.ArrowUp className="h-4 w-4 text-success" />
              ) : isNegative ? (
                <Icons.ArrowDown className="h-4 w-4 text-destructive" />
              ) : null}
              <p
                className={`text-xl font-bold ${
                  isPositive ? "text-success" : isNegative ? "text-destructive" : "text-foreground"
                }`}
              >
                {isPositive ? "+" : ""}
                {actualPercentChange.toFixed(2)}%
              </p>
            </div>
          </div>

          {/* MOIC */}
          <div className="space-y-1">
            <p className="text-muted-foreground text-sm">MOIC</p>
            <div className="flex items-center gap-1">
              <TrendingUp className={`h-4 w-4 ${moic >= 1 ? "text-success" : "text-destructive"}`} />
              <p
                className={`text-xl font-bold ${
                  moic >= 1 ? "text-success" : "text-destructive"
                }`}
              >
                {moic.toFixed(2)}x
              </p>
            </div>
          </div>

          {/* Duration & Annualized Return */}
          <div className="space-y-1">
            <p className="text-muted-foreground text-sm">Durée / Rdt annualisé</p>
            <div className="flex items-center gap-2">
              <Calendar className="h-4 w-4 text-muted-foreground" />
              <div>
                <p className="text-lg font-semibold">{durationLabel}</p>
                {duration > 30 && (
                  <p className={`text-xs ${annualizedReturn >= 0 ? "text-success" : "text-destructive"}`}>
                    {annualizedReturn >= 0 ? "+" : ""}{annualizedReturn.toFixed(1)}%/an
                  </p>
                )}
              </div>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

export default PerformanceCard;
