/**
 * Risk Service
 * Comprehensive risk calculations: performance metrics, VaR, ratios, portfolio analysis
 * Based on Orus riskService.ts
 */
import { useQuery } from "@tanstack/react-query";
import { orusSupabase } from "./supabase-client";
import type { OrusPosition, OrusInstrument } from "./types";

// =====================================================
// TYPES
// =====================================================

export interface PerformanceMetrics {
  currentNAV: number;
  previousNAV: number;
  dailyReturn: number;
  weeklyReturn: number;
  monthlyReturn: number;
  ytdReturn: number;
  yearReturn: number;
  totalReturn: number;
  annualizedReturn: number;
  sharpeRatio: number;
  sortinoRatio: number;
  calmarRatio: number;
  maxDrawdown: number;
  maxDrawdownDuration: number;
  volatility: number;
  trackingError: number;
  informationRatio: number;
}

export interface RiskIndicators {
  valueAtRisk95: number; // 95% VaR
  valueAtRisk99: number; // 99% VaR
  conditionalVaR: number; // CVaR / Expected Shortfall
  beta: number;
  alpha: number;
  treynorRatio: number;
  standardDeviation: number;
  downsideDeviation: number;
  correlationWithMarket: number;
}

export interface PortfolioMetrics {
  totalMarketValue: number;
  totalCostBasis: number;
  unrealizedPnL: number;
  realizedPnL: number;
  totalPnL: number;
  exposureByAssetClass: Record<string, { value: number; percentage: number }>;
  exposureByCurrency: Record<string, { value: number; percentage: number }>;
  exposureByGeography: Record<string, { value: number; percentage: number }>;
  topHoldings: Array<{ symbol: string; name: string; value: number; percentage: number }>;
  concentrationRisk: number; // Herfindahl-Hirschman Index
  diversificationRatio: number;
  numberOfPositions: number;
}

export interface LiquidityMetrics {
  cashBalance: number;
  cashPercentage: number;
  liquidAssets: number;
  liquidAssetsPercentage: number;
  illiquidAssets: number;
  illiquidPercentage: number;
  liquidityRatio: number;
  daysToLiquidate: number;
  averageDailyVolume: number;
}

export interface LeverageMetrics {
  grossExposure: number;
  netExposure: number;
  longExposure: number;
  shortExposure: number;
  leverageRatio: number;
  marginUsed: number;
  marginAvailable: number;
  marginUtilization: number;
}

export interface ComplianceMetrics {
  isCompliant: boolean;
  violations: Array<{
    rule: string;
    current: number;
    limit: number;
    severity: "warning" | "critical";
  }>;
  riskLevel: "low" | "medium" | "high" | "critical";
  lastCheckDate: string;
}

export interface DebtMetrics {
  totalDebt: number;
  principalOutstanding: number;
  accruedInterest: number;
  weightedAverageRate: number;
  averageMaturity: number;
  debtToEquityRatio: number;
  interestCoverageRatio: number;
  debtByType: Record<string, { principal: number; interest: number }>;
  maturitiesSchedule: Array<{ date: string; amount: number }>;
}

export interface RiskAlert {
  id: string;
  category: "performance" | "concentration" | "liquidity" | "leverage" | "compliance" | "market";
  severity: "info" | "warning" | "critical";
  title: string;
  message: string;
  currentValue: number;
  threshold: number;
  createdAt: string;
}

export interface RiskDashboard {
  performance: PerformanceMetrics;
  risk: RiskIndicators;
  portfolio: PortfolioMetrics;
  liquidity: LiquidityMetrics;
  leverage: LeverageMetrics;
  compliance: ComplianceMetrics;
  debt: DebtMetrics;
  alerts: RiskAlert[];
}

// =====================================================
// UTILITY FUNCTIONS
// =====================================================

const RISK_FREE_RATE = 0.04; // 4% annual risk-free rate

/**
 * Calculate standard deviation
 */
function calculateStandardDeviation(values: number[]): number {
  if (values.length < 2) return 0;
  const mean = values.reduce((a, b) => a + b, 0) / values.length;
  const squaredDiffs = values.map((v) => Math.pow(v - mean, 2));
  const variance = squaredDiffs.reduce((a, b) => a + b, 0) / (values.length - 1);
  return Math.sqrt(variance);
}

/**
 * Calculate downside deviation (for Sortino ratio)
 */
function calculateDownsideDeviation(returns: number[], targetReturn: number = 0): number {
  const downsideReturns = returns.filter((r) => r < targetReturn);
  if (downsideReturns.length < 2) return 0;
  const squaredDownside = downsideReturns.map((r) => Math.pow(r - targetReturn, 2));
  const downsideVariance = squaredDownside.reduce((a, b) => a + b, 0) / downsideReturns.length;
  return Math.sqrt(downsideVariance);
}

/**
 * Calculate Value at Risk using historical simulation
 */
function calculateVaR(returns: number[], confidence: number): number {
  if (returns.length === 0) return 0;
  const sorted = [...returns].sort((a, b) => a - b);
  const idx = Math.floor((1 - confidence) * sorted.length);
  return -sorted[idx] || 0;
}

/**
 * Calculate Conditional VaR (Expected Shortfall)
 */
function calculateCVaR(returns: number[], confidence: number): number {
  if (returns.length === 0) return 0;
  const sorted = [...returns].sort((a, b) => a - b);
  const idx = Math.floor((1 - confidence) * sorted.length);
  const tailReturns = sorted.slice(0, idx + 1);
  return -tailReturns.reduce((a, b) => a + b, 0) / tailReturns.length;
}

/**
 * Calculate max drawdown
 */
function calculateMaxDrawdown(navHistory: number[]): { maxDrawdown: number; maxDuration: number } {
  if (navHistory.length < 2) return { maxDrawdown: 0, maxDuration: 0 };

  let maxDrawdown = 0;
  let maxDuration = 0;
  let currentDuration = 0;
  let peak = navHistory[0];

  for (let i = 1; i < navHistory.length; i++) {
    const nav = navHistory[i];
    if (nav > peak) {
      peak = nav;
      currentDuration = 0;
    } else {
      const drawdown = (peak - nav) / peak;
      currentDuration++;
      if (drawdown > maxDrawdown) {
        maxDrawdown = drawdown;
      }
      if (currentDuration > maxDuration) {
        maxDuration = currentDuration;
      }
    }
  }

  return { maxDrawdown, maxDuration };
}

/**
 * Calculate Herfindahl-Hirschman Index (concentration)
 */
function calculateHHI(weights: number[]): number {
  return weights.reduce((sum, w) => sum + Math.pow(w, 2), 0);
}

/**
 * Calculate beta vs market
 */
function calculateBeta(
  portfolioReturns: number[],
  marketReturns: number[]
): { beta: number; alpha: number; correlation: number } {
  const n = Math.min(portfolioReturns.length, marketReturns.length);
  if (n < 10) return { beta: 1, alpha: 0, correlation: 0 };

  const portRet = portfolioReturns.slice(0, n);
  const mktRet = marketReturns.slice(0, n);

  const meanPort = portRet.reduce((a, b) => a + b, 0) / n;
  const meanMkt = mktRet.reduce((a, b) => a + b, 0) / n;

  let cov = 0;
  let varMkt = 0;
  let varPort = 0;

  for (let i = 0; i < n; i++) {
    const portDev = portRet[i] - meanPort;
    const mktDev = mktRet[i] - meanMkt;
    cov += portDev * mktDev;
    varMkt += mktDev * mktDev;
    varPort += portDev * portDev;
  }

  const beta = varMkt > 0 ? cov / varMkt : 1;
  const alpha = meanPort - beta * meanMkt;
  const correlation = Math.sqrt(varPort * varMkt) > 0 ? cov / Math.sqrt(varPort * varMkt) : 0;

  return { beta, alpha, correlation };
}

// =====================================================
// RISK CALCULATION SERVICE
// =====================================================

export class RiskService {
  /**
   * Calculate performance metrics from NAV history
   */
  static calculatePerformance(
    navHistory: Array<{ date: string; nav: number }>
  ): PerformanceMetrics {
    if (navHistory.length < 2) {
      return {
        currentNAV: navHistory[0]?.nav || 0,
        previousNAV: 0,
        dailyReturn: 0,
        weeklyReturn: 0,
        monthlyReturn: 0,
        ytdReturn: 0,
        yearReturn: 0,
        totalReturn: 0,
        annualizedReturn: 0,
        sharpeRatio: 0,
        sortinoRatio: 0,
        calmarRatio: 0,
        maxDrawdown: 0,
        maxDrawdownDuration: 0,
        volatility: 0,
        trackingError: 0,
        informationRatio: 0,
      };
    }

    const sorted = [...navHistory].sort(
      (a, b) => new Date(a.date).getTime() - new Date(b.date).getTime()
    );
    const current = sorted[sorted.length - 1];
    const navValues = sorted.map((h) => h.nav);

    // Calculate returns
    const returns: number[] = [];
    for (let i = 1; i < navValues.length; i++) {
      returns.push((navValues[i] - navValues[i - 1]) / navValues[i - 1]);
    }

    // Period returns
    const getReturnFromDaysAgo = (days: number): number => {
      const target = new Date();
      target.setDate(target.getDate() - days);
      const pastNav = sorted.find(
        (h) => new Date(h.date) >= target
      )?.nav;
      if (!pastNav) return 0;
      return (current.nav - pastNav) / pastNav;
    };

    const dailyReturn = returns.length > 0 ? returns[returns.length - 1] : 0;
    const weeklyReturn = getReturnFromDaysAgo(7);
    const monthlyReturn = getReturnFromDaysAgo(30);
    const yearReturn = getReturnFromDaysAgo(365);

    // YTD return
    const startOfYear = new Date(new Date().getFullYear(), 0, 1);
    const ytdNav = sorted.find((h) => new Date(h.date) >= startOfYear)?.nav || sorted[0].nav;
    const ytdReturn = (current.nav - ytdNav) / ytdNav;

    // Total return
    const totalReturn = (current.nav - sorted[0].nav) / sorted[0].nav;

    // Annualized return
    const days = Math.max(1, (new Date().getTime() - new Date(sorted[0].date).getTime()) / (1000 * 60 * 60 * 24));
    const years = days / 365;
    const annualizedReturn = years > 0 ? Math.pow(1 + totalReturn, 1 / years) - 1 : 0;

    // Volatility (annualized)
    const volatility = calculateStandardDeviation(returns) * Math.sqrt(252);

    // Max drawdown
    const { maxDrawdown, maxDuration } = calculateMaxDrawdown(navValues);

    // Sharpe Ratio
    const meanReturn = returns.reduce((a, b) => a + b, 0) / returns.length;
    const annualizedMeanReturn = meanReturn * 252;
    const sharpeRatio = volatility > 0 ? (annualizedMeanReturn - RISK_FREE_RATE) / volatility : 0;

    // Sortino Ratio
    const downsideDeviation = calculateDownsideDeviation(returns) * Math.sqrt(252);
    const sortinoRatio = downsideDeviation > 0 ? (annualizedMeanReturn - RISK_FREE_RATE) / downsideDeviation : 0;

    // Calmar Ratio
    const calmarRatio = maxDrawdown > 0 ? annualizedReturn / maxDrawdown : 0;

    return {
      currentNAV: current.nav,
      previousNAV: sorted[sorted.length - 2]?.nav || 0,
      dailyReturn,
      weeklyReturn,
      monthlyReturn,
      ytdReturn,
      yearReturn,
      totalReturn,
      annualizedReturn,
      sharpeRatio,
      sortinoRatio,
      calmarRatio,
      maxDrawdown,
      maxDrawdownDuration: maxDuration,
      volatility,
      trackingError: 0, // Would need benchmark data
      informationRatio: 0, // Would need benchmark data
    };
  }

  /**
   * Calculate risk indicators
   */
  static calculateRiskIndicators(
    navHistory: Array<{ date: string; nav: number }>,
    marketReturns: number[] = []
  ): RiskIndicators {
    if (navHistory.length < 2) {
      return {
        valueAtRisk95: 0,
        valueAtRisk99: 0,
        conditionalVaR: 0,
        beta: 1,
        alpha: 0,
        treynorRatio: 0,
        standardDeviation: 0,
        downsideDeviation: 0,
        correlationWithMarket: 0,
      };
    }

    const navValues = navHistory.map((h) => h.nav);
    const returns: number[] = [];
    for (let i = 1; i < navValues.length; i++) {
      returns.push((navValues[i] - navValues[i - 1]) / navValues[i - 1]);
    }

    const valueAtRisk95 = calculateVaR(returns, 0.95);
    const valueAtRisk99 = calculateVaR(returns, 0.99);
    const conditionalVaR = calculateCVaR(returns, 0.95);

    const stdDev = calculateStandardDeviation(returns);
    const downsideDeviation = calculateDownsideDeviation(returns);

    let beta = 1;
    let alpha = 0;
    let correlation = 0;

    if (marketReturns.length > 0) {
      const result = calculateBeta(returns, marketReturns);
      beta = result.beta;
      alpha = result.alpha;
      correlation = result.correlation;
    }

    const meanReturn = returns.reduce((a, b) => a + b, 0) / returns.length;
    const annualizedMean = meanReturn * 252;
    const treynorRatio = beta !== 0 ? (annualizedMean - RISK_FREE_RATE) / beta : 0;

    return {
      valueAtRisk95,
      valueAtRisk99,
      conditionalVaR,
      beta,
      alpha: alpha * 252, // Annualized
      treynorRatio,
      standardDeviation: stdDev * Math.sqrt(252),
      downsideDeviation: downsideDeviation * Math.sqrt(252),
      correlationWithMarket: correlation,
    };
  }

  /**
   * Calculate portfolio metrics
   */
  static calculatePortfolioMetrics(
    positions: OrusPosition[],
    instruments: OrusInstrument[],
    cashBalance: number = 0
  ): PortfolioMetrics {
    const instrumentMap = new Map(instruments.map((i) => [i.id, i]));

    let totalMarketValue = cashBalance;
    let totalCostBasis = 0;
    let realizedPnL = 0;

    const exposureByAssetClass: Record<string, { value: number; percentage: number }> = {};
    const exposureByCurrency: Record<string, { value: number; percentage: number }> = {};
    const exposureByGeography: Record<string, { value: number; percentage: number }> = {};
    const holdings: Array<{ symbol: string; name: string; value: number; percentage: number }> = [];

    for (const pos of positions) {
      const instrument = instrumentMap.get(pos.instrument_id || "");
      const marketValue = pos.market_value || ((pos.quantity || 0) * (pos.price || 0));
      const costBasis = (pos.quantity || 0) * (pos.price || 0);

      totalMarketValue += marketValue;
      totalCostBasis += costBasis;
      // realizedPnL not tracked in current schema

      // Asset class exposure
      const assetClass = pos.asset_class || "Other";
      if (!exposureByAssetClass[assetClass]) {
        exposureByAssetClass[assetClass] = { value: 0, percentage: 0 };
      }
      exposureByAssetClass[assetClass].value += marketValue;

      // Currency exposure
      const currency = pos.currency || instrument?.currency || "USD";
      if (!exposureByCurrency[currency]) {
        exposureByCurrency[currency] = { value: 0, percentage: 0 };
      }
      exposureByCurrency[currency].value += marketValue;

      // Geography exposure
      const geography = pos.country || instrument?.country || "Unknown";
      if (!exposureByGeography[geography]) {
        exposureByGeography[geography] = { value: 0, percentage: 0 };
      }
      exposureByGeography[geography].value += marketValue;

      // Holdings
      holdings.push({
        symbol: instrument?.symbol || pos.instrument || pos.instrument_id || "Unknown",
        name: instrument?.name || pos.instrument || pos.instrument_id || "Unknown",
        value: marketValue,
        percentage: 0,
      });
    }

    // Calculate percentages
    for (const key in exposureByAssetClass) {
      exposureByAssetClass[key].percentage = totalMarketValue > 0
        ? (exposureByAssetClass[key].value / totalMarketValue) * 100
        : 0;
    }
    for (const key in exposureByCurrency) {
      exposureByCurrency[key].percentage = totalMarketValue > 0
        ? (exposureByCurrency[key].value / totalMarketValue) * 100
        : 0;
    }
    for (const key in exposureByGeography) {
      exposureByGeography[key].percentage = totalMarketValue > 0
        ? (exposureByGeography[key].value / totalMarketValue) * 100
        : 0;
    }
    for (const h of holdings) {
      h.percentage = totalMarketValue > 0 ? (h.value / totalMarketValue) * 100 : 0;
    }

    // Sort and get top 10 holdings
    holdings.sort((a, b) => b.value - a.value);
    const topHoldings = holdings.slice(0, 10);

    // Concentration (HHI)
    const weights = holdings.map((h) => h.percentage / 100);
    const concentrationRisk = calculateHHI(weights);

    // Diversification Ratio (simplified)
    const diversificationRatio = positions.length > 0 ? 1 / Math.sqrt(concentrationRisk) : 0;

    return {
      totalMarketValue,
      totalCostBasis,
      unrealizedPnL: totalMarketValue - totalCostBasis,
      realizedPnL,
      totalPnL: totalMarketValue - totalCostBasis + realizedPnL,
      exposureByAssetClass,
      exposureByCurrency,
      exposureByGeography,
      topHoldings,
      concentrationRisk,
      diversificationRatio,
      numberOfPositions: positions.length,
    };
  }

  /**
   * Calculate liquidity metrics
   */
  static calculateLiquidityMetrics(
    positions: OrusPosition[],
    instruments: OrusInstrument[],
    cashBalance: number
  ): LiquidityMetrics {
    const instrumentMap = new Map(instruments.map((i) => [i.id, i]));

    let totalValue = cashBalance;
    let liquidAssets = cashBalance; // Cash is fully liquid
    let illiquidAssets = 0;
    let totalVolume = 0;
    let positionsWithVolume = 0;

    for (const pos of positions) {
      const instrument = instrumentMap.get(pos.instrument_id || "");
      const marketValue = pos.market_value || ((pos.quantity || 0) * (pos.price || 0));
      totalValue += marketValue;

      // Determine liquidity based on asset class
      const assetClass = pos.asset_class?.toLowerCase() || "";
      const isLiquid =
        assetClass.includes("stock") ||
        assetClass.includes("bond") ||
        assetClass.includes("etf") ||
        assetClass.includes("equity");

      if (isLiquid) {
        liquidAssets += marketValue;
      } else {
        illiquidAssets += marketValue;
      }

      // Average daily volume (would need market data)
      // Using placeholder
      if (instrument && marketValue > 0) {
        totalVolume += marketValue * 0.01; // Assume 1% daily turnover
        positionsWithVolume++;
      }
    }

    const averageVolume = positionsWithVolume > 0 ? totalVolume / positionsWithVolume : 0;
    const daysToLiquidate = averageVolume > 0 ? (totalValue - cashBalance) / averageVolume : 999;

    return {
      cashBalance,
      cashPercentage: totalValue > 0 ? (cashBalance / totalValue) * 100 : 0,
      liquidAssets,
      liquidAssetsPercentage: totalValue > 0 ? (liquidAssets / totalValue) * 100 : 0,
      illiquidAssets,
      illiquidPercentage: totalValue > 0 ? (illiquidAssets / totalValue) * 100 : 0,
      liquidityRatio: illiquidAssets > 0 ? liquidAssets / illiquidAssets : Infinity,
      daysToLiquidate: Math.min(daysToLiquidate, 999),
      averageDailyVolume: averageVolume,
    };
  }

  /**
   * Calculate leverage metrics
   */
  static calculateLeverageMetrics(
    positions: OrusPosition[],
    cashBalance: number,
    marginUsed: number = 0,
    marginAvailable: number = 0
  ): LeverageMetrics {
    let longExposure = 0;
    let shortExposure = 0;

    for (const pos of positions) {
      const marketValue = pos.market_value || ((pos.quantity || 0) * (pos.price || 0));
      if ((pos.quantity || 0) > 0) {
        longExposure += marketValue;
      } else {
        shortExposure += Math.abs(marketValue);
      }
    }

    const grossExposure = longExposure + shortExposure;
    const netExposure = longExposure - shortExposure;
    const equity = cashBalance + netExposure;
    const leverageRatio = equity > 0 ? grossExposure / equity : 0;
    const marginUtilization =
      marginUsed + marginAvailable > 0
        ? marginUsed / (marginUsed + marginAvailable)
        : 0;

    return {
      grossExposure,
      netExposure,
      longExposure,
      shortExposure,
      leverageRatio,
      marginUsed,
      marginAvailable,
      marginUtilization,
    };
  }

  /**
   * Calculate debt metrics
   */
  static calculateDebtMetrics(
    debtPositions: Array<{
      type: string;
      principal: number;
      interest_rate: number;
      maturity_date?: string;
      accrued_interest?: number;
    }>,
    equity: number
  ): DebtMetrics {
    let totalPrincipal = 0;
    let totalAccruedInterest = 0;
    let weightedRate = 0;
    let weightedMaturity = 0;
    const debtByType: Record<string, { principal: number; interest: number }> = {};
    const maturities: Array<{ date: string; amount: number }> = [];

    const now = new Date();

    for (const debt of debtPositions) {
      totalPrincipal += debt.principal;
      totalAccruedInterest += debt.accrued_interest || 0;
      weightedRate += debt.principal * debt.interest_rate;

      // Debt by type
      if (!debtByType[debt.type]) {
        debtByType[debt.type] = { principal: 0, interest: 0 };
      }
      debtByType[debt.type].principal += debt.principal;
      debtByType[debt.type].interest += debt.accrued_interest || 0;

      // Maturity calculation
      if (debt.maturity_date) {
        const maturityDate = new Date(debt.maturity_date);
        const yearsToMaturity =
          (maturityDate.getTime() - now.getTime()) / (1000 * 60 * 60 * 24 * 365);
        weightedMaturity += debt.principal * Math.max(0, yearsToMaturity);

        maturities.push({
          date: debt.maturity_date,
          amount: debt.principal,
        });
      }
    }

    // Sort maturities by date
    maturities.sort((a, b) => new Date(a.date).getTime() - new Date(b.date).getTime());

    const totalDebt = totalPrincipal + totalAccruedInterest;
    const avgRate = totalPrincipal > 0 ? weightedRate / totalPrincipal : 0;
    const avgMaturity = totalPrincipal > 0 ? weightedMaturity / totalPrincipal : 0;

    // Interest coverage (would need income data)
    // Using simplified calculation
    const annualInterest = totalPrincipal * avgRate;
    const assumedIncome = equity * 0.1; // Assume 10% return on equity
    const interestCoverage = annualInterest > 0 ? assumedIncome / annualInterest : Infinity;

    return {
      totalDebt,
      principalOutstanding: totalPrincipal,
      accruedInterest: totalAccruedInterest,
      weightedAverageRate: avgRate,
      averageMaturity: avgMaturity,
      debtToEquityRatio: equity > 0 ? totalDebt / equity : Infinity,
      interestCoverageRatio: interestCoverage,
      debtByType,
      maturitiesSchedule: maturities,
    };
  }

  /**
   * Check compliance against limits
   */
  static checkCompliance(
    portfolioMetrics: PortfolioMetrics,
    liquidityMetrics: LiquidityMetrics,
    leverageMetrics: LeverageMetrics,
    limits: {
      maxConcentration?: number;
      maxLeverage?: number;
      minLiquidity?: number;
      maxSinglePosition?: number;
    } = {}
  ): ComplianceMetrics {
    const violations: ComplianceMetrics["violations"] = [];

    const {
      maxConcentration = 0.25, // 25% max in single position
      maxLeverage = 2.0,
      minLiquidity = 0.1, // 10% min cash
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      maxSinglePosition: _maxSinglePosition = 0.2,
    } = limits;

    // Check concentration
    const topHolding = portfolioMetrics.topHoldings[0];
    if (topHolding && topHolding.percentage / 100 > maxConcentration) {
      violations.push({
        rule: "Maximum concentration limit",
        current: topHolding.percentage,
        limit: maxConcentration * 100,
        severity: topHolding.percentage / 100 > maxConcentration * 1.5 ? "critical" : "warning",
      });
    }

    // Check leverage
    if (leverageMetrics.leverageRatio > maxLeverage) {
      violations.push({
        rule: "Maximum leverage limit",
        current: leverageMetrics.leverageRatio,
        limit: maxLeverage,
        severity: leverageMetrics.leverageRatio > maxLeverage * 1.2 ? "critical" : "warning",
      });
    }

    // Check liquidity
    if (liquidityMetrics.cashPercentage / 100 < minLiquidity) {
      violations.push({
        rule: "Minimum liquidity requirement",
        current: liquidityMetrics.cashPercentage,
        limit: minLiquidity * 100,
        severity: liquidityMetrics.cashPercentage / 100 < minLiquidity * 0.5 ? "critical" : "warning",
      });
    }

    // Determine risk level
    const criticalCount = violations.filter((v) => v.severity === "critical").length;
    const warningCount = violations.filter((v) => v.severity === "warning").length;

    let riskLevel: ComplianceMetrics["riskLevel"];
    if (criticalCount > 0) {
      riskLevel = "critical";
    } else if (warningCount > 2) {
      riskLevel = "high";
    } else if (warningCount > 0) {
      riskLevel = "medium";
    } else {
      riskLevel = "low";
    }

    return {
      isCompliant: violations.length === 0,
      violations,
      riskLevel,
      lastCheckDate: new Date().toISOString(),
    };
  }

  /**
   * Generate risk alerts based on all metrics
   */
  static generateRiskAlerts(
    performance: PerformanceMetrics,
    risk: RiskIndicators,
    portfolio: PortfolioMetrics,
    liquidity: LiquidityMetrics,
    leverage: LeverageMetrics
  ): RiskAlert[] {
    const alerts: RiskAlert[] = [];
    const now = new Date().toISOString();

    // Performance alerts
    if (performance.dailyReturn < -0.03) {
      alerts.push({
        id: crypto.randomUUID(),
        category: "performance",
        severity: performance.dailyReturn < -0.05 ? "critical" : "warning",
        title: "Significant daily loss",
        message: `Portfolio declined ${(performance.dailyReturn * 100).toFixed(2)}% today`,
        currentValue: performance.dailyReturn * 100,
        threshold: -3,
        createdAt: now,
      });
    }

    if (performance.maxDrawdown > 0.15) {
      alerts.push({
        id: crypto.randomUUID(),
        category: "performance",
        severity: performance.maxDrawdown > 0.25 ? "critical" : "warning",
        title: "High drawdown",
        message: `Maximum drawdown reached ${(performance.maxDrawdown * 100).toFixed(1)}%`,
        currentValue: performance.maxDrawdown * 100,
        threshold: 15,
        createdAt: now,
      });
    }

    // Risk alerts
    if (risk.valueAtRisk95 > 0.05) {
      alerts.push({
        id: crypto.randomUUID(),
        category: "market",
        severity: risk.valueAtRisk95 > 0.08 ? "critical" : "warning",
        title: "High Value at Risk",
        message: `95% VaR is ${(risk.valueAtRisk95 * 100).toFixed(1)}%`,
        currentValue: risk.valueAtRisk95 * 100,
        threshold: 5,
        createdAt: now,
      });
    }

    // Concentration alerts
    if (portfolio.concentrationRisk > 0.25) {
      alerts.push({
        id: crypto.randomUUID(),
        category: "concentration",
        severity: portfolio.concentrationRisk > 0.4 ? "critical" : "warning",
        title: "High concentration risk",
        message: `Portfolio concentration (HHI) is ${(portfolio.concentrationRisk * 100).toFixed(1)}%`,
        currentValue: portfolio.concentrationRisk * 100,
        threshold: 25,
        createdAt: now,
      });
    }

    // Liquidity alerts
    if (liquidity.cashPercentage < 5) {
      alerts.push({
        id: crypto.randomUUID(),
        category: "liquidity",
        severity: liquidity.cashPercentage < 2 ? "critical" : "warning",
        title: "Low cash position",
        message: `Cash is only ${liquidity.cashPercentage.toFixed(1)}% of portfolio`,
        currentValue: liquidity.cashPercentage,
        threshold: 5,
        createdAt: now,
      });
    }

    // Leverage alerts
    if (leverage.leverageRatio > 1.5) {
      alerts.push({
        id: crypto.randomUUID(),
        category: "leverage",
        severity: leverage.leverageRatio > 2 ? "critical" : "warning",
        title: "High leverage",
        message: `Leverage ratio is ${leverage.leverageRatio.toFixed(2)}x`,
        currentValue: leverage.leverageRatio,
        threshold: 1.5,
        createdAt: now,
      });
    }

    return alerts;
  }

  /**
   * Get full risk dashboard
   */
  static async getFullDashboard(
    positions: OrusPosition[],
    instruments: OrusInstrument[],
    navHistory: Array<{ date: string; nav: number }>,
    cashBalance: number,
    debtPositions: Array<{
      type: string;
      principal: number;
      interest_rate: number;
      maturity_date?: string;
      accrued_interest?: number;
    }> = []
  ): Promise<RiskDashboard> {
    const performance = this.calculatePerformance(navHistory);
    const risk = this.calculateRiskIndicators(navHistory);
    const portfolio = this.calculatePortfolioMetrics(positions, instruments, cashBalance);
    const liquidity = this.calculateLiquidityMetrics(positions, instruments, cashBalance);
    const leverage = this.calculateLeverageMetrics(positions, cashBalance);
    const compliance = this.checkCompliance(portfolio, liquidity, leverage);
    const debt = this.calculateDebtMetrics(debtPositions, portfolio.totalMarketValue);
    const alerts = this.generateRiskAlerts(performance, risk, portfolio, liquidity, leverage);

    return {
      performance,
      risk,
      portfolio,
      liquidity,
      leverage,
      compliance,
      debt,
      alerts,
    };
  }
}

// =====================================================
// REACT HOOKS
// =====================================================

/**
 * Hook to get risk alerts count
 */
export function useRiskAlerts() {
  return useQuery({
    queryKey: ["orus", "risk", "alerts"],
    queryFn: async () => {
      // Fetch positions
      const { data: positions } = await (orusSupabase as any)
        .from("positions")
        .select("*");

      const { data: instruments } = await (orusSupabase as any)
        .from("instruments")
        .select("*");

      const { data: navHistory } = await (orusSupabase as any)
        .from("nav_history")
        .select("*")
        .order("date", { ascending: true });

      const { data: cash } = await (orusSupabase as any)
        .from("cash_balances")
        .select("balance")
        .single();

      const cashBalance = cash?.balance || 0;

      const performance = RiskService.calculatePerformance(navHistory || []);
      const risk = RiskService.calculateRiskIndicators(navHistory || []);
      const portfolio = RiskService.calculatePortfolioMetrics(
        positions || [],
        instruments || [],
        cashBalance
      );
      const liquidity = RiskService.calculateLiquidityMetrics(
        positions || [],
        instruments || [],
        cashBalance
      );
      const leverage = RiskService.calculateLeverageMetrics(positions || [], cashBalance);

      const alerts = RiskService.generateRiskAlerts(
        performance,
        risk,
        portfolio,
        liquidity,
        leverage
      );

      return {
        alerts,
        criticalCount: alerts.filter((a) => a.severity === "critical").length,
        warningCount: alerts.filter((a) => a.severity === "warning").length,
        totalCount: alerts.length,
      };
    },
    staleTime: 5 * 60 * 1000, // 5 minutes
    refetchInterval: 10 * 60 * 1000, // 10 minutes
  });
}

/**
 * Hook to get full risk dashboard
 */
export function useRiskDashboard() {
  return useQuery({
    queryKey: ["orus", "risk", "dashboard"],
    queryFn: async () => {
      // Fetch all required data
      const { data: positions } = await (orusSupabase as any)
        .from("positions")
        .select("*");

      const { data: instruments } = await (orusSupabase as any)
        .from("instruments")
        .select("*");

      const { data: navHistory } = await (orusSupabase as any)
        .from("nav_history")
        .select("*")
        .order("date", { ascending: true });

      const { data: cash } = await (orusSupabase as any)
        .from("cash_balances")
        .select("balance")
        .single();

      const { data: debtPositions } = await (orusSupabase as any)
        .from("debt_positions")
        .select("*");

      const cashBalance = cash?.balance || 0;

      return RiskService.getFullDashboard(
        positions || [],
        instruments || [],
        navHistory || [],
        cashBalance,
        debtPositions || []
      );
    },
    staleTime: 5 * 60 * 1000,
    refetchInterval: 10 * 60 * 1000,
  });
}

/**
 * Hook to get performance metrics only
 */
export function usePerformanceMetrics() {
  return useQuery({
    queryKey: ["orus", "risk", "performance"],
    queryFn: async () => {
      const { data: navHistory } = await (orusSupabase as any)
        .from("nav_history")
        .select("*")
        .order("date", { ascending: true });

      return RiskService.calculatePerformance(navHistory || []);
    },
    staleTime: 5 * 60 * 1000,
  });
}

/**
 * Hook to get portfolio metrics only
 */
export function usePortfolioRiskMetrics() {
  return useQuery({
    queryKey: ["orus", "risk", "portfolio"],
    queryFn: async () => {
      const { data: positions } = await (orusSupabase as any)
        .from("positions")
        .select("*");

      const { data: instruments } = await (orusSupabase as any)
        .from("instruments")
        .select("*");

      const { data: cash } = await (orusSupabase as any)
        .from("cash_balances")
        .select("balance")
        .single();

      const cashBalance = cash?.balance || 0;

      return RiskService.calculatePortfolioMetrics(
        positions || [],
        instruments || [],
        cashBalance
      );
    },
    staleTime: 5 * 60 * 1000,
  });
}
