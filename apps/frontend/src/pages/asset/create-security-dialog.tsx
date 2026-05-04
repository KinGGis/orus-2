import { getExchanges, getTaxonomy } from "@/adapters";
import TickerSearchInput from "@/components/ticker-search";
import { useSettingsContext } from "@/lib/settings-provider";
import { QueryKeys } from "@/lib/query-keys";
import type { NewAsset, SymbolSearchResult, TaxonomyCategory } from "@/lib/types";
import { useTaxonomies } from "@/hooks/use-taxonomies";
import { zodResolver } from "@hookform/resolvers/zod";
import { useQueries, useQuery } from "@tanstack/react-query";
import { CurrencyInput, SearchableSelect } from "@wealthfolio/ui";
import { Button } from "@wealthfolio/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@wealthfolio/ui/components/ui/dialog";
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
import { Textarea } from "@wealthfolio/ui/components/ui/textarea";
import { useCallback, useEffect, useMemo } from "react";
import { useForm } from "react-hook-form";
import { z } from "zod";

const INSTRUMENT_TYPE_OPTIONS = [
  { value: "EQUITY", label: "Equity (Stock, ETF, Fund)" },
  { value: "CRYPTO", label: "Cryptocurrency" },
  { value: "BOND", label: "Bond" },
  { value: "OPTION", label: "Option" },
  { value: "METAL", label: "Precious Metal" },
] as const;

const QUOTE_MODE_OPTIONS = [
  { value: "MANUAL", label: "Manual" },
  { value: "MARKET", label: "Market (auto-sync)" },
  { value: "INTERNAL_YTM", label: "Internal YTM (bond model)" },
] as const;

/** Map search result quoteType to our InstrumentType form values */
function mapQuoteTypeToInstrumentType(quoteType: string): string {
  switch (quoteType.toUpperCase()) {
    case "EQUITY":
    case "ETF":
    case "MUTUALFUND":
    case "INDEX":
    case "ECNQUOTE":
      return "EQUITY";
    case "CRYPTOCURRENCY":
      return "CRYPTO";
    case "BOND":
    case "MONEYMARKET":
      return "BOND";
    case "OPTION":
      return "OPTION";
    default:
      return "EQUITY";
  }
}

const createSecuritySchema = z.object({
  symbol: z
    .string()
    .min(1, "Symbol is required")
    .max(20, "Symbol must be 20 characters or less")
    .transform((val) => val.toUpperCase().trim()),
  name: z.string().min(1, "Name is required").max(100, "Name must be 100 characters or less"),
  instrumentType: z.string().min(1, "Instrument type is required"),
  quoteCcy: z.string().min(1, "Currency is required"),
  quoteMode: z.enum(["MANUAL", "MARKET", "INTERNAL_YTM"]),
  instrumentExchangeMic: z.string().optional(),
  country: z.string().optional(),
  sector: z.string().optional(),
  notes: z.string().optional(),
});

type CreateSecurityFormValues = z.infer<typeof createSecuritySchema>;

/** Classifications resolved from free-text country/sector inputs at submit time. */
export interface ResolvedClassifications {
  /** Selected category in the Regions taxonomy (best fuzzy match for the country input). */
  region?: { taxonomyId: string; categoryId: string };
  /** Selected category in the Industries (GICS) taxonomy (best fuzzy match for the sector input). */
  sector?: { taxonomyId: string; categoryId: string };
  /** When true, caller should auto-assign the default Bond classifications. */
  autoBondClassify: boolean;
}

/** Normalize a string for fuzzy comparison: lowercase, strip diacritics & non-alphanum. */
function normalizeForMatch(s: string): string {
  return s
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "");
}

/**
 * Find the best matching category for a free-text query against a list of
 * taxonomy categories. Returns `null` if the input is empty or no candidate
 * looks plausible. Match priority:
 *   1. exact normalized name or key
 *   2. category name starts with the query
 *   3. category name contains the query
 */
function findBestCategory(
  query: string | undefined,
  categories: TaxonomyCategory[],
): TaxonomyCategory | null {
  if (!query) return null;
  const q = normalizeForMatch(query);
  if (!q) return null;
  const candidates = categories.map((c) => ({
    cat: c,
    name: normalizeForMatch(c.name),
    key: normalizeForMatch(c.key ?? ""),
  }));
  const exact = candidates.find((c) => c.name === q || c.key === q);
  if (exact) return exact.cat;
  const startsWith = candidates.find((c) => c.name.startsWith(q));
  if (startsWith) return startsWith.cat;
  const contains = candidates.find((c) => c.name.includes(q) || q.includes(c.name));
  if (contains) return contains.cat;
  return null;
}

const normalizeMic = (mic?: string | null): string => mic?.trim().toUpperCase() ?? "";

interface CreateSecurityDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (payload: NewAsset, classifications: ResolvedClassifications) => void;
  isPending?: boolean;
}

export function CreateSecurityDialog({
  open,
  onOpenChange,
  onSubmit,
  isPending = false,
}: CreateSecurityDialogProps) {
  const { settings } = useSettingsContext();
  const defaultCurrency = settings?.baseCurrency || "USD";

  const { data: exchanges = [] } = useQuery({
    queryKey: ["exchanges"],
    queryFn: getExchanges,
    staleTime: Infinity,
  });

  const exchangeOptions = useMemo(
    () =>
      exchanges.map((e) => ({
        value: normalizeMic(e.mic),
        label: `${e.longName} (${e.name})`,
      })),
    [exchanges],
  );

  // Load taxonomies once to find the IDs for "Regions" and "Industries (GICS)".
  const { data: taxonomies = [] } = useTaxonomies();
  const regionsTaxonomy = useMemo(
    () =>
      taxonomies.find((t) => {
        const n = t.name.toLowerCase();
        return n === "regions" || n === "region" || n.includes("region");
      }),
    [taxonomies],
  );
  const sectorsTaxonomy = useMemo(
    () =>
      taxonomies.find((t) => {
        const n = t.name.toLowerCase();
        return n.includes("industr") || n.includes("sector") || n.includes("gics");
      }),
    [taxonomies],
  );

  // Lazy-load each target taxonomy's categories (only when its id is known).
  const taxonomyDetailQueries = useQueries({
    queries: [regionsTaxonomy?.id, sectorsTaxonomy?.id]
      .filter((id): id is string => !!id)
      .map((id) => ({
        queryKey: QueryKeys.taxonomy(id),
        queryFn: () => getTaxonomy(id),
        staleTime: 5 * 60 * 1000,
      })),
  });
  const regionCategories: TaxonomyCategory[] = useMemo(() => {
    const q = taxonomyDetailQueries.find(
      (r) => r.data?.taxonomy.id === regionsTaxonomy?.id,
    );
    return q?.data?.categories ?? [];
  }, [taxonomyDetailQueries, regionsTaxonomy?.id]);
  const sectorCategories: TaxonomyCategory[] = useMemo(() => {
    const q = taxonomyDetailQueries.find(
      (r) => r.data?.taxonomy.id === sectorsTaxonomy?.id,
    );
    return q?.data?.categories ?? [];
  }, [taxonomyDetailQueries, sectorsTaxonomy?.id]);

  const form = useForm<CreateSecurityFormValues>({
    resolver: zodResolver(createSecuritySchema),
    defaultValues: {
      symbol: "",
      name: "",
      instrumentType: "EQUITY",
      quoteCcy: defaultCurrency,
      quoteMode: "MANUAL",
      instrumentExchangeMic: "",
      country: "",
      sector: "",
      notes: "",
    },
  });

  useEffect(() => {
    if (open) {
      form.reset({
        symbol: "",
        name: "",
        instrumentType: "EQUITY",
        quoteCcy: defaultCurrency,
        quoteMode: "MANUAL",
        instrumentExchangeMic: "",
        country: "",
        sector: "",
        notes: "",
      });
    }
  }, [open, defaultCurrency, form]);

  const handleTickerSelect = useCallback(
    (_symbol: string, result?: SymbolSearchResult) => {
      if (!result) return;

      form.setValue("symbol", result.symbol.toUpperCase(), { shouldValidate: true });
      form.setValue("name", result.longName || result.shortName || "", { shouldValidate: true });

      if (result.quoteType) {
        form.setValue("instrumentType", mapQuoteTypeToInstrumentType(result.quoteType));
      }
      if (result.currency) {
        form.setValue("quoteCcy", result.currency, { shouldValidate: true });
      }
      if (result.exchangeMic) {
        form.setValue("instrumentExchangeMic", normalizeMic(result.exchangeMic));
      }

      // If the result comes from a data source (not manual), default to auto-sync
      const isManual = result.dataSource === "MANUAL";
      form.setValue("quoteMode", isManual ? "MANUAL" : "MARKET");
    },
    [form],
  );

  const handleSubmit = (values: CreateSecurityFormValues) => {
    const payload: NewAsset = {
      kind: "INVESTMENT",
      name: values.name,
      displayCode: values.symbol,
      isActive: true,
      quoteMode: values.quoteMode,
      quoteCcy: values.quoteCcy,
      instrumentType: values.instrumentType,
      instrumentSymbol: values.symbol,
      instrumentExchangeMic: values.instrumentExchangeMic || undefined,
      notes: values.notes || undefined,
    };

    const regionCat = regionsTaxonomy
      ? findBestCategory(values.country, regionCategories)
      : null;
    const sectorCat = sectorsTaxonomy
      ? findBestCategory(values.sector, sectorCategories)
      : null;

    const classifications: ResolvedClassifications = {
      region:
        regionsTaxonomy && regionCat
          ? { taxonomyId: regionsTaxonomy.id, categoryId: regionCat.id }
          : undefined,
      sector:
        sectorsTaxonomy && sectorCat
          ? { taxonomyId: sectorsTaxonomy.id, categoryId: sectorCat.id }
          : undefined,
      autoBondClassify:
        values.instrumentType === "BOND" || values.quoteMode === "INTERNAL_YTM",
    };

    onSubmit(payload, classifications);
  };

  const handleDialogKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "Enter") return;
    if ((e.target as HTMLElement).tagName === "TEXTAREA") return;
    // Don't submit when interacting with the ticker search popover
    const inPopover = (e.target as HTMLElement).closest("[data-radix-popper-content-wrapper]");
    if (inPopover) return;
    e.preventDefault();
    void form.handleSubmit(handleSubmit)();
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Add Security</DialogTitle>
          <DialogDescription>
            Search for a security to auto-fill details, or enter them manually.
          </DialogDescription>
        </DialogHeader>

        <Form {...form}>
          <div className="space-y-4" onKeyDown={handleDialogKeyDown}>
            {/* Ticker search - auto-populates form fields on selection */}
            {open && (
              <div className="space-y-2">
                <label className="text-sm font-medium">Search</label>
                <TickerSearchInput
                  onSelectResult={handleTickerSelect}
                  placeholder="Search by name or symbol..."
                  defaultCurrency={defaultCurrency}
                  autoFocusSearch
                  hideCustomCreate
                />
              </div>
            )}

            <div className="grid grid-cols-2 gap-4">
              <FormField
                control={form.control}
                name="symbol"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Symbol</FormLabel>
                    <FormControl>
                      <Input
                        placeholder="e.g., AAPL"
                        {...field}
                        onChange={(e) => field.onChange(e.target.value.toUpperCase())}
                        className="uppercase"
                      />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="instrumentType"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Type</FormLabel>
                    <Select onValueChange={field.onChange} value={field.value}>
                      <FormControl>
                        <SelectTrigger>
                          <SelectValue placeholder="Select type" />
                        </SelectTrigger>
                      </FormControl>
                      <SelectContent>
                        {INSTRUMENT_TYPE_OPTIONS.map((option) => (
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
            </div>

            <FormField
              control={form.control}
              name="name"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Name</FormLabel>
                  <FormControl>
                    <Input placeholder="e.g., Apple Inc." {...field} />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />

            <div className="grid grid-cols-2 gap-4">
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

              <FormField
                control={form.control}
                name="quoteMode"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Quote Mode</FormLabel>
                    <Select onValueChange={field.onChange} value={field.value}>
                      <FormControl>
                        <SelectTrigger>
                          <SelectValue />
                        </SelectTrigger>
                      </FormControl>
                      <SelectContent>
                        {QUOTE_MODE_OPTIONS.map((option) => (
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
            </div>

            <FormField
              control={form.control}
              name="instrumentExchangeMic"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>
                    Exchange <span className="text-muted-foreground text-xs">(optional)</span>
                  </FormLabel>
                  <FormControl>
                    <SearchableSelect
                      options={exchangeOptions}
                      value={field.value ?? ""}
                      onValueChange={field.onChange}
                      placeholder="Select exchange"
                      searchPlaceholder="Search exchanges..."
                    />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />

            <div className="grid grid-cols-2 gap-4">
              <FormField
                control={form.control}
                name="country"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>
                      Country{" "}
                      <span className="text-muted-foreground text-xs">(optional)</span>
                    </FormLabel>
                    <FormControl>
                      <Input
                        list="create-security-country-list"
                        placeholder="e.g., Switzerland"
                        {...field}
                        value={field.value ?? ""}
                      />
                    </FormControl>
                    <datalist id="create-security-country-list">
                      {regionCategories.map((c) => (
                        <option key={c.id} value={c.name} />
                      ))}
                    </datalist>
                    <p className="text-muted-foreground text-xs">
                      Free text — best match in the Regions taxonomy is auto-selected.
                    </p>
                    <FormMessage />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="sector"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>
                      Sector{" "}
                      <span className="text-muted-foreground text-xs">(optional)</span>
                    </FormLabel>
                    <FormControl>
                      <Input
                        list="create-security-sector-list"
                        placeholder="e.g., Financials"
                        {...field}
                        value={field.value ?? ""}
                      />
                    </FormControl>
                    <datalist id="create-security-sector-list">
                      {sectorCategories.map((c) => (
                        <option key={c.id} value={c.name} />
                      ))}
                    </datalist>
                    <p className="text-muted-foreground text-xs">
                      Free text — best match in the Industries (GICS) taxonomy is
                      auto-selected.
                    </p>
                    <FormMessage />
                  </FormItem>
                )}
              />
            </div>

            <FormField
              control={form.control}
              name="notes"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>
                    Notes <span className="text-muted-foreground text-xs">(optional)</span>
                  </FormLabel>
                  <FormControl>
                    <Textarea rows={2} placeholder="Any additional notes..." {...field} />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />

            <DialogFooter className="gap-2 sm:gap-0">
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
                disabled={isPending}
              >
                Cancel
              </Button>
              <Button
                type="button"
                onClick={() => void form.handleSubmit(handleSubmit)()}
                disabled={isPending}
              >
                {isPending ? (
                  <span className="flex items-center gap-2">
                    <Icons.Spinner className="h-4 w-4 animate-spin" /> Creating...
                  </span>
                ) : (
                  "Create Security"
                )}
              </Button>
            </DialogFooter>
          </div>
        </Form>
      </DialogContent>
    </Dialog>
  );
}
