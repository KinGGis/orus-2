import {
  checkActivitiesImport,
  getAssets,
  getHoldings,
  logger,
  saveAccountImportMapping,
} from "@/adapters";
import {
  ACTIVITY_SUBTYPES,
  ActivityType,
  ImportFormat,
  SUBTYPES_BY_ACTIVITY_TYPE,
} from "@/lib/constants";
import type { ActivityImport, Asset, SymbolSearchResult } from "@/lib/types";
import { tryParseDate } from "@/lib/utils";
import { QueryKeys } from "@/lib/query-keys";
import { useQuery } from "@tanstack/react-query";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { ProgressIndicator } from "@wealthfolio/ui/components/ui/progress-indicator";
import { isValid, parse, parseISO } from "date-fns";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ImportAlert } from "../components/import-alert";
import { ImportReviewGrid, type ImportReviewFilter } from "../components/import-review-grid";
import {
  SymbolResolutionPanel,
  type UnresolvedSymbol,
} from "../components/symbol-resolution-panel";
import {
  bulkSetAccount,
  bulkSetCurrency,
  bulkSkipDrafts,
  bulkUnskipDrafts,
  setDraftActivities,
  setMapping,
  updateDraft,
  useImportContext,
  type DraftActivity,
  type DraftActivityStatus,
} from "../context";
import { findMappedActivityType } from "../utils/activity-type-mapping";
import { getDateFnsPattern } from "../utils/date-format-options";
import { normalizeInstrumentType, splitInstrumentPrefixedSymbol } from "../utils/instrument-type";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

interface FilterStats {
  all: number;
  errors: number;
  warnings: number;
  duplicates: number;
  skipped: number;
  valid: number;
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Parse a numeric value from a string, handling various formats
 */
function parseNumericValue(
  value: string | undefined,
  decimalSeparator: string,
  thousandsSeparator: string,
): string | undefined {
  if (!value || value.trim() === "") return undefined;

  let normalized = value.trim();
  let isNegative = false;

  if (normalized.startsWith("(") && normalized.endsWith(")")) {
    isNegative = true;
    normalized = normalized.slice(1, -1);
  }

  let mantissa = normalized;
  let exponent = "";
  const expIndex = normalized.search(/[eE]/);
  if (expIndex >= 0) {
    mantissa = normalized.slice(0, expIndex);
    exponent = normalized.slice(expIndex + 1);
  }

  const lastComma = mantissa.lastIndexOf(",");
  const lastDot = mantissa.lastIndexOf(".");
  let resolvedDecimal = decimalSeparator;
  if (decimalSeparator === "auto") {
    if (lastComma !== -1 && lastDot !== -1) {
      resolvedDecimal = lastComma > lastDot ? "," : ".";
    } else if (lastComma !== -1) {
      resolvedDecimal = ",";
    } else {
      resolvedDecimal = ".";
    }
  }

  let cleaned = mantissa.replace(/[^\d.,+-]/g, "");

  if (thousandsSeparator !== "none" && thousandsSeparator !== "auto") {
    cleaned = cleaned.replace(new RegExp(`\\${thousandsSeparator}`, "g"), "");
  } else {
    const defaultThousands = resolvedDecimal === "," ? "." : ",";
    cleaned = cleaned.replace(new RegExp(`\\${defaultThousands}`, "g"), "");
  }

  if (resolvedDecimal === ",") {
    const parts = cleaned.split(",");
    if (parts.length > 1) {
      const decimalPart = parts.pop() ?? "";
      cleaned = `${parts.join("")}.${decimalPart}`;
    }
  } else {
    const parts = cleaned.split(".");
    if (parts.length > 1) {
      const decimalPart = parts.pop() ?? "";
      cleaned = `${parts.join("")}.${decimalPart}`;
    }
  }

  const expClean = exponent.replace(/[^\d+-]/g, "");
  let candidate = cleaned;
  if (isNegative && candidate && !candidate.startsWith("-")) {
    candidate = `-${candidate}`;
  }
  if (expClean) {
    candidate = `${candidate}e${expClean}`;
  }

  if (candidate === "" || candidate === "-" || candidate === "+") {
    return undefined;
  }

  const numericCheck = Number(candidate);
  return Number.isFinite(numericCheck) ? candidate : undefined;
}

function toNumber(value: string | number | null | undefined): number | undefined {
  if (value === null || value === undefined || value === "") {
    return undefined;
  }
  const parsed = typeof value === "number" ? value : Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function hasPositiveValue(value: string | number | null | undefined): boolean {
  const parsed = toNumber(value);
  return parsed !== undefined && parsed > 0;
}

function hasNonZeroValue(value: string | number | null | undefined): boolean {
  const parsed = toNumber(value);
  return parsed !== undefined && parsed !== 0;
}

function mergeIssueMaps(
  current: Record<string, string[]>,
  incoming: Record<string, string[]>,
): Record<string, string[]> {
  const merged: Record<string, string[]> = { ...current };
  for (const [key, messages] of Object.entries(incoming)) {
    const existing = merged[key] ?? [];
    const next = [...existing];
    for (const message of messages) {
      if (!next.includes(message)) {
        next.push(message);
      }
    }
    merged[key] = next;
  }
  return merged;
}

function hasDuplicateWarning(draft: DraftActivity): boolean {
  const hasDuplicateLineNumber = typeof draft.duplicateOfLineNumber === "number";
  return Boolean(
    draft.duplicateOfId || hasDuplicateLineNumber || draft.warnings?._duplicate?.length,
  );
}

/**
 * Parse a date value using the configured format (priority) then auto-detection fallback.
 * Returns a full ISO datetime string preserving any time component from the source.
 */
function parseDateValue(value: string | undefined, dateFormat: string): string {
  if (!value || value.trim() === "") return "";

  const trimmed = value.trim();

  // 1. If user specified a format, try it first
  const pattern = getDateFnsPattern(dateFormat);
  if (pattern) {
    try {
      const parsed = parse(trimmed, pattern, new Date());
      if (isValid(parsed)) return parsed.toISOString();
    } catch {
      // fall through to auto-detection
    }
  }

  // 2. For ISO8601 preset, try parseISO directly
  if (dateFormat === "ISO8601") {
    try {
      const parsed = parseISO(trimmed);
      if (isValid(parsed)) return parsed.toISOString();
    } catch {
      // fall through
    }
  }

  // 3. Auto-detection fallback (handles 80+ formats)
  const autoDetected = tryParseDate(trimmed);
  if (autoDetected) return autoDetected.toISOString();

  // 4. Return as-is if nothing works (will surface as validation error)
  return trimmed;
}

/**
 * Map a CSV activity type value to a Wealthfolio activity type.
 * Uses findMappedActivityType which checks explicit mappings + smart defaults.
 */
function mapActivityType(
  csvValue: string | undefined,
  activityMappings: Record<string, string[]>,
): string | undefined {
  if (!csvValue) return undefined;
  return findMappedActivityType(csvValue, activityMappings) ?? csvValue.trim();
}

/**
 * Map a CSV symbol to a resolved symbol, optionally with exchange MIC and name metadata
 */
function mapSymbol(
  csvSymbol: string | undefined,
  symbolMappings: Record<string, string>,
  symbolMappingMeta?: Record<
    string,
    {
      exchangeMic?: string;
      symbolName?: string;
      quoteCcy?: string;
      instrumentType?: string;
      quoteMode?: string;
    }
  >,
): {
  symbol: string | undefined;
  exchangeMic?: string;
  symbolName?: string;
  quoteCcy?: string;
  instrumentType?: string;
  quoteMode?: string;
} {
  if (!csvSymbol) return { symbol: undefined };

  const trimmed = csvSymbol.trim();
  const normalizeKey = (value: string): string =>
    value
      .trim()
      .toUpperCase()
      .replace(/\s+/g, "")
      .replace(/[^A-Z0-9]/g, "");

  const upperTrimmed = trimmed.toUpperCase();
  const normalizedTrimmed = normalizeKey(trimmed);

  const directKey =
    symbolMappings[trimmed] !== undefined
      ? trimmed
      : symbolMappings[upperTrimmed] !== undefined
        ? upperTrimmed
        : undefined;

  let resolvedKey = directKey;
  if (!resolvedKey) {
    resolvedKey = Object.keys(symbolMappings).find((key) => {
      const keyTrimmed = key.trim();
      if (keyTrimmed.toUpperCase() === upperTrimmed) return true;
      return normalizeKey(keyTrimmed) === normalizedTrimmed;
    });
  }

  const symbol = resolvedKey ? symbolMappings[resolvedKey] : trimmed;

  const metaKey = resolvedKey
    ? resolvedKey
    : symbolMappingMeta
      ? Object.keys(symbolMappingMeta).find((key) => {
          const keyTrimmed = key.trim();
          if (keyTrimmed.toUpperCase() === upperTrimmed) return true;
          return normalizeKey(keyTrimmed) === normalizedTrimmed;
        })
      : undefined;

  const meta = metaKey ? symbolMappingMeta?.[metaKey] : symbolMappingMeta?.[trimmed];
  return {
    symbol,
    exchangeMic: meta?.exchangeMic,
    symbolName: meta?.symbolName,
    quoteCcy: meta?.quoteCcy,
    instrumentType: meta?.instrumentType,
    quoteMode: meta?.quoteMode,
  };
}

/**
 * Validate a draft activity and return errors/warnings
 */
function validateDraft(draft: Partial<DraftActivity>): {
  status: DraftActivityStatus;
  errors: Record<string, string[]>;
  warnings: Record<string, string[]>;
} {
  const errors: Record<string, string[]> = {};
  const warnings: Record<string, string[]> = {};

  // Required field validation
  if (!draft.activityDate) {
    errors.activityDate = ["Date is required"];
  }

  if (!draft.activityType) {
    errors.activityType = ["Activity type is required"];
  }

  if (!draft.currency) {
    errors.currency = ["Currency is required"];
  }

  if (!draft.accountId) {
    errors.accountId = ["Account is required"];
  }

  // Activity-type specific validation
  const activityType = draft.activityType?.toUpperCase();
  const subtype = draft.subtype?.toUpperCase();

  // Validate subtype is allowed for this activity type
  if (subtype && activityType) {
    const allowedSubtypes = SUBTYPES_BY_ACTIVITY_TYPE[activityType] || [];
    if (allowedSubtypes.length > 0 && !allowedSubtypes.includes(subtype)) {
      warnings.subtype = [`'${subtype}' is not a recognized subtype for ${activityType}`];
    }
  }

  // Trade activities (BUY/SELL)
  if (activityType === ActivityType.BUY || activityType === ActivityType.SELL) {
    if (!draft.symbol) {
      errors.symbol = ["Symbol is required for trade activities"];
    }
    if (!hasPositiveValue(draft.quantity)) {
      errors.quantity = ["Quantity must be greater than 0"];
    }
    if (!hasPositiveValue(draft.unitPrice)) {
      errors.unitPrice = ["Unit price must be greater than 0"];
    }
  }

  // DIVIDEND validation
  if (activityType === ActivityType.DIVIDEND) {
    if (!draft.symbol) {
      errors.symbol = ["Symbol is required for dividend activities"];
    }

    if (subtype === ACTIVITY_SUBTYPES.DRIP) {
      // DRIP: cash dividend → reinvested as BUY of same ticker
      // Needs: quantity (shares received), unit price (reinvest price)
      // Amount is optional (dividend cash amount)
      if (!hasPositiveValue(draft.quantity)) {
        errors.quantity = ["Quantity is required for DRIP (shares received)"];
      }
      if (!hasPositiveValue(draft.unitPrice)) {
        errors.unitPrice = ["Unit price is required for DRIP (reinvestment price)"];
      }
    } else if (subtype === ACTIVITY_SUBTYPES.DIVIDEND_IN_KIND) {
      // DIVIDEND_IN_KIND: dividend paid in asset (not cash)
      // Needs: symbol (received asset), quantity, unit price (FMV), amount (value)
      if (!hasPositiveValue(draft.quantity)) {
        errors.quantity = ["Quantity is required for dividend in kind (shares received)"];
      }
      if (!hasPositiveValue(draft.unitPrice)) {
        errors.unitPrice = ["Unit price is required for dividend in kind (FMV at receipt)"];
      }
      if (!hasNonZeroValue(draft.amount)) {
        errors.amount = ["Amount is required for dividend in kind (value of shares)"];
      }
    } else {
      // Regular cash dividend - amount is required
      if (!hasNonZeroValue(draft.amount)) {
        errors.amount = ["Amount is required for dividend activities"];
      }
    }
  }

  // INTEREST validation
  if (activityType === ActivityType.INTEREST) {
    // STAKING_REWARD - needs quantity (tokens received) and may have unit price
    if (subtype === ACTIVITY_SUBTYPES.STAKING_REWARD) {
      if (!draft.symbol) {
        errors.symbol = ["Symbol is required for staking rewards"];
      }
      if (!hasPositiveValue(draft.quantity)) {
        errors.quantity = ["Quantity is required for staking rewards (tokens received)"];
      }
      // Amount is optional for staking - can be calculated from quantity * price
      if (!hasNonZeroValue(draft.amount) && !hasPositiveValue(draft.unitPrice)) {
        warnings.amount = ["Either amount or unit price is recommended for staking rewards"];
      }
    } else {
      // Regular interest - amount is required
      if (!hasNonZeroValue(draft.amount)) {
        errors.amount = ["Amount is required for interest activities"];
      }
    }
  }

  // DEPOSIT/WITHDRAWAL - amount is required
  if (activityType === ActivityType.DEPOSIT || activityType === ActivityType.WITHDRAWAL) {
    if (!hasNonZeroValue(draft.amount)) {
      errors.amount = ["Amount is required for deposit/withdrawal activities"];
    }
  }

  // FEE validation - either fee or amount required
  if (activityType === ActivityType.FEE) {
    const hasFee = hasPositiveValue(draft.fee);
    const hasAmount = hasPositiveValue(draft.amount);
    if (!hasFee && !hasAmount) {
      errors.fee = ["Either fee or amount is required for fee activities"];
    }
  }

  // TAX validation - amount is required
  if (activityType === ActivityType.TAX) {
    const hasFee = hasPositiveValue(draft.fee);
    const hasAmount = hasPositiveValue(draft.amount);
    if (!hasFee && !hasAmount) {
      errors.amount = ["Amount or fee is required for tax activities"];
    }
  }

  // TRANSFER_IN/TRANSFER_OUT - amount or quantity required
  if (activityType === ActivityType.TRANSFER_IN || activityType === ActivityType.TRANSFER_OUT) {
    const hasAmount = hasPositiveValue(draft.amount);
    const hasQuantity = hasPositiveValue(draft.quantity);
    if (!hasAmount && !hasQuantity) {
      errors.amount = ["Amount or quantity is required for transfer activities"];
    }
  }

  // SPLIT validation
  if (activityType === ActivityType.SPLIT) {
    if (!draft.symbol) {
      errors.symbol = ["Symbol is required for split activities"];
    }
    if (toNumber(draft.amount) === undefined) {
      errors.amount = ["Amount (split ratio) is required for split activities"];
    }
  }

  // CREDIT validation
  if (activityType === ActivityType.CREDIT) {
    if (!hasNonZeroValue(draft.amount)) {
      errors.amount = ["Amount is required for credit activities"];
    }
  }

  // Determine status
  const hasErrors = Object.keys(errors).length > 0;
  const hasWarnings = Object.keys(warnings).length > 0;

  let status: DraftActivityStatus = "valid";
  if (hasErrors) {
    status = "error";
  } else if (hasWarnings) {
    status = "warning";
  }

  return { status, errors, warnings };
}

/**
 * Create DraftActivity objects from parsed CSV data and mapping
 */
function createDraftActivities(
  parsedRows: string[][],
  headers: string[],
  mapping: {
    fieldMappings: Record<string, string>;
    activityMappings: Record<string, string[]>;
    symbolMappings: Record<string, string>;
    accountMappings: Record<string, string>;
    symbolMappingMeta?: Record<
      string,
      { exchangeMic?: string; symbolName?: string; quoteCcy?: string; instrumentType?: string }
    >;
  },
  parseConfig: {
    dateFormat: string;
    decimalSeparator: string;
    thousandsSeparator: string;
    defaultCurrency: string;
  },
  defaultAccountId: string,
  exactSymbolMatches?: Record<
    string,
    {
      symbol: string;
      exchangeMic?: string;
      symbolName?: string;
      quoteCcy?: string;
      instrumentType?: string;
      quoteMode?: string;
    }
  >,
): DraftActivity[] {
  const { fieldMappings, activityMappings, symbolMappings, accountMappings, symbolMappingMeta } =
    mapping;
  const { dateFormat, decimalSeparator, thousandsSeparator, defaultCurrency } = parseConfig;

  // Create header index lookup
  const headerIndex: Record<string, number> = {};
  headers.forEach((header, idx) => {
    headerIndex[header] = idx;
  });

  // Get column indices for each mapped field
  const getColumnValue = (row: string[], field: ImportFormat): string | undefined => {
    const csvHeader = fieldMappings[field];
    if (!csvHeader) return undefined;
    const idx = headerIndex[csvHeader];
    if (idx === undefined) return undefined;
    return row[idx];
  };

  if (import.meta.env.DEV) {
    const symbolHeader = fieldMappings[ImportFormat.SYMBOL];
    const symbolIndex = symbolHeader ? headerIndex[symbolHeader] : undefined;
    if (symbolIndex !== undefined) {
      const uniqueRawSymbols = new Set<string>();
      const mappedHits: string[] = [];
      for (const row of parsedRows) {
        const raw = row[symbolIndex]?.trim();
        if (!raw) continue;
        if (uniqueRawSymbols.has(raw)) continue;
        uniqueRawSymbols.add(raw);
        if (symbolMappings[raw] !== undefined) {
          mappedHits.push(raw);
        }
      }

      logger.warn(
        "[ImportReview] draft build mapping hits",
        JSON.stringify({
          uniqueRawSymbols: uniqueRawSymbols.size,
          mappingEntries: Object.keys(symbolMappings || {}).length,
          directHitCount: mappedHits.length,
          directHitPreview: mappedHits.slice(0, 20),
        }),
      );
    }
  }

  return parsedRows.map((row, rowIndex): DraftActivity => {
    // Extract raw values from CSV
    const rawDate = getColumnValue(row, ImportFormat.DATE);
    const rawType = getColumnValue(row, ImportFormat.ACTIVITY_TYPE);
    const rawSymbol = getColumnValue(row, ImportFormat.SYMBOL);
    const rawQuantity = getColumnValue(row, ImportFormat.QUANTITY);
    const rawUnitPrice = getColumnValue(row, ImportFormat.UNIT_PRICE);
    const rawAmount = getColumnValue(row, ImportFormat.AMOUNT);
    const rawCurrency = getColumnValue(row, ImportFormat.CURRENCY);
    const rawFee = getColumnValue(row, ImportFormat.FEE);
    const rawComment = getColumnValue(row, ImportFormat.COMMENT);
    const rawAccount = getColumnValue(row, ImportFormat.ACCOUNT);
    const rawFxRate = getColumnValue(row, ImportFormat.FX_RATE);
    const rawSubtype = getColumnValue(row, ImportFormat.SUBTYPE);
    const rawInstrumentType = getColumnValue(row, ImportFormat.INSTRUMENT_TYPE);

    // Parse and normalize values
    const activityDate = parseDateValue(rawDate, dateFormat);
    const activityType = mapActivityType(rawType, activityMappings);
    const exactMatch = rawSymbol?.trim()
      ? (() => {
          const rawKey = rawSymbol.trim().toUpperCase();
          const normalizedKey = rawKey.replace(/\s+/g, "").replace(/[^A-Z0-9]/g, "");
          return exactSymbolMatches?.[rawKey] || exactSymbolMatches?.[normalizedKey];
        })()
      : undefined;
    const {
      symbol: mappedSymbol,
      exchangeMic: mappedExchangeMic,
      symbolName: mappedSymbolName,
      quoteCcy: mappedQuoteCcy,
      instrumentType: mappedInstrumentType,
      quoteMode: mappedQuoteMode,
    } = mapSymbol(rawSymbol, symbolMappings, symbolMappingMeta);

    // Also look up the canonical (mapped) symbol in the portfolio to fill missing metadata
    // (e.g. MC→MC.PA: symbolMappingMeta may lack exchangeMic, but the portfolio asset has it)
    const canonicalMatch = mappedSymbol?.trim()
      ? (() => {
          const canonicalKey = mappedSymbol.trim().toUpperCase();
          const normalizedKey = canonicalKey.replace(/\s+/g, "").replace(/[^A-Z0-9]/g, "");
          return exactSymbolMatches?.[canonicalKey] || exactSymbolMatches?.[normalizedKey];
        })()
      : undefined;

    const effectiveSymbol = exactMatch?.symbol ?? mappedSymbol;
    const effectiveExchangeMic =
      exactMatch?.exchangeMic ?? mappedExchangeMic ?? canonicalMatch?.exchangeMic;
    const effectiveSymbolName =
      exactMatch?.symbolName ?? mappedSymbolName ?? canonicalMatch?.symbolName;
    const effectiveQuoteCcy =
      exactMatch?.quoteCcy ?? mappedQuoteCcy ?? canonicalMatch?.quoteCcy;
    const effectiveInstrumentType =
      exactMatch?.instrumentType ?? mappedInstrumentType ?? canonicalMatch?.instrumentType;
    const effectiveQuoteMode =
      exactMatch?.quoteMode ?? mappedQuoteMode ?? canonicalMatch?.quoteMode;

    // Parse typed symbol prefixes (e.g., "bond:US037833DU14")
    const { symbol: prefixParsedSymbol, instrumentType: prefixInstrumentType } =
      splitInstrumentPrefixedSymbol(effectiveSymbol);
    const symbol = prefixParsedSymbol;

    // Normalize instrument type: explicit CSV column > prefix > symbol mapping meta
    const normalizedCsvInstrumentType = normalizeInstrumentType(rawInstrumentType);
    const resolvedInstrumentType =
      normalizedCsvInstrumentType || prefixInstrumentType || effectiveInstrumentType;
    const quantity = parseNumericValue(rawQuantity, decimalSeparator, thousandsSeparator);
    const unitPrice = parseNumericValue(rawUnitPrice, decimalSeparator, thousandsSeparator);
    const amount = parseNumericValue(rawAmount, decimalSeparator, thousandsSeparator);
    const currency = rawCurrency?.trim() || defaultCurrency;
    const fee = parseNumericValue(rawFee, decimalSeparator, thousandsSeparator);
    const comment = rawComment?.trim();
    const fxRate = parseNumericValue(rawFxRate, decimalSeparator, thousandsSeparator);
    const subtype = rawSubtype?.trim().toUpperCase() || undefined;

    // Resolve account ID: use CSV account mapping, or fall back to default
    let accountId = defaultAccountId;
    if (rawAccount?.trim()) {
      const mappedAccount = accountMappings[rawAccount.trim()];
      if (mappedAccount) {
        accountId = mappedAccount;
      } else if (rawAccount.trim()) {
        // Use raw account value if no mapping exists (might be an account ID already)
        accountId = rawAccount.trim();
      }
    }

    // Create draft object
    const draft: Partial<DraftActivity> = {
      rowIndex,
      rawRow: row,
      activityDate,
      activityType,
      symbol,
      exchangeMic: effectiveExchangeMic,
      symbolName: effectiveSymbolName,
      quoteCcy: effectiveQuoteCcy,
      instrumentType: resolvedInstrumentType,
      quoteMode: effectiveQuoteMode,
      quantity,
      unitPrice,
      amount,
      currency,
      fee,
      fxRate,
      subtype,
      accountId,
      comment,
      isEdited: false,
    };

    // Validate and get status
    const validation = validateDraft(draft);

    return {
      ...draft,
      status: validation.status,
      errors: validation.errors,
      warnings: validation.warnings,
    } as DraftActivity;
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// Filter Stats Component
// ─────────────────────────────────────────────────────────────────────────────

interface FilterStatsProps {
  stats: FilterStats;
  currentFilter: ImportReviewFilter;
  onFilterChange: (filter: ImportReviewFilter) => void;
}

function FilterStatsBar({ stats, currentFilter, onFilterChange }: FilterStatsProps) {
  // Define filter configs - only show colored variants when count > 0
  const filters: {
    id: ImportReviewFilter;
    label: string;
    count: number;
    colorVariant: "default" | "destructive" | "secondary" | "outline";
  }[] = [
    { id: "all", label: "All", count: stats.all, colorVariant: "secondary" },
    { id: "errors", label: "Errors", count: stats.errors, colorVariant: "destructive" },
    { id: "warnings", label: "Warnings", count: stats.warnings, colorVariant: "secondary" },
    { id: "duplicates", label: "Duplicates", count: stats.duplicates, colorVariant: "secondary" },
    { id: "skipped", label: "Skipped", count: stats.skipped, colorVariant: "secondary" },
  ];

  return (
    <div className="flex flex-wrap items-center gap-2">
      {filters.map((filter) => {
        // Use colored variant only when count > 0, otherwise use outline
        const variant =
          currentFilter === filter.id
            ? "default"
            : filter.count > 0
              ? filter.colorVariant
              : "outline";

        return (
          <Badge
            key={filter.id}
            variant={variant}
            className={`cursor-pointer transition-all ${
              currentFilter === filter.id ? "" : "opacity-70 hover:opacity-100"
            }`}
            onClick={() => onFilterChange(filter.id)}
          >
            {filter.label}: {filter.count}
          </Badge>
        );
      })}
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Component
// ─────────────────────────────────────────────────────────────────────────────

export function ReviewStep() {
  const { state, dispatch } = useImportContext();
  const { parsedRows, headers, mapping, parseConfig, accountId, draftActivities } = state;

  const [selectedRows, setSelectedRows] = useState<number[]>([]);
  const [filter, setFilter] = useState<ImportReviewFilter>("all");
  const [isValidating, setIsValidating] = useState(false);
  const validationRunRef = useRef(0);

  const validateDraftsWithBackend = useCallback(
    async (drafts: DraftActivity[]) => {
      const validationRun = ++validationRunRef.current;
      setIsValidating(true);
      try {
        if (!accountId) {
          logger.warn("No account selected - skipping backend validation");
          if (validationRun === validationRunRef.current) {
            dispatch(setDraftActivities(drafts));
          }
          return;
        }

        const activitiesToValidate = drafts
          .filter((d) => d.status !== "skipped" && d.activityType)
          .map(
            (draft) =>
              ({
                accountId: draft.accountId || accountId,
                activityType: draft.activityType as ActivityImport["activityType"],
                date: draft.activityDate || "",
                symbol: draft.symbol || "",
                exchangeMic: draft.exchangeMic,
                quoteCcy: draft.quoteCcy,
                instrumentType: draft.instrumentType,
                quoteMode: draft.quoteMode,
                quantity: draft.quantity,
                unitPrice: draft.unitPrice,
                amount: draft.amount,
                currency: draft.currency || parseConfig.defaultCurrency,
                fee: draft.fee,
                isDraft: true,
                isValid: draft.status === "valid" || draft.status === "warning",
                lineNumber: draft.rowIndex + 1,
                comment: draft.comment,
                fxRate: draft.fxRate,
                subtype: draft.subtype,
              }) satisfies Partial<ActivityImport>,
          ) as ActivityImport[];

        let updatedDrafts = drafts;
        if (activitiesToValidate.length > 0) {
          const validated = await checkActivitiesImport({
            accountId,
            activities: activitiesToValidate,
          });

          if (import.meta.env.DEV) {
            const symbolFailures = validated
              .filter((v) => {
                const errs = v.errors || {};
                return Boolean(errs.symbol && errs.symbol.length > 0);
              })
              .map((v) => ({
                lineNumber: v.lineNumber,
                symbol: v.symbol,
                exchangeMic: v.exchangeMic,
                instrumentType: v.instrumentType,
                quoteMode: v.quoteMode,
                errors: v.errors?.symbol,
              }));

            if (symbolFailures.length > 0) {
              logger.warn(
                "[ImportReview] backend symbol failures",
                JSON.stringify(symbolFailures.slice(0, 20)),
              );
            }
          }

          if (validationRun !== validationRunRef.current) {
            return;
          }

          updatedDrafts = drafts.map((draft) => {
            const backendResult = validated.find((v) => v.lineNumber === draft.rowIndex + 1);
            if (!backendResult) {
              return {
                ...draft,
                accountId: draft.accountId || accountId,
                duplicateOfId: undefined,
                duplicateOfLineNumber: undefined,
              };
            }

            const backendErrors: Record<string, string[]> = {};
            if (backendResult.errors) {
              for (const [key, value] of Object.entries(backendResult.errors)) {
                backendErrors[key] = Array.isArray(value) ? value : [String(value)];
              }
            }
            const backendWarnings: Record<string, string[]> = {};
            if (backendResult.warnings) {
              for (const [key, value] of Object.entries(backendResult.warnings)) {
                backendWarnings[key] = Array.isArray(value) ? value : [String(value)];
              }
            }
            if (!backendResult.isValid && Object.keys(backendErrors).length === 0) {
              backendErrors.general = ["Validation failed"];
            }

            const localValidation = validateDraft(draft);
            const mergedErrors = mergeIssueMaps(localValidation.errors, backendErrors);
            const retainedWarnings = { ...localValidation.warnings };
            delete retainedWarnings._duplicate;
            const mergedWarnings = mergeIssueMaps(retainedWarnings, backendWarnings);
            const hasErrors = Object.keys(mergedErrors).length > 0;
            const hasWarnings = Object.keys(mergedWarnings).length > 0;

            return {
              ...draft,
              accountId: draft.accountId || accountId,
              errors: mergedErrors,
              warnings: mergedWarnings,
              duplicateOfId: backendResult.duplicateOfId,
              duplicateOfLineNumber: backendResult.duplicateOfLineNumber,
              symbolName: backendResult.symbolName,
              exchangeMic: backendResult.exchangeMic,
              quoteCcy: backendResult.quoteCcy,
              instrumentType: backendResult.instrumentType,
              status:
                draft.status === "skipped"
                  ? draft.status
                  : hasErrors
                    ? "error"
                    : hasWarnings
                      ? "warning"
                      : "valid",
            } as DraftActivity;
          });
        }
        if (validationRun === validationRunRef.current) {
          dispatch(setDraftActivities(updatedDrafts));
        }
      } catch (error) {
        logger.error(`Backend validation failed: ${error}`);
        if (validationRun === validationRunRef.current) {
          dispatch(setDraftActivities(drafts));
        }
      } finally {
        if (validationRun === validationRunRef.current) {
          setIsValidating(false);
        }
      }
    },
    [accountId, dispatch, parseConfig.defaultCurrency],
  );

  // Calculate filter stats
  const filterStats = useMemo<FilterStats>(() => {
    const stats: FilterStats = {
      all: draftActivities.length,
      errors: 0,
      warnings: 0,
      duplicates: 0,
      skipped: 0,
      valid: 0,
    };

    for (const draft of draftActivities) {
      switch (draft.status) {
        case "error":
          stats.errors++;
          break;
        case "warning":
          stats.warnings++;
          if (hasDuplicateWarning(draft)) {
            stats.duplicates++;
          }
          break;
        case "duplicate":
          stats.warnings++;
          stats.duplicates++;
          break;
        case "skipped":
          stats.skipped++;
          break;
        case "valid":
          stats.valid++;
          if (hasDuplicateWarning(draft)) {
            stats.duplicates++;
          }
          break;
      }
    }

    return stats;
  }, [draftActivities]);

  // Handlers
  const handleDraftUpdate = useCallback(
    (rowIndex: number, updates: Partial<DraftActivity>) => {
      // Find the current draft and merge with updates
      const currentDraft = draftActivities.find((d) => d.rowIndex === rowIndex);
      if (!currentDraft) {
        dispatch(updateDraft(rowIndex, updates));
        return;
      }

      const symbolChanged =
        Object.prototype.hasOwnProperty.call(updates, "symbol") &&
        updates.symbol !== currentDraft.symbol;
      const sanitizedUpdates: Partial<DraftActivity> = symbolChanged
        ? {
            symbolName: undefined,
            exchangeMic: undefined,
            quoteCcy: undefined,
            instrumentType: undefined,
            quoteMode: undefined,
            ...updates,
          }
        : updates;

      // When a symbol is corrected, propagate the fix to every other row that shares
      // the same original symbol so the user only has to map each ticker once. The
      // correction is also recorded in symbolMappings for future imports.
      const originalSymbol = currentDraft.symbol;
      const newSymbol = updates.symbol;
      const shouldPropagate =
        symbolChanged &&
        !!originalSymbol &&
        !!newSymbol &&
        draftActivities.some((d) => d.rowIndex !== rowIndex && d.symbol === originalSymbol);

      if (shouldPropagate) {
        const nextDrafts = draftActivities.map((draft) => {
          if (draft.symbol !== originalSymbol || draft.status === "skipped") return draft;
          const mergedDraft = { ...draft, ...sanitizedUpdates, symbol: newSymbol };
          const validation = validateDraft(mergedDraft);
          return {
            ...mergedDraft,
            status: validation.status,
            errors: validation.errors,
            warnings: validation.warnings,
            duplicateOfId: undefined,
            duplicateOfLineNumber: undefined,
          } as DraftActivity;
        });
        dispatch(setDraftActivities(nextDrafts));
        void validateDraftsWithBackend(nextDrafts);

        if (mapping) {
          const updatedMapping = {
            ...mapping,
            symbolMappings: { ...mapping.symbolMappings, [originalSymbol]: newSymbol },
          };
          dispatch(setMapping(updatedMapping));
          if (accountId) {
            saveAccountImportMapping({ ...updatedMapping, accountId }).catch((err) =>
              logger.error(`Failed to save symbol mapping: ${err}`),
            );
          }
        }
        return;
      }

      const mergedDraft = { ...currentDraft, ...sanitizedUpdates };
      // Re-validate the merged draft
      const validation = validateDraft(mergedDraft);
      // Don't override status if it was explicitly skipped.
      const shouldRevalidateStatus = currentDraft.status !== "skipped";
      dispatch(
        updateDraft(rowIndex, {
          ...sanitizedUpdates,
          ...(shouldRevalidateStatus
            ? {
                status: validation.status,
                errors: validation.errors,
                warnings: validation.warnings,
                duplicateOfId: undefined,
                duplicateOfLineNumber: undefined,
              }
            : {}),
        }),
      );
    },
    [dispatch, draftActivities, mapping, accountId, validateDraftsWithBackend],
  );

  const handleBulkSkip = useCallback(
    (rowIndexes: number[]) => {
      dispatch(bulkSkipDrafts(rowIndexes, "Skipped by user"));
      setSelectedRows([]);
    },
    [dispatch],
  );

  const handleBulkUnskip = useCallback(
    (rowIndexes: number[]) => {
      dispatch(bulkUnskipDrafts(rowIndexes));
      setSelectedRows([]);
    },
    [dispatch],
  );

  const handleBulkSetCurrency = useCallback(
    (rowIndexes: number[], currency: string) => {
      dispatch(bulkSetCurrency(rowIndexes, currency));
    },
    [dispatch],
  );

  const handleBulkSetAccount = useCallback(
    (rowIndexes: number[], newAccountId: string) => {
      dispatch(bulkSetAccount(rowIndexes, newAccountId));
    },
    [dispatch],
  );

  const handleSymbolResolution = useCallback(
    (mappings: Record<string, SymbolSearchResult>) => {
      // 1. Update all affected drafts in-memory, then run backend validation+dedupe again.
      const nextDrafts = draftActivities.map((draft) => {
        const result = draft.symbol ? mappings[draft.symbol] : undefined;
        if (!result || !draft.errors.symbol) {
          return draft;
        }

        const symbolUpdates: Partial<DraftActivity> = {
          symbol: result.symbol,
          exchangeMic: result.exchangeMic,
          symbolName: result.longName,
          quoteCcy: result.currency,
          instrumentType: result.quoteType,
          quoteMode: result.dataSource === "MANUAL" ? "MANUAL" : undefined,
        };
        const { symbol: _removed, ...otherErrors } = draft.errors;
        const merged = { ...draft, ...symbolUpdates };
        const validation = validateDraft(merged);
        const finalErrors = { ...otherErrors, ...validation.errors };
        const hasErrors = Object.keys(finalErrors).length > 0;
        const hasWarnings = Object.keys(validation.warnings).length > 0;

        return {
          ...merged,
          errors: finalErrors,
          warnings: validation.warnings,
          duplicateOfId: undefined,
          duplicateOfLineNumber: undefined,
          status:
            draft.status === "skipped"
              ? draft.status
              : hasErrors
                ? "error"
                : hasWarnings
                  ? "warning"
                  : "valid",
        } as DraftActivity;
      });
      dispatch(setDraftActivities(nextDrafts));
      void validateDraftsWithBackend(nextDrafts);

      // 2. Save resolved symbols to mapping profile for future imports
      if (mapping) {
        const newSymbolMappings = { ...mapping.symbolMappings };
        const newSymbolMappingMeta = { ...(mapping.symbolMappingMeta || {}) };

        for (const [csvSymbol, result] of Object.entries(mappings)) {
          newSymbolMappings[csvSymbol] = result.symbol;
          newSymbolMappingMeta[csvSymbol] = {
            exchangeMic: result.exchangeMic,
            symbolName: result.longName,
            quoteCcy: result.currency,
            instrumentType: result.quoteType,
            quoteMode: result.dataSource === "MANUAL" ? "MANUAL" : undefined,
          };
        }

        const updatedMapping = {
          ...mapping,
          symbolMappings: newSymbolMappings,
          symbolMappingMeta: newSymbolMappingMeta,
        };

        dispatch(setMapping(updatedMapping));

        // Persist to backend
        if (accountId) {
          saveAccountImportMapping({ ...updatedMapping, accountId }).catch((err) =>
            logger.error(`Failed to save symbol mappings: ${err}`),
          );
        }
      }
    },
    [draftActivities, dispatch, mapping, accountId, validateDraftsWithBackend],
  );

  const unresolvedSymbols = useMemo<UnresolvedSymbol[]>(() => {
    const symbolMap = new Map<string, number>();
    for (const draft of draftActivities) {
      if (draft.errors.symbol && draft.symbol) {
        symbolMap.set(draft.symbol, (symbolMap.get(draft.symbol) || 0) + 1);
      }
    }
    return Array.from(symbolMap.entries())
      .map(([csvSymbol, count]) => ({ csvSymbol, affectedCount: count }))
      .sort((a, b) => (b.affectedCount ?? 0) - (a.affectedCount ?? 0));
  }, [draftActivities]);

  useEffect(() => {
    if (!import.meta.env.DEV) return;
    if (!mapping || unresolvedSymbols.length === 0) return;

    const mappedPairs = Object.entries(mapping.symbolMappings || {}).slice(0, 12);
    const unresolvedPreview = unresolvedSymbols.slice(0, 12).map((s) => s.csvSymbol);

    logger.warn(
      "[ImportReview] unresolved symbols remain",
      JSON.stringify({
        unresolvedCount: unresolvedSymbols.length,
        unresolvedPreview,
        symbolMappingsCount: Object.keys(mapping.symbolMappings || {}).length,
        symbolMappingMetaCount: Object.keys(mapping.symbolMappingMeta || {}).length,
        mappedPairs,
      }),
    );
  }, [mapping, unresolvedSymbols]);

  const { data: accountHoldings = [], isLoading: isHoldingsLoading } = useQuery({
    queryKey: [QueryKeys.HOLDINGS, accountId],
    queryFn: () => getHoldings(accountId),
    enabled: !!accountId,
  });

  const { data: assets = [], isLoading: isAssetsLoading } = useQuery({
    queryKey: [QueryKeys.ASSETS],
    queryFn: () => getAssets(),
  });

  const assetsById = useMemo(() => {
    return new Map(assets.map((asset) => [asset.id, asset]));
  }, [assets]);

  const preferredSymbolResults = useMemo<SymbolSearchResult[]>(() => {
    const bySymbol = new Map<string, SymbolSearchResult>();

    // 1) Prefer symbols currently held in the selected account.
    for (const holding of accountHoldings) {
      const symbol = holding.instrument?.symbol?.trim();
      if (!symbol || bySymbol.has(symbol)) continue;

      const instrumentName = holding.instrument?.name ?? symbol;
      const existingAssetId = holding.instrument?.id;
      const asset = existingAssetId ? assetsById.get(existingAssetId) : undefined;
      const instrumentType = asset?.instrumentType ?? "EQUITY";

      bySymbol.set(symbol, {
        symbol,
        exchange: "PORTFOLIO",
        exchangeName: "Portfolio",
        exchangeMic: asset?.instrumentExchangeMic ?? undefined,
        shortName: instrumentName,
        longName: instrumentName,
        quoteType: instrumentType,
        index: "portfolio",
        score: 2000,
        typeDisplay: instrumentType,
        currency: holding.instrument?.currency ?? asset?.quoteCcy,
        dataSource: asset?.quoteMode,
        existingAssetId,
      });
    }

    // 2) Add symbols from all known assets in the portfolio (cross-account fallback).
    for (const asset of assets) {
      const symbol = asset.instrumentSymbol?.trim();
      if (!symbol) continue;
      const instrumentType = asset.instrumentType ?? "EQUITY";
      const name = asset.name ?? symbol;

      const assetPayload: SymbolSearchResult = {
        symbol,
        exchange: "PORTFOLIO",
        exchangeName: "Portfolio",
        exchangeMic: asset.instrumentExchangeMic ?? undefined,
        shortName: name,
        longName: name,
        quoteType: instrumentType,
        index: "portfolio",
        score: 1000,
        typeDisplay: instrumentType,
        currency: asset.quoteCcy,
        dataSource: asset.quoteMode,
        existingAssetId: asset.id,
      };

      const existing = bySymbol.get(symbol);
      if (!existing) {
        bySymbol.set(symbol, assetPayload);
        continue;
      }

      // Preserve account-holdings priority while filling missing identity metadata from assets.
      bySymbol.set(symbol, {
        ...existing,
        exchangeMic: existing.exchangeMic ?? assetPayload.exchangeMic,
        shortName: existing.shortName || assetPayload.shortName,
        longName: existing.longName || assetPayload.longName,
        quoteType: existing.quoteType || assetPayload.quoteType,
        typeDisplay: existing.typeDisplay || assetPayload.typeDisplay,
        currency: existing.currency || assetPayload.currency,
        dataSource: existing.dataSource || assetPayload.dataSource,
        existingAssetId: existing.existingAssetId || assetPayload.existingAssetId,
      });
    }

    return Array.from(bySymbol.values()).sort((a, b) => a.symbol.localeCompare(b.symbol));
  }, [accountHoldings, assets, assetsById]);

  const exactPortfolioSymbolMatches = useMemo<
    Record<
      string,
      {
        symbol: string;
        exchangeMic?: string;
        symbolName?: string;
        quoteCcy?: string;
        instrumentType?: string;
        quoteMode?: string;
      }
    >
  >(() => {
    const normalizeSymbolKey = (value: string): string =>
      value
        .trim()
        .toUpperCase()
        .replace(/\s+/g, "")
        .replace(/[^A-Z0-9]/g, "");

    const baseSymbolKey = (value: string): string => {
      const trimmed = value.trim().toUpperCase();
      const dotIndex = trimmed.indexOf(".");
      if (dotIndex > 0) {
        return trimmed.slice(0, dotIndex);
      }
      return "";
    };

    const matches: Record<
      string,
      {
        symbol: string;
        exchangeMic?: string;
        symbolName?: string;
        quoteCcy?: string;
        instrumentType?: string;
        quoteMode?: string;
      }
    > = {};

    const chooseRicherPayload = (
      existing:
        | {
            symbol: string;
            exchangeMic?: string;
            symbolName?: string;
            quoteCcy?: string;
            instrumentType?: string;
            quoteMode?: string;
          }
        | undefined,
      candidate: {
        symbol: string;
        exchangeMic?: string;
        symbolName?: string;
        quoteCcy?: string;
        instrumentType?: string;
        quoteMode?: string;
      },
    ) => {
      if (!existing) return candidate;

      const score = (value: {
        symbol: string;
        exchangeMic?: string;
        symbolName?: string;
        quoteCcy?: string;
        instrumentType?: string;
        quoteMode?: string;
      }) =>
        (value.exchangeMic ? 8 : 0) +
        (value.instrumentType ? 4 : 0) +
        (value.quoteCcy ? 2 : 0) +
        (value.symbolName ? 1 : 0);

      const existingScore = score(existing);
      const candidateScore = score(candidate);

      if (candidateScore > existingScore) return candidate;

      return {
        ...existing,
        exchangeMic: existing.exchangeMic ?? candidate.exchangeMic,
        symbolName: existing.symbolName ?? candidate.symbolName,
        quoteCcy: existing.quoteCcy ?? candidate.quoteCcy,
        instrumentType: existing.instrumentType ?? candidate.instrumentType,
        quoteMode: existing.quoteMode ?? candidate.quoteMode,
      };
    };
    const baseAliasCandidates: Record<
      string,
      {
        symbol: string;
        exchangeMic?: string;
        symbolName?: string;
        quoteCcy?: string;
        instrumentType?: string;
        quoteMode?: string;
      }[]
    > = {};

    for (const result of preferredSymbolResults) {
      const rawKey = result.symbol.trim().toUpperCase();
      if (!rawKey) continue;

      const normalizedKey = normalizeSymbolKey(rawKey);
      const payload = {
        symbol: result.symbol,
        exchangeMic: result.exchangeMic,
        symbolName: result.longName,
        quoteCcy: result.currency,
        instrumentType: result.quoteType,
        quoteMode: result.dataSource === "MANUAL" ? "MANUAL" : undefined,
      };

      if (!matches[rawKey]) {
        matches[rawKey] = payload;
      } else {
        matches[rawKey] = chooseRicherPayload(matches[rawKey], payload);
      }

      if (normalizedKey) {
        matches[normalizedKey] = chooseRicherPayload(matches[normalizedKey], payload);
      }

      const baseKey = baseSymbolKey(rawKey);
      if (baseKey) {
        if (!baseAliasCandidates[baseKey]) {
          baseAliasCandidates[baseKey] = [];
        }
        baseAliasCandidates[baseKey].push(payload);
      }
    }

    // Add base-symbol aliases only when unique to avoid ambiguous mappings
    // (e.g. MC -> MC.PA, SAP -> SAP.FRK)
    for (const [baseKey, candidates] of Object.entries(baseAliasCandidates)) {
      if (candidates.length !== 1) continue;
      matches[baseKey] = chooseRicherPayload(matches[baseKey], candidates[0]);
    }

    return matches;
  }, [preferredSymbolResults]);

  const draftBuildSignature = useMemo(
    () =>
      JSON.stringify({
        accountId,
        parseConfig: {
          dateFormat: parseConfig.dateFormat,
          decimalSeparator: parseConfig.decimalSeparator,
          thousandsSeparator: parseConfig.thousandsSeparator,
          defaultCurrency: parseConfig.defaultCurrency,
        },
        fieldMappings: mapping?.fieldMappings || {},
        activityMappings: mapping?.activityMappings || {},
        symbolMappings: mapping?.symbolMappings || {},
        accountMappings: mapping?.accountMappings || {},
        symbolMappingMeta: mapping?.symbolMappingMeta || {},
        headerCount: headers.length,
        rowCount: parsedRows.length,
      }),
    [accountId, parseConfig, mapping, headers.length, parsedRows.length],
  );

  const lastDraftBuildSignatureRef = useRef<string | null>(null);

  // Create draft activities and validate with backend when entering this step.
  // Wait for portfolio data to load so symbol auto-matching is populated.
  useEffect(() => {
    if (!parsedRows.length || !mapping || isHoldingsLoading || isAssetsLoading) {
      return;
    }

    // Keep user edits intact once they started manually editing rows in review.
    if (draftActivities.some((draft) => draft.isEdited)) {
      return;
    }

    const hasBuiltBefore = lastDraftBuildSignatureRef.current !== null;
    const hasSameSignature = lastDraftBuildSignatureRef.current === draftBuildSignature;
    if (hasBuiltBefore && hasSameSignature && draftActivities.length > 0) {
      return;
    }

    lastDraftBuildSignatureRef.current = draftBuildSignature;

      const drafts = createDraftActivities(
        parsedRows,
        headers,
        {
          fieldMappings: mapping.fieldMappings,
          activityMappings: mapping.activityMappings,
          symbolMappings: mapping.symbolMappings,
          accountMappings: mapping.accountMappings || {},
          symbolMappingMeta: mapping.symbolMappingMeta || {},
        },
        {
          dateFormat: parseConfig.dateFormat,
          decimalSeparator: parseConfig.decimalSeparator,
          thousandsSeparator: parseConfig.thousandsSeparator,
          defaultCurrency: parseConfig.defaultCurrency,
        },
        accountId,
        exactPortfolioSymbolMatches,
      );

      void validateDraftsWithBackend(drafts);
  }, [
    parsedRows,
    headers,
    mapping,
    parseConfig,
    accountId,
    draftActivities.length,
    draftActivities,
    draftBuildSignature,
    exactPortfolioSymbolMatches,
    isHoldingsLoading,
    isAssetsLoading,
    validateDraftsWithBackend,
  ]);

  // --- All hooks above this line ---

  // Show loading state while drafts are being created or validated
  if ((draftActivities.length === 0 && parsedRows.length > 0) || isValidating) {
    return (
      <div className="flex flex-col items-center justify-center py-12">
        <ProgressIndicator
          message={isValidating ? "Validating activities..." : "Processing activities..."}
          className="border-none shadow-none"
        />
      </div>
    );
  }

  // Show error if no data
  if (parsedRows.length === 0) {
    return (
      <ImportAlert
        variant="destructive"
        title="No Data"
        description="No CSV data available. Please go back and upload a file."
      />
    );
  }

  // Show error if no mapping
  if (!mapping || Object.keys(mapping.fieldMappings).length === 0) {
    return (
      <ImportAlert
        variant="warning"
        title="Missing Mapping"
        description="Column mappings are not configured. Please go back and configure the mapping."
      />
    );
  }

  const validCount = filterStats.valid + filterStats.warnings;
  const hasErrors = filterStats.errors > 0;
  const hasWarnings = filterStats.warnings > 0;
  const hasIssues = hasErrors || hasWarnings;

  return (
    <div className="flex flex-col gap-4">
      {/* Summary alert */}
      {hasIssues ? (
        <ImportAlert
          variant={hasErrors ? "destructive" : "warning"}
          title={`${validCount} of ${filterStats.all} activities ready to import`}
          description={`${filterStats.errors} errors, ${filterStats.warnings} warnings. Review and fix issues below, or skip problematic rows.`}
        />
      ) : (
        <ImportAlert
          variant="success"
          title={`All ${filterStats.all} activities are valid`}
          description="Your data is ready for import. You can still review and make adjustments if needed."
        />
      )}

      {/* Symbol resolution for unrecognized symbols */}
      <SymbolResolutionPanel
        unresolvedSymbols={unresolvedSymbols}
        preferredResults={preferredSymbolResults}
        onApplyMappings={handleSymbolResolution}
      />

      {/* Stats and filter */}
      <div className="flex flex-col gap-3">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <h2 className="text-base font-semibold">Review Activities</h2>
          <FilterStatsBar stats={filterStats} currentFilter={filter} onFilterChange={setFilter} />
        </div>
        <ImportReviewGrid
          drafts={draftActivities}
          onDraftUpdate={handleDraftUpdate}
          selectedRows={selectedRows}
          onSelectionChange={setSelectedRows}
          filter={filter}
          onBulkSkip={handleBulkSkip}
          onBulkUnskip={handleBulkUnskip}
          onBulkSetCurrency={handleBulkSetCurrency}
          onBulkSetAccount={handleBulkSetAccount}
        />
      </div>
    </div>
  );
}

export default ReviewStep;
