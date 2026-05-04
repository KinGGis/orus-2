import { useMemo, useState } from "react";

import { Separator } from "@wealthfolio/ui";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@wealthfolio/ui/components/ui/alert-dialog";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { RefreshQuotesConfirmDialog } from "./refresh-quotes-confirm-dialog";

import { useHoldings } from "@/hooks/use-holdings";
import { useIsMobileViewport } from "@/hooks/use-platform";
import { useSyncMarketDataMutation } from "@/hooks/use-sync-market-data";
import { PORTFOLIO_ACCOUNT_ID } from "@/lib/constants";
import { SettingsHeader } from "../settings/settings-header";
import { AssetEditSheet } from "./asset-edit-sheet";
import { ParsedAsset, toParsedAsset } from "./asset-utils";
import { AssetsTable } from "./assets-table";
import { AssetsTableMobile } from "./assets-table-mobile";
import { CreateSecurityDialog } from "./create-security-dialog";
import type { ResolvedClassifications } from "./create-security-dialog";
import { useAssetManagement } from "./hooks/use-asset-management";
import { useAssets } from "./hooks/use-assets";
import { useLatestQuotes } from "./hooks/use-latest-quotes";
import { useAssignAssetToCategory, useTaxonomies } from "@/hooks/use-taxonomies";

export default function AssetsPage() {
  const { assets, isLoading } = useAssets();
  const { createAssetMutation, deleteAssetMutation } = useAssetManagement();
  const refetchQuotesMutation = useSyncMarketDataMutation(true);
  const updateQuotesMutation = useSyncMarketDataMutation(false);
  const isMobileViewport = useIsMobileViewport();
  const { holdings } = useHoldings(PORTFOLIO_ACCOUNT_ID);

  const heldAssetIds = useMemo(() => {
    const ids = new Set<string>();
    for (const h of holdings) {
      if (h.instrument?.id) {
        ids.add(h.instrument.id);
      }
    }
    return ids;
  }, [holdings]);

  const parsedAssets = useMemo(() => assets.map(toParsedAsset), [assets]);
  const assetIds = useMemo(() => parsedAssets.map((asset) => asset.id), [parsedAssets]);
  const { data: latestQuotes = {}, isLoading: isQuotesLoading } = useLatestQuotes(assetIds);

  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [editingAsset, setEditingAsset] = useState<ParsedAsset | null>(null);
  const [assetPendingDelete, setAssetPendingDelete] = useState<ParsedAsset | null>(null);
  const [assetPendingRefetch, setAssetPendingRefetch] = useState<ParsedAsset | null>(null);

  // Taxonomy data needed to apply default Bond classifications after asset creation.
  const { data: allTaxonomies = [] } = useTaxonomies();
  const assignAssetToCategory = useAssignAssetToCategory();

  /**
   * After a security is successfully created, persist the resolved
   * classifications (region/sector picked via fuzzy match) and, when the
   * asset is a bond, also auto-assign the default Instrument-Type and
   * Asset-Class categories ("Bonds" / "Fixed Income"). Failures are logged
   * but not surfaced as errors so they never block the creation flow.
   */
  const applyClassificationsAfterCreate = async (
    assetId: string,
    classifications: ResolvedClassifications,
  ) => {
    const tasks: Array<Promise<unknown>> = [];

    if (classifications.region) {
      tasks.push(
        assignAssetToCategory.mutateAsync({
          assetId,
          taxonomyId: classifications.region.taxonomyId,
          categoryId: classifications.region.categoryId,
          weight: 10000,
          source: "manual",
        }),
      );
    }
    if (classifications.sector) {
      tasks.push(
        assignAssetToCategory.mutateAsync({
          assetId,
          taxonomyId: classifications.sector.taxonomyId,
          categoryId: classifications.sector.categoryId,
          weight: 10000,
          source: "manual",
        }),
      );
    }
    if (classifications.autoBondClassify) {
      // Look up the Instrument Type and Asset Classes taxonomies, then find
      // their "Bonds" / "Fixed Income" categories. We accept a few common
      // variants of the names/keys so this stays robust to taxonomy edits.
      const findTaxonomy = (predicates: Array<(name: string) => boolean>) =>
        allTaxonomies.find((t) => predicates.some((p) => p(t.name.toLowerCase())));
      const instrumentTypeTaxonomy = findTaxonomy([
        (n) => n.includes("instrument type"),
        (n) => n === "instrument types",
      ]);
      const assetClassTaxonomy = findTaxonomy([
        (n) => n.includes("asset class"),
      ]);

      const enqueueBondCategory = async (
        taxonomyId: string,
        wantedKeys: string[],
        wantedNames: string[],
      ) => {
        try {
          const { getTaxonomy } = await import("@/adapters");
          const detail = await getTaxonomy(taxonomyId);
          if (!detail) return;
          const cat = detail.categories.find(
            (c) =>
              wantedKeys.includes(c.key?.toUpperCase() ?? "") ||
              wantedNames.some(
                (n) => c.name.toLowerCase() === n.toLowerCase(),
              ),
          );
          if (!cat) return;
          await assignAssetToCategory.mutateAsync({
            assetId,
            taxonomyId,
            categoryId: cat.id,
            weight: 10000,
            source: "auto",
          });
        } catch {
          // Swallow — auto-classification is best-effort.
        }
      };

      if (instrumentTypeTaxonomy) {
        tasks.push(
          enqueueBondCategory(
            instrumentTypeTaxonomy.id,
            ["BOND_CORPORATE", "BOND", "BONDS"],
            ["Bonds", "Bond", "Corporate Bond", "Corporate Bonds"],
          ),
        );
      }
      if (assetClassTaxonomy) {
        tasks.push(
          enqueueBondCategory(
            assetClassTaxonomy.id,
            ["FIXED_INCOME", "FIXEDINCOME", "BONDS"],
            ["Fixed Income", "Bonds"],
          ),
        );
      }
    }

    await Promise.allSettled(tasks);
  };

  const handleDelete = async () => {
    if (!assetPendingDelete) return;
    await deleteAssetMutation.mutateAsync(assetPendingDelete.id);
    setAssetPendingDelete(null);
  };

  return (
    <div className="space-y-6">
      <SettingsHeader
        heading="Securities"
        text="Browse and manage the securities available in your portfolio."
      >
        <Button onClick={() => setCreateDialogOpen(true)} size="sm">
          <Icons.Plus className="mr-2 h-4 w-4" />
          Add Security
        </Button>
      </SettingsHeader>
      <Separator />
      <div className="w-full">
        {isMobileViewport ? (
          <AssetsTableMobile
            assets={parsedAssets}
            latestQuotes={latestQuotes}
            heldAssetIds={heldAssetIds}
            isLoading={isLoading || isQuotesLoading}
            onEdit={(asset) => setEditingAsset(asset)}
            onDelete={(asset) => setAssetPendingDelete(asset)}
            onUpdateQuotes={(asset) => updateQuotesMutation.mutate([asset.id])}
            onRefetchQuotes={(asset) => setAssetPendingRefetch(asset)}
            isUpdatingQuotes={updateQuotesMutation.isPending}
            isRefetchingQuotes={refetchQuotesMutation.isPending}
          />
        ) : (
          <AssetsTable
            assets={parsedAssets}
            latestQuotes={latestQuotes}
            heldAssetIds={heldAssetIds}
            isLoading={isLoading || isQuotesLoading}
            onEdit={(asset) => setEditingAsset(asset)}
            onDelete={(asset) => setAssetPendingDelete(asset)}
            onUpdateQuotes={(asset) => updateQuotesMutation.mutate([asset.id])}
            onRefetchQuotes={(asset) => setAssetPendingRefetch(asset)}
            isUpdatingQuotes={updateQuotesMutation.isPending}
            isRefetchingQuotes={refetchQuotesMutation.isPending}
          />
        )}
      </div>

      <AssetEditSheet
        asset={editingAsset}
        latestQuote={editingAsset ? (latestQuotes[editingAsset.id]?.quote ?? null) : null}
        open={!!editingAsset}
        onOpenChange={(open) => {
          if (!open) {
            setEditingAsset(null);
          }
        }}
      />

      <AlertDialog
        open={!!assetPendingDelete}
        onOpenChange={(open) => {
          if (!open) {
            setAssetPendingDelete(null);
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete security</AlertDialogTitle>
            <AlertDialogDescription>
              {assetPendingDelete
                ? `Are you sure you want to delete ${assetPendingDelete.displayCode ?? assetPendingDelete.name ?? "this security"}? This will also remove its related quote and cannot be undone.`
                : "Are you sure you want to delete this security? This will also remove related quotes and cannot be undone."}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={handleDelete}
              disabled={deleteAssetMutation.isPending}
              className="bg-destructive hover:bg-destructive/90 dark:text-foreground"
            >
              {deleteAssetMutation.isPending ? "Deleting..." : "Delete"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <RefreshQuotesConfirmDialog
        open={!!assetPendingRefetch}
        onOpenChange={(open) => {
          if (!open) setAssetPendingRefetch(null);
        }}
        onConfirm={() => {
          if (assetPendingRefetch) {
            refetchQuotesMutation.mutate([assetPendingRefetch.id]);
          }
          setAssetPendingRefetch(null);
        }}
        assetName={assetPendingRefetch?.displayCode ?? assetPendingRefetch?.name ?? undefined}
      />

      <CreateSecurityDialog
        open={createDialogOpen}
        onOpenChange={setCreateDialogOpen}
        onSubmit={(payload, classifications) => {
          createAssetMutation.mutate(payload, {
            onSuccess: (asset) => {
              setCreateDialogOpen(false);
              void applyClassificationsAfterCreate(asset.id, classifications);
            },
          });
        }}
        isPending={createAssetMutation.isPending}
      />
    </div>
  );
}
