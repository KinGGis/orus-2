/**
 * Formatting utilities for Orus integration
 */

export function formatCurrency(
  value: number | null | undefined,
  currency: string = "EUR",
  options?: Intl.NumberFormatOptions,
): string {
  if (value === null || value === undefined) return "-";
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    minimumFractionDigits: 0,
    maximumFractionDigits: 2,
    ...options,
  }).format(value);
}

export function formatNumber(
  value: number | null | undefined,
  options?: Intl.NumberFormatOptions,
): string {
  if (value === null || value === undefined) return "-";
  return new Intl.NumberFormat(undefined, {
    minimumFractionDigits: 0,
    maximumFractionDigits: 4,
    ...options,
  }).format(value);
}

export function formatPercent(
  value: number | null | undefined,
  options?: Intl.NumberFormatOptions,
): string {
  if (value === null || value === undefined) return "-";
  return new Intl.NumberFormat(undefined, {
    style: "percent",
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
    ...options,
  }).format(value);
}

export function formatDate(
  value: string | Date | null | undefined,
  options?: Intl.DateTimeFormatOptions,
): string {
  if (!value) return "-";
  const date = typeof value === "string" ? new Date(value) : value;
  return new Intl.DateTimeFormat(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    ...options,
  }).format(date);
}
