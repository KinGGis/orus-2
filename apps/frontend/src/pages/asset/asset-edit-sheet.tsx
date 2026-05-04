import { getExchanges } from "@/adapters";
import { MultiSelectTaxonomy } from "@/components/classification/multi-select-taxonomy";
import { SingleSelectTaxonomy } from "@/components/classification/single-select-taxonomy";
import { TickerAvatar } from "@/components/ticker-avatar";
import { useMarketDataProviders } from "@/hooks/use-market-data-providers";
import { useTaxonomies } from "@/hooks/use-taxonomies";
import type { Asset, Quote } from "@/lib/types";
import { formatAmount } from "@/lib/utils";
import { zodResolver } from "@hookform/resolvers/zod";
import { useQuery } from "@tanstack/react-query";
import {
  Alert,
  AlertDescription,
  CurrencyInput,
  ResponsiveSelect,
  type ResponsiveSelectOption,
  SearchableSelect,
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@wealthfolio/ui";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Button } from "@wealthfolio/ui/components/ui/button";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@wealthfolio/ui/components/ui/form";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Input } from "@wealthfolio/ui/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui/components/ui/select";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@wealthfolio/ui/components/ui/tabs";
import { Textarea } from "@wealthfolio/ui/components/ui/textarea";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useFieldArray, useForm } from "react-hook-form";
import * as z from "zod";
import { useAssetProfileMutations } from "./hooks/use-asset-profile-mutations";

const PROVIDERS = [
  { value: "YAHOO", label: "Yahoo Finance" },
  { value: "ALPHA_VANTAGE", label: "Alpha Vantage" },
  { value: "FINNHUB", label: "Finnhub" },
  { value: "MARKETDATA_APP", label: "MarketData.app" },
] as const;

// Schema for a single provider override (type is derived from asset kind)
const providerOverrideSchema = z.object({
  provider: z.string(),
  symbol: z.string(),
});

// Derive override type from asset kind
function getOverrideTypeForKind(kind: string): "equity_symbol" | "crypto_symbol" | "fx_symbol" {
  switch (kind) {
    case "FX":
      return "fx_symbol";
    default:
      return "equity_symbol";
  }
}

// QuoteMode values matching Rust enum
const QuoteMode = {
  MARKET: "MARKET",
  MANUAL: "MANUAL",
  INTERNAL_YTM: "INTERNAL_YTM",
} as const;

type QuoteMode = (typeof QuoteMode)[keyof typeof QuoteMode];

const assetFormSchema = z.object({
  name: z.string().optional(),
  notes: z.string().optional(),
  instrumentType: z.string().optional(),
  quoteCcy: z.string().min(1, "Currency is required"),
  instrumentExchangeMic: z.string().optional(),
  quoteMode: z.enum([QuoteMode.MARKET, QuoteMode.MANUAL, QuoteMode.INTERNAL_YTM]),
  preferredProvider: z.string().optional(),
  providerConfig: z.array(providerOverrideSchema).optional(),
  bond: z
    .object({
      maturityDate: z.string().optional(),
      couponRate: z.coerce.number().min(0).max(100).optional(),
      faceValue: z.coerce.number().positive().optional(),
      couponFrequency: z.string().optional(),
      isin: z.string().optional(),
      ytmSource: z.string().optional(),
      ytmFixedValue: z.coerce.number().min(0).max(100).optional(),
      ytmSpreadBps: z.coerce.number().optional(),
      dayCountConvention: z.string().optional(),
      settlementDays: z.coerce.number().int().min(0).optional(),
      pricingMethod: z.string().optional(),
      couponSchedule: z.string().optional(),
      firstCouponDate: z.string().optional(),
    })
    .optional(),
});

type AssetFormValues = z.infer<typeof assetFormSchema>;
type ProviderOverride = z.infer<typeof providerOverrideSchema>;

const normalizeMic = (mic?: string | null): string => mic?.trim().toUpperCase() ?? "";

const EDIT_INSTRUMENT_TYPE_OPTIONS = [
  { value: "EQUITY", label: "Equity (Stock, ETF, Fund)" },
  { value: "CRYPTO", label: "Cryptocurrency" },
  { value: "BOND", label: "Bond" },
  { value: "OPTION", label: "Option" },
  { value: "METAL", label: "Precious Metal" },
] as const;

// Parse provider overrides from config JSON (supports nested and flat formats)
function parseProviderOverrides(
  config: Record<string, unknown> | null | undefined,
): ProviderOverride[] {
  if (!config) return [];
  // Nested format: { overrides: { YAHOO: { symbol: "..." } } }
  // Flat format (legacy): { YAHOO: { symbol: "..." } }
  const source = (config.overrides as Record<string, unknown> | undefined) ?? config;
  const result: ProviderOverride[] = [];
  for (const [provider, value] of Object.entries(source)) {
    if (typeof value === "object" && value !== null) {
      const obj = value as Record<string, unknown>;
      const symbol = obj.symbol as string;
      if (symbol) {
        result.push({ provider, symbol });
      }
    }
  }
  return result;
}

// Extract preferred_provider from config JSON
function parsePreferredProvider(
  config: Record<string, unknown> | null | undefined,
): string | undefined {
  if (!config) return undefined;
  const pref = config.preferred_provider;
  return typeof pref === "string" ? pref : undefined;
}

// Parse BondSpec from asset.metadata.bond (values stored as decimals, displayed as %)
function parseBondSpec(bondData: unknown): AssetFormValues["bond"] {
  if (!bondData || typeof bondData !== "object") return undefined;
  const b = bondData as Record<string, unknown>;
  const toDisplayPct = (v: unknown) =>
    typeof v === "number" ? Math.round(v * 100 * 1e8) / 1e8 : undefined;
  return {
    maturityDate: typeof b.maturityDate === "string" ? b.maturityDate : undefined,
    couponRate: toDisplayPct(b.couponRate),
    faceValue: typeof b.faceValue === "number" ? b.faceValue : undefined,
    couponFrequency: typeof b.couponFrequency === "string" ? b.couponFrequency : undefined,
    isin: typeof b.isin === "string" ? b.isin : undefined,
    ytmSource: typeof b.ytmSource === "string" ? b.ytmSource : undefined,
    ytmFixedValue: toDisplayPct(b.ytmFixedValue),
    ytmSpreadBps: typeof b.ytmSpreadBps === "number" ? b.ytmSpreadBps : undefined,
    dayCountConvention:
      typeof b.dayCountConvention === "string" ? b.dayCountConvention : undefined,
    settlementDays: typeof b.settlementDays === "number" ? b.settlementDays : undefined,
    pricingMethod: typeof b.pricingMethod === "string" ? b.pricingMethod : undefined,
    couponSchedule: typeof b.couponSchedule === "string" ? b.couponSchedule : undefined,
    firstCouponDate: typeof b.firstCouponDate === "string" ? b.firstCouponDate : undefined,
  };
}

// Serialize bond form values back to metadata.bond (convert display % to decimal)
function serializeBondSpec(bond: NonNullable<AssetFormValues["bond"]>): Record<string, unknown> {
  const toDecimal = (v: number | undefined) => (v != null ? v / 100 : null);
  // If a first_coupon_date is provided, derive the schedule from it so the
  // pricing engine can run even when the user never manually fills the
  // comma-separated schedule field.
  const derivedSchedule = generateCouponSchedule(
    bond.firstCouponDate ?? null,
    bond.couponFrequency ?? null,
    bond.maturityDate ?? null,
  );
  const couponSchedule =
    bond.couponSchedule && bond.couponSchedule.trim().length > 0
      ? bond.couponSchedule
      : derivedSchedule.length > 0
        ? derivedSchedule.join(",")
        : null;
  return {
    maturityDate: bond.maturityDate ?? null,
    couponRate: toDecimal(bond.couponRate),
    faceValue: bond.faceValue ?? null,
    couponFrequency: bond.couponFrequency ?? null,
    isin: bond.isin ?? null,
    ytmSource: bond.ytmSource ?? null,
    ytmFixedValue: toDecimal(bond.ytmFixedValue),
    ytmSpreadBps: bond.ytmSpreadBps ?? null,
    dayCountConvention: bond.dayCountConvention ?? null,
    settlementDays: bond.settlementDays ?? null,
    pricingMethod: bond.pricingMethod ?? null,
    couponSchedule,
    firstCouponDate: bond.firstCouponDate ?? null,
  };
}

/**
 * Derive coupon-payment dates from `(firstCouponDate, frequency, maturityDate)`.
 * Returns an empty array if any input is missing or unparseable. The day of
 * month is taken from `firstCouponDate`; months that have fewer days clamp
 * the day to the month's last day. The maturity date is appended as the
 * final coupon anchor when it falls strictly after the last derived date.
 */
export function generateCouponSchedule(
  firstCouponDateStr: string | null | undefined,
  frequency: string | null | undefined,
  maturityDateStr: string | null | undefined,
): string[] {
  if (!firstCouponDateStr || !frequency || !maturityDateStr) return [];
  const firstDate = parseISODate(firstCouponDateStr);
  const maturityDate = parseISODate(maturityDateStr);
  if (!firstDate || !maturityDate) return [];
  const monthsPerPeriod: Record<string, number> = {
    ANNUAL: 12,
    SEMI_ANNUAL: 6,
    QUARTERLY: 3,
    MONTHLY: 1,
  };
  const months = monthsPerPeriod[frequency.toUpperCase()];
  if (!months) return [];

  const out: string[] = [];
  let i = 0;
  while (true) {
    const d = addMonthsClamped(firstDate, i * months);
    if (d > maturityDate) break;
    out.push(formatISODate(d));
    i += 1;
    // Hard guard against runaway loops
    if (out.length > 2000) break;
  }
  // Ensure maturity date is included as the final cash-flow anchor
  const lastIso = out[out.length - 1];
  const maturityIso = formatISODate(maturityDate);
  if (lastIso !== maturityIso) {
    out.push(maturityIso);
  }
  return out;
}

function parseISODate(s: string): Date | null {
  // Use UTC to avoid local timezone shifting the day
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(s);
  if (!m) return null;
  const year = Number(m[1]);
  const month = Number(m[2]);
  const day = Number(m[3]);
  const d = new Date(Date.UTC(year, month - 1, day));
  if (Number.isNaN(d.getTime())) return null;
  return d;
}

function formatISODate(d: Date): string {
  const y = d.getUTCFullYear();
  const m = String(d.getUTCMonth() + 1).padStart(2, "0");
  const day = String(d.getUTCDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function addMonthsClamped(date: Date, months: number): Date {
  const targetMonth = date.getUTCMonth() + months;
  const year = date.getUTCFullYear() + Math.floor(targetMonth / 12);
  const monthIdx = ((targetMonth % 12) + 12) % 12;
  // Last day of target month
  const lastDay = new Date(Date.UTC(year, monthIdx + 1, 0)).getUTCDate();
  const day = Math.min(date.getUTCDate(), lastDay);
  return new Date(Date.UTC(year, monthIdx, day));
}

// Serialize form values to nested provider config JSON
function serializeProviderConfig(
  preferredProvider: string | undefined,
  overrides: ProviderOverride[],
  assetKind: string,
): Record<string, unknown> | null {
  const overrideType = getOverrideTypeForKind(assetKind);
  const overridesMap: Record<string, unknown> = {};
  for (const override of overrides ?? []) {
    if (override.provider && override.symbol) {
      overridesMap[override.provider] = {
        type: overrideType,
        symbol: override.symbol,
      };
    }
  }
  const hasOverrides = Object.keys(overridesMap).length > 0;
  const hasPref = !!preferredProvider;
  if (!hasOverrides && !hasPref) return null;
  const result: Record<string, unknown> = {};
  if (hasPref) result.preferred_provider = preferredProvider;
  if (hasOverrides) result.overrides = overridesMap;
  return result;
}

type EditTab = "general" | "classification" | "market-data" | "fx-settings";

interface AssetEditSheetProps {
  asset: Asset | null;
  latestQuote?: Quote | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  defaultTab?: EditTab;
}

export function AssetEditSheet({
  asset,
  latestQuote,
  open,
  onOpenChange,
  defaultTab = "general",
}: AssetEditSheetProps) {
  const [activeTab, setActiveTab] = useState<EditTab>(defaultTab);
  const { data: taxonomies = [], isLoading: isTaxonomiesLoading } = useTaxonomies();
  const { updateAssetProfileMutation } = useAssetProfileMutations();
  const { data: marketDataProviders = [] } = useMarketDataProviders();

  const providerOptions: ResponsiveSelectOption[] = useMemo(() => {
    return [
      { value: "__auto__", label: "Auto (default)" },
      ...marketDataProviders.map((p) => ({ value: p.id, label: p.name })),
    ];
  }, [marketDataProviders]);

  const { data: exchanges = [] } = useQuery({
    queryKey: ["exchanges"],
    queryFn: getExchanges,
    staleTime: Infinity,
  });

  const currentMic = normalizeMic(asset?.instrumentExchangeMic);

  const exchangeOptions = useMemo(() => {
    const options = exchanges.map((e) => ({
      value: normalizeMic(e.mic),
      label: `${e.longName} (${e.name})`,
    }));

    if (currentMic && !options.some((option) => option.value === currentMic)) {
      options.unshift({
        value: currentMic,
        label: asset?.exchangeName ? `${asset.exchangeName} (${currentMic})` : currentMic,
      });
    }

    return options;
  }, [exchanges, currentMic, asset?.exchangeName]);

  // Split taxonomies by selection type
  const { singleSelectTaxonomies, multiSelectTaxonomies } = useMemo(() => {
    const sorted = [...taxonomies].sort((a, b) => a.sortOrder - b.sortOrder);
    return {
      singleSelectTaxonomies: sorted.filter((t) => t.isSingleSelect),
      multiSelectTaxonomies: sorted.filter((t) => !t.isSingleSelect),
    };
  }, [taxonomies]);

  const form = useForm<AssetFormValues>({
    resolver: zodResolver(assetFormSchema),
    defaultValues: {
      name: asset?.name ?? "",
      notes: asset?.notes ?? "",
      instrumentType: asset?.instrumentType ?? "",
      quoteCcy: asset?.quoteCcy ?? "",
      instrumentExchangeMic: normalizeMic(asset?.instrumentExchangeMic),
      quoteMode:
        asset?.quoteMode === "MANUAL"
          ? QuoteMode.MANUAL
          : asset?.quoteMode === "INTERNAL_YTM"
            ? QuoteMode.INTERNAL_YTM
            : QuoteMode.MARKET,
      preferredProvider: parsePreferredProvider(
        asset?.providerConfig as Record<string, unknown> | null,
      ),
      providerConfig: parseProviderOverrides(
        asset?.providerConfig as Record<string, unknown> | null,
      ),
      bond: parseBondSpec(asset?.metadata?.bond),
    },
  });

  const {
    fields: overrideFields,
    append: appendOverride,
    remove: removeOverride,
  } = useFieldArray({
    control: form.control,
    name: "providerConfig",
  });

  // Reset form when asset changes
  useEffect(() => {
    if (asset) {
      form.reset({
        name: asset.name ?? "",
        notes: asset.notes ?? "",
        instrumentType: asset.instrumentType ?? "",
        quoteCcy: asset.quoteCcy ?? "",
        instrumentExchangeMic: normalizeMic(asset.instrumentExchangeMic),
        quoteMode:
          asset.quoteMode === "MANUAL"
            ? QuoteMode.MANUAL
            : asset.quoteMode === "INTERNAL_YTM"
              ? QuoteMode.INTERNAL_YTM
              : QuoteMode.MARKET,
        preferredProvider: parsePreferredProvider(
          asset.providerConfig as Record<string, unknown> | null,
        ),
        providerConfig: parseProviderOverrides(
          asset.providerConfig as Record<string, unknown> | null,
        ),
        bond: parseBondSpec(asset.metadata?.bond),
      });
    }
  }, [asset, form]);

  // Reset tab when sheet opens
  useEffect(() => {
    if (open) {
      setActiveTab(defaultTab);
    }
  }, [open, defaultTab]);

  // Auto-compute YTM (current yield at par) from coupon rate when the four
  // bond inputs (face value, coupon rate, frequency, maturity) are all set
  // and the user hasn't typed an explicit YTM value. At par, current yield
  // = coupon rate, which is the only YTM derivable without a market price.
  const watchedBondCouponRate = form.watch("bond.couponRate");
  const watchedBondFaceValue = form.watch("bond.faceValue");
  const watchedBondFrequency = form.watch("bond.couponFrequency");
  const watchedBondMaturity = form.watch("bond.maturityDate");
  const watchedBondYtmSource = form.watch("bond.ytmSource");
  const watchedBondYtmFixed = form.watch("bond.ytmFixedValue");
  useEffect(() => {
    if (watchedBondYtmSource !== "FIXED") return;
    if (
      watchedBondCouponRate == null ||
      watchedBondFaceValue == null ||
      !watchedBondFrequency ||
      !watchedBondMaturity
    ) {
      return;
    }
    if (watchedBondYtmFixed == null) {
      form.setValue("bond.ytmFixedValue", watchedBondCouponRate, {
        shouldDirty: false,
        shouldTouch: false,
        shouldValidate: false,
      });
    }
  }, [
    form,
    watchedBondCouponRate,
    watchedBondFaceValue,
    watchedBondFrequency,
    watchedBondMaturity,
    watchedBondYtmSource,
    watchedBondYtmFixed,
  ]);

  const handleSave = useCallback(
    async (values: AssetFormValues) => {
      if (!asset) return;

      // Serialize provider config to nested JSON format
      const serializedOverrides = serializeProviderConfig(
        values.preferredProvider,
        values.providerConfig ?? [],
        asset.kind ?? "INVESTMENT",
      );
      const normalizedMic = normalizeMic(values.instrumentExchangeMic);

      // Build metadata: merge existing metadata, update bond section if INTERNAL_YTM
      const updatedMetadata: Record<string, unknown> = { ...(asset.metadata ?? {}) };
      if (values.quoteMode === QuoteMode.INTERNAL_YTM && values.bond) {
        updatedMetadata.bond = serializeBondSpec(values.bond);
      }

      try {
        // Update profile with all fields including quote mode
        await updateAssetProfileMutation.mutateAsync({
          id: asset.id,
          displayCode: asset.displayCode,
          name: values.name || "",
          notes: values.notes ?? "",
          instrumentType: values.instrumentType || null,
          quoteMode: values.quoteMode,
          quoteCcy: values.quoteCcy,
          instrumentExchangeMic: normalizedMic || null,
          providerConfig: serializedOverrides,
          metadata: updatedMetadata,
        });

        onOpenChange(false);
      } catch {
        // Error toast is shown by mutation's onError callback
        // Keep sheet open so user can retry
      }
    },
    [asset, updateAssetProfileMutation, onOpenChange],
  );

  const selectedQuoteMode = form.watch("quoteMode");
  const isMarketMode = selectedQuoteMode === QuoteMode.MARKET;
  const isInternalYtmMode = selectedQuoteMode === QuoteMode.INTERNAL_YTM;
  const watchedYtmSource = form.watch("bond.ytmSource");

  const quoteModeOptions: ResponsiveSelectOption[] = [
    { value: QuoteMode.MARKET, label: "Market (auto-sync)" },
    { value: QuoteMode.MANUAL, label: "Manual" },
    { value: QuoteMode.INTERNAL_YTM, label: "Internal YTM (bond model)" },
  ];
  const isSaving = updateAssetProfileMutation.isPending;

  // Check if current asset kind is system-managed (shouldn't allow editing)
  const isSystemManagedKind = asset?.kind === "FX";

  if (!asset) return null;

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="pb-safe flex h-full w-full flex-col sm:max-w-2xl">
        <SheetHeader className="shrink-0 pb-4">
          <div className="flex items-center gap-3">
            <TickerAvatar symbol={asset.displayCode ?? ""} className="size-10" />
            <div className="min-w-0 flex-1">
              <SheetTitle className="truncate text-lg">
                {asset.displayCode ?? asset.name ?? "Unknown"}
              </SheetTitle>
              <SheetDescription className="truncate text-sm">
                {asset.name || "Edit asset"}
              </SheetDescription>
            </div>
          </div>
        </SheetHeader>

        <Tabs
          value={activeTab}
          onValueChange={(v) => setActiveTab(v as EditTab)}
          className="flex min-h-0 flex-1 flex-col"
        >
          {asset.kind === "FX" ? (
            <TabsList className="grid w-full grid-cols-2">
              <TabsTrigger value="general">General</TabsTrigger>
              <TabsTrigger value="market-data">Market Data</TabsTrigger>
            </TabsList>
          ) : (
            <TabsList className="grid w-full grid-cols-3">
              <TabsTrigger value="general">General</TabsTrigger>
              <TabsTrigger value="classification">Classification</TabsTrigger>
              <TabsTrigger value="market-data">Market Data</TabsTrigger>
            </TabsList>
          )}

          <div className="min-h-0 flex-1 overflow-y-auto pt-4">
            {/* General Tab */}
            <TabsContent value="general" className="mt-0 h-full">
              <Form {...form}>
                <form onSubmit={form.handleSubmit(handleSave)} className="space-y-6">
                  {/* FX: Base and Quote Currency (both disabled) */}
                  {asset.kind === "FX" ? (
                    <div className="space-y-6">
                      <div className="grid gap-4 md:grid-cols-2">
                        <div className="space-y-2">
                          <label className="text-sm font-medium">Base Currency</label>
                          <Input
                            value={asset.instrumentSymbol ?? ""}
                            disabled
                            className="bg-muted/50"
                          />
                        </div>
                        <div className="space-y-2">
                          <label className="text-sm font-medium">Quote Currency</label>
                          <Input value={asset.quoteCcy ?? ""} disabled className="bg-muted/50" />
                        </div>
                      </div>

                      <FormField
                        control={form.control}
                        name="name"
                        render={({ field }) => (
                          <FormItem>
                            <FormLabel>Name</FormLabel>
                            <FormControl>
                              <Input placeholder="Asset display name" {...field} />
                            </FormControl>
                            <FormMessage />
                          </FormItem>
                        )}
                      />

                      <FormField
                        control={form.control}
                        name="notes"
                        render={({ field }) => (
                          <FormItem>
                            <FormLabel>Notes</FormLabel>
                            <FormControl>
                              <Textarea
                                rows={6}
                                placeholder="Add any context or links"
                                {...field}
                              />
                            </FormControl>
                            <FormMessage />
                          </FormItem>
                        )}
                      />

                      <div className="flex justify-end gap-3 pt-4">
                        <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                          Cancel
                        </Button>
                        <Button type="submit" disabled={isSaving}>
                          {isSaving ? "Saving..." : "Save Changes"}
                        </Button>
                      </div>
                    </div>
                  ) : (
                    /* Regular assets: Symbol, Currency, Name, Notes, Asset Type, Exchange */
                    <div className="space-y-6">
                      <div className="grid gap-4 md:grid-cols-2">
                        <div className="space-y-2">
                          <label className="text-sm font-medium">Symbol</label>
                          <Input value={asset.displayCode ?? ""} disabled className="bg-muted/50" />
                        </div>
                        <FormField
                          control={form.control}
                          name="quoteCcy"
                          render={({ field }) => (
                            <FormItem>
                              <FormLabel>Currency</FormLabel>
                              <FormControl>
                                <CurrencyInput
                                  value={field.value}
                                  onChange={field.onChange}
                                  placeholder="Select currency"
                                  valueDisplay="code"
                                  allowCustom
                                />
                              </FormControl>
                              <FormMessage />
                            </FormItem>
                          )}
                        />
                      </div>

                      {/* Editable fields */}
                      <FormField
                        control={form.control}
                        name="name"
                        render={({ field }) => (
                          <FormItem>
                            <FormLabel>Name</FormLabel>
                            <FormControl>
                              <Input placeholder="Asset display name" {...field} />
                            </FormControl>
                            <FormMessage />
                          </FormItem>
                        )}
                      />

                      <FormField
                        control={form.control}
                        name="notes"
                        render={({ field }) => (
                          <FormItem>
                            <FormLabel>Notes</FormLabel>
                            <FormControl>
                              <Textarea
                                rows={10}
                                placeholder="Add any context or links"
                                {...field}
                              />
                            </FormControl>
                            <FormMessage />
                          </FormItem>
                        )}
                      />

                      {/* Instrument Type and Exchange */}
                      <div className="grid gap-4 md:grid-cols-2">
                        <FormField
                          control={form.control}
                          name="instrumentType"
                          render={({ field }) => (
                            <FormItem>
                              <FormLabel>Instrument Type</FormLabel>
                              <Select
                                onValueChange={field.onChange}
                                value={field.value ?? ""}
                                disabled={isSystemManagedKind}
                              >
                                <FormControl>
                                  <SelectTrigger className="h-11">
                                    <SelectValue placeholder="Select type" />
                                  </SelectTrigger>
                                </FormControl>
                                <SelectContent>
                                  {EDIT_INSTRUMENT_TYPE_OPTIONS.map((option) => (
                                    <SelectItem key={option.value} value={option.value}>
                                      {option.label}
                                    </SelectItem>
                                  ))}
                                </SelectContent>
                              </Select>
                              <FormMessage />
                            </FormItem>
                          )}
                        />

                        <FormField
                          control={form.control}
                          name="instrumentExchangeMic"
                          render={({ field }) => (
                            <FormItem>
                              <FormLabel>Exchange</FormLabel>
                              <FormControl>
                                <SearchableSelect
                                  options={exchangeOptions}
                                  value={field.value ?? ""}
                                  onValueChange={field.onChange}
                                  placeholder="Select exchange"
                                  searchPlaceholder="Search exchanges..."
                                  className="h-11"
                                />
                              </FormControl>
                              <FormMessage />
                            </FormItem>
                          )}
                        />
                      </div>

                      <div className="flex justify-end gap-3 pt-4">
                        <Button
                          type="button"
                          variant="ghost"
                          onClick={() => onOpenChange(false)}
                          disabled={isSaving}
                        >
                          Cancel
                        </Button>
                        <Button type="submit" disabled={isSaving}>
                          {isSaving ? (
                            <span className="flex items-center gap-2">
                              <Icons.Spinner className="h-4 w-4 animate-spin" /> Saving
                            </span>
                          ) : (
                            "Save changes"
                          )}
                        </Button>
                      </div>
                    </div>
                  )}
                </form>
              </Form>
            </TabsContent>

            {/* Classification Tab */}
            <TabsContent value="classification" className="mt-0 h-full">
              <div className="space-y-8 pb-8">
                {isTaxonomiesLoading && <ClassificationSkeleton />}

                {!isTaxonomiesLoading &&
                  singleSelectTaxonomies.length === 0 &&
                  multiSelectTaxonomies.length === 0 && (
                    <div className="py-8 text-center">
                      <p className="text-muted-foreground text-sm">
                        No taxonomies configured. Create taxonomies in Settings to classify assets.
                      </p>
                    </div>
                  )}

                {!isTaxonomiesLoading &&
                  singleSelectTaxonomies.map((taxonomy) => (
                    <SingleSelectTaxonomy
                      key={taxonomy.id}
                      taxonomyId={taxonomy.id}
                      assetId={asset.id}
                      label={taxonomy.name}
                    />
                  ))}

                {!isTaxonomiesLoading &&
                  multiSelectTaxonomies.map((taxonomy) => (
                    <MultiSelectTaxonomy
                      key={taxonomy.id}
                      taxonomyId={taxonomy.id}
                      assetId={asset.id}
                      label={taxonomy.name}
                    />
                  ))}
              </div>
            </TabsContent>

            {/* Market Data Tab */}
            <TabsContent value="market-data" className="mt-0 h-full">
              <div className="space-y-6 pb-8">
                <Form {...form}>
                  <div className="space-y-6">
                    {/* Latest Quote Card - First */}
                    <div className="bg-muted/30 rounded-lg border p-4">
                      {latestQuote ? (
                        <div className="grid grid-cols-3 gap-4 text-center">
                          <div>
                            <p className="text-xl font-semibold">
                              {formatAmount(latestQuote.close, latestQuote.currency)}
                            </p>
                            <p className="text-muted-foreground text-xs">Latest price</p>
                          </div>
                          <div>
                            <p className="text-sm font-medium">
                              {new Date(latestQuote.timestamp).toLocaleDateString()}
                            </p>
                            <p className="text-muted-foreground text-xs">
                              {new Date(latestQuote.timestamp).toLocaleTimeString([], {
                                hour: "2-digit",
                                minute: "2-digit",
                              })}
                            </p>
                          </div>
                          <div>
                            <Badge variant="secondary" className="text-xs">
                              {latestQuote.dataSource}
                            </Badge>
                            <p className="text-muted-foreground mt-1 text-xs">Source</p>
                          </div>
                        </div>
                      ) : (
                        <Alert variant="destructive" className="border-0 bg-transparent p-0">
                          <Icons.AlertCircle className="h-4 w-4" />
                          <AlertDescription>
                            Unable to fetch price data for this asset. Check if the symbol is
                            correct or try adding a symbol mapping below.
                          </AlertDescription>
                        </Alert>
                      )}
                    </div>

                    {/* Pricing Mode Toggle Card */}
                    <FormField
                      control={form.control}
                      name="quoteMode"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>Pricing Mode</FormLabel>
                          <FormControl>
                            <ResponsiveSelect
                              value={field.value}
                              onValueChange={field.onChange}
                              options={quoteModeOptions}
                              placeholder="Select pricing mode"
                              sheetTitle="Pricing Mode"
                              sheetDescription="Choose how this asset gets its price."
                              triggerClassName="h-11"
                            />
                          </FormControl>
                        </FormItem>
                      )}
                    />

                    {/* Bond YTM Parameters - Only shown for INTERNAL_YTM mode */}
                    {isInternalYtmMode && (
                      <div className="space-y-4 rounded-lg border p-4">
                        <p className="text-sm font-medium">Bond Parameters</p>

                        {/* Basic bond info */}
                        <div className="grid gap-4 sm:grid-cols-2">
                          <FormField
                            control={form.control}
                            name="bond.maturityDate"
                            render={({ field }) => (
                              <FormItem>
                                <FormLabel>Maturity Date</FormLabel>
                                <FormControl>
                                  <Input type="date" {...field} value={field.value ?? ""} />
                                </FormControl>
                                <FormMessage />
                              </FormItem>
                            )}
                          />
                          <FormField
                            control={form.control}
                            name="bond.faceValue"
                            render={({ field }) => (
                              <FormItem>
                                <FormLabel>Face Value</FormLabel>
                                <FormControl>
                                  <Input
                                    type="number"
                                    placeholder="1000"
                                    {...field}
                                    value={field.value ?? ""}
                                  />
                                </FormControl>
                                <FormMessage />
                              </FormItem>
                            )}
                          />
                          <FormField
                            control={form.control}
                            name="bond.couponRate"
                            render={({ field }) => (
                              <FormItem>
                                <FormLabel>Coupon Rate (%)</FormLabel>
                                <FormControl>
                                  <Input
                                    type="number"
                                    step="0.001"
                                    placeholder="4.375"
                                    {...field}
                                    value={field.value ?? ""}
                                  />
                                </FormControl>
                                <FormMessage />
                              </FormItem>
                            )}
                          />
                          <FormField
                            control={form.control}
                            name="bond.couponFrequency"
                            render={({ field }) => (
                              <FormItem>
                                <FormLabel>Coupon Frequency</FormLabel>
                                <Select
                                  onValueChange={field.onChange}
                                  value={field.value ?? ""}
                                >
                                  <FormControl>
                                    <SelectTrigger className="h-9">
                                      <SelectValue placeholder="Select frequency" />
                                    </SelectTrigger>
                                  </FormControl>
                                  <SelectContent>
                                    <SelectItem value="ANNUAL">Annual</SelectItem>
                                    <SelectItem value="SEMI_ANNUAL">Semi-Annual</SelectItem>
                                    <SelectItem value="QUARTERLY">Quarterly</SelectItem>
                                    <SelectItem value="MONTHLY">Monthly</SelectItem>
                                  </SelectContent>
                                </Select>
                                <FormMessage />
                              </FormItem>
                            )}
                          />
                          <FormField
                            control={form.control}
                            name="bond.isin"
                            render={({ field }) => (
                              <FormItem>
                                <FormLabel>ISIN</FormLabel>
                                <FormControl>
                                  <Input
                                    placeholder="e.g. FR0014008IJ3"
                                    {...field}
                                    value={field.value ?? ""}
                                  />
                                </FormControl>
                                <FormMessage />
                              </FormItem>
                            )}
                          />
                        </div>

                        <p className="text-muted-foreground text-xs font-medium uppercase tracking-wide pt-2">
                          YTM Settings
                        </p>

                        <div className="grid gap-4 sm:grid-cols-2">
                          <FormField
                            control={form.control}
                            name="bond.ytmSource"
                            render={({ field }) => (
                              <FormItem>
                                <FormLabel>YTM Source</FormLabel>
                                <Select
                                  onValueChange={field.onChange}
                                  value={field.value ?? ""}
                                >
                                  <FormControl>
                                    <SelectTrigger className="h-9">
                                      <SelectValue placeholder="Select source" />
                                    </SelectTrigger>
                                  </FormControl>
                                  <SelectContent>
                                    <SelectItem value="FIXED">Fixed YTM</SelectItem>
                                    <SelectItem value="CURVE_PLUS_SPREAD">
                                      Curve + Z-Spread
                                    </SelectItem>
                                  </SelectContent>
                                </Select>
                                <FormMessage />
                              </FormItem>
                            )}
                          />

                          {watchedYtmSource === "FIXED" && (
                            <FormField
                              control={form.control}
                              name="bond.ytmFixedValue"
                              render={({ field }) => (
                                <FormItem>
                                  <FormLabel>YTM (%)</FormLabel>
                                  <FormControl>
                                    <Input
                                      type="number"
                                      step="0.001"
                                      placeholder="3.25"
                                      {...field}
                                      value={field.value ?? ""}
                                    />
                                  </FormControl>
                                  <p className="text-muted-foreground text-xs">
                                    Auto-rempli avec le taux de coupon (rendement courant
                                    au pair) lorsque face value, coupon, fréquence et
                                    maturité sont renseignés. Modifiez si vous disposez
                                    d'un YTM de marché.
                                  </p>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
                          )}

                          {watchedYtmSource === "CURVE_PLUS_SPREAD" && (
                            <FormField
                              control={form.control}
                              name="bond.ytmSpreadBps"
                              render={({ field }) => (
                                <FormItem>
                                  <FormLabel>Z-Spread (bps)</FormLabel>
                                  <FormControl>
                                    <Input
                                      type="number"
                                      step="1"
                                      placeholder="120"
                                      {...field}
                                      value={field.value ?? ""}
                                    />
                                  </FormControl>
                                  <FormMessage />
                                </FormItem>
                              )}
                            />
                          )}

                          <FormField
                            control={form.control}
                            name="bond.dayCountConvention"
                            render={({ field }) => (
                              <FormItem>
                                <FormLabel>Day Count</FormLabel>
                                <Select
                                  onValueChange={field.onChange}
                                  value={field.value ?? ""}
                                >
                                  <FormControl>
                                    <SelectTrigger className="h-9">
                                      <SelectValue placeholder="Select convention" />
                                    </SelectTrigger>
                                  </FormControl>
                                  <SelectContent>
                                    <SelectItem value="ACT/ACT">ACT/ACT</SelectItem>
                                    <SelectItem value="30/360">30/360</SelectItem>
                                    <SelectItem value="ACT/365">ACT/365</SelectItem>
                                    <SelectItem value="ACT/360">ACT/360</SelectItem>
                                  </SelectContent>
                                </Select>
                                <FormMessage />
                              </FormItem>
                            )}
                          />

                          <FormField
                            control={form.control}
                            name="bond.pricingMethod"
                            render={({ field }) => (
                              <FormItem>
                                <FormLabel>Price Type</FormLabel>
                                <Select
                                  onValueChange={field.onChange}
                                  value={field.value ?? ""}
                                >
                                  <FormControl>
                                    <SelectTrigger className="h-9">
                                      <SelectValue placeholder="Select type" />
                                    </SelectTrigger>
                                  </FormControl>
                                  <SelectContent>
                                    <SelectItem value="CLEAN">Clean price</SelectItem>
                                    <SelectItem value="DIRTY">Dirty price (+ accrued)</SelectItem>
                                  </SelectContent>
                                </Select>
                                <FormMessage />
                              </FormItem>
                            )}
                          />

                          <FormField
                            control={form.control}
                            name="bond.settlementDays"
                            render={({ field }) => (
                              <FormItem>
                                <FormLabel>Settlement Days (T+N)</FormLabel>
                                <FormControl>
                                  <Input
                                    type="number"
                                    step="1"
                                    min="0"
                                    placeholder="1"
                                    {...field}
                                    value={field.value ?? ""}
                                  />
                                </FormControl>
                                <FormMessage />
                              </FormItem>
                            )}
                          />
                        </div>

                        <FormField
                          control={form.control}
                          name="bond.firstCouponDate"
                          render={({ field }) => {
                            // Watch the dependency fields so the preview updates live
                            const bondValues = form.watch("bond");
                            const previewDates = generateCouponSchedule(
                              field.value ?? null,
                              bondValues?.couponFrequency ?? null,
                              bondValues?.maturityDate ?? null,
                            );
                            const manualSchedule = bondValues?.couponSchedule?.trim() ?? "";
                            return (
                              <FormItem>
                                <FormLabel>First Coupon Date</FormLabel>
                                <FormControl>
                                  <Input
                                    type="date"
                                    {...field}
                                    value={field.value ?? ""}
                                  />
                                </FormControl>
                                <p className="text-muted-foreground text-xs">
                                  The schedule is auto-generated from this date, the coupon
                                  frequency and the maturity date. You can override the full
                                  schedule below if your bond has irregular coupons.
                                </p>
                                {previewDates.length > 0 && manualSchedule.length === 0 && (
                                  <div className="bg-muted/40 rounded-md border px-3 py-2">
                                    <p className="text-muted-foreground text-xs font-medium">
                                      Generated schedule ({previewDates.length} payment
                                      {previewDates.length === 1 ? "" : "s"})
                                    </p>
                                    <p className="mt-1 break-words font-mono text-xs">
                                      {previewDates.join(", ")}
                                    </p>
                                  </div>
                                )}
                                <FormMessage />
                              </FormItem>
                            );
                          }}
                        />

                        <FormField
                          control={form.control}
                          name="bond.couponSchedule"
                          render={({ field }) => (
                            <FormItem>
                              <FormLabel>
                                Custom Coupon Schedule{" "}
                                <span className="text-muted-foreground text-xs">
                                  (optional)
                                </span>
                              </FormLabel>
                              <FormControl>
                                <Textarea
                                  rows={2}
                                  placeholder="Leave empty to use the auto-generated schedule above"
                                  {...field}
                                  value={field.value ?? ""}
                                />
                              </FormControl>
                              <p className="text-muted-foreground text-xs">
                                Comma-separated ISO dates (YYYY-MM-DD). Use only if your
                                bond has an irregular schedule that the auto-generator can't
                                produce.
                              </p>
                              <FormMessage />
                            </FormItem>
                          )}
                        />
                      </div>
                    )}

                    {/* Preferred Provider - Only show for automatic pricing */}
                    {isMarketMode && (
                      <FormField
                        control={form.control}
                        name="preferredProvider"
                        render={({ field }) => (
                          <FormItem>
                            <FormLabel>Preferred Provider</FormLabel>
                            <FormControl>
                              <ResponsiveSelect
                                value={field.value ?? "__auto__"}
                                onValueChange={(v) =>
                                  field.onChange(v === "__auto__" ? undefined : v)
                                }
                                options={providerOptions}
                                placeholder="Auto (default)"
                                sheetTitle="Preferred Provider"
                                sheetDescription="Select which provider to use first for this asset"
                                triggerClassName="h-11"
                              />
                            </FormControl>
                            <p className="text-muted-foreground text-xs">
                              Choose which provider to try first when fetching prices.
                            </p>
                          </FormItem>
                        )}
                      />
                    )}

                    {/* Symbol Mapping - Only show for automatic pricing */}
                    {isMarketMode && (
                      <div className="space-y-3">
                        <div className="flex items-center justify-between">
                          <div>
                            <label className="text-sm font-medium">Symbol Mapping</label>
                            <p className="text-muted-foreground text-xs">
                              Use a different ticker for specific providers if the default
                              doesn&apos;t work.
                            </p>
                          </div>
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={() => appendOverride({ provider: "YAHOO", symbol: "" })}
                          >
                            <Icons.Plus className="mr-1 h-3 w-3" />
                            Add
                          </Button>
                        </div>

                        {overrideFields.length === 0 ? (
                          <div className="rounded-lg border border-dashed p-6 text-center">
                            <Icons.Link className="text-muted-foreground/50 mx-auto h-8 w-8" />
                            <p className="text-muted-foreground mt-2 text-sm">
                              No symbol mappings configured
                            </p>
                            <p className="text-muted-foreground text-xs">
                              Using &quot;{asset.displayCode ?? ""}&quot; for all providers.
                            </p>
                          </div>
                        ) : (
                          <div className="rounded-lg border">
                            <table className="w-full">
                              <thead>
                                <tr className="bg-muted/50 border-b">
                                  <th className="text-muted-foreground px-4 py-2 text-left text-xs font-medium">
                                    Provider
                                  </th>
                                  <th className="text-muted-foreground px-4 py-2 text-left text-xs font-medium">
                                    Symbol
                                  </th>
                                  <th className="w-10"></th>
                                </tr>
                              </thead>
                              <tbody>
                                {overrideFields.map((field, index) => (
                                  <tr key={field.id} className="border-b last:border-b-0">
                                    <td className="px-4 py-2">
                                      <FormField
                                        control={form.control}
                                        name={`providerConfig.${index}.provider`}
                                        render={({ field: providerField }) => (
                                          <FormItem className="space-y-0">
                                            <FormControl>
                                              <ResponsiveSelect
                                                value={providerField.value}
                                                onValueChange={providerField.onChange}
                                                options={PROVIDERS.map((p) => ({
                                                  label: p.label,
                                                  value: p.value,
                                                }))}
                                                placeholder="Select provider"
                                                sheetTitle="Data Provider"
                                                sheetDescription="Select the data provider for this symbol mapping"
                                              />
                                            </FormControl>
                                          </FormItem>
                                        )}
                                      />
                                    </td>
                                    <td className="px-4 py-2">
                                      <FormField
                                        control={form.control}
                                        name={`providerConfig.${index}.symbol`}
                                        render={({ field: symbolField }) => (
                                          <FormItem className="space-y-0">
                                            <FormControl>
                                              <Input
                                                placeholder="e.g., SHOP.TO"
                                                {...symbolField}
                                                className="h-9 uppercase"
                                              />
                                            </FormControl>
                                          </FormItem>
                                        )}
                                      />
                                    </td>
                                    <td className="px-2 py-2">
                                      <Button
                                        type="button"
                                        variant="ghost"
                                        size="icon"
                                        className="h-8 w-8"
                                        onClick={() => removeOverride(index)}
                                      >
                                        <Icons.Close className="h-4 w-4" />
                                      </Button>
                                    </td>
                                  </tr>
                                ))}
                              </tbody>
                            </table>
                          </div>
                        )}
                      </div>
                    )}

                    {/* Save Actions */}
                    <div className="flex justify-end gap-3 border-t pt-4">
                      <Button
                        type="button"
                        variant="ghost"
                        onClick={() => onOpenChange(false)}
                        disabled={isSaving}
                      >
                        Cancel
                      </Button>
                      <Button
                        type="button"
                        onClick={form.handleSubmit(handleSave)}
                        disabled={isSaving}
                      >
                        {isSaving ? (
                          <span className="flex items-center gap-2">
                            <Icons.Spinner className="h-4 w-4 animate-spin" /> Saving
                          </span>
                        ) : (
                          "Save changes"
                        )}
                      </Button>
                    </div>
                  </div>
                </Form>
              </div>
            </TabsContent>
          </div>
        </Tabs>

        <div className="mt-auto border-t pt-4 sm:hidden">
          <Button
            type="button"
            variant="ghost"
            className="w-full"
            onClick={() => onOpenChange(false)}
          >
            Close
          </Button>
        </div>
      </SheetContent>
    </Sheet>
  );
}

function ClassificationSkeleton() {
  return (
    <div className="space-y-8">
      {Array.from({ length: 2 }).map((_, i) => (
        <div key={`single-${i}`} className="space-y-2">
          <Skeleton className="h-4 w-28" />
          <div className="flex flex-wrap gap-1.5">
            {Array.from({ length: 4 }).map((_, j) => (
              <Skeleton key={j} className="h-7 w-16 rounded-full" />
            ))}
          </div>
        </div>
      ))}
      {Array.from({ length: 3 }).map((_, i) => (
        <div key={`multi-${i}`} className="space-y-2">
          <Skeleton className="h-4 w-24" />
          <Skeleton className="h-10 w-full rounded-md" />
        </div>
      ))}
    </div>
  );
}

export default AssetEditSheet;
