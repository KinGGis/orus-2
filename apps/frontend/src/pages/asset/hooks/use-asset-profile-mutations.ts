import { useMutation, useQueryClient } from "@tanstack/react-query";
import { updateAssetProfile, updateQuoteMode, logger } from "@/adapters";
import { toast } from "@wealthfolio/ui/components/ui/use-toast";
import { QueryKeys } from "@/lib/query-keys";

export const useAssetProfileMutations = () => {
  const queryClient = useQueryClient();

  const handleSuccess = (message: string, assetId: string) => {
    queryClient.invalidateQueries({ queryKey: [QueryKeys.HOLDINGS] });
    queryClient.invalidateQueries({ queryKey: [QueryKeys.ASSET_DATA, assetId] });
    queryClient.invalidateQueries({ queryKey: [QueryKeys.ACTIVITY_DATA] });
    toast({
      title: message,
      variant: "success",
    });
  };

  const extractMessage = (error: unknown): string | null => {
    if (!error) return null;
    if (typeof error === "string") return error;
    if (error instanceof Error) return error.message;
    if (typeof error === "object" && "message" in error) {
      const m = (error as { message?: unknown }).message;
      return typeof m === "string" ? m : null;
    }
    return null;
  };

  const friendlyServerMessage = (raw: string | null): string | null => {
    if (!raw) return null;
    if (raw.includes("UNIQUE constraint failed: assets.instrument_key")) {
      return (
        "Another asset already exists with the same Instrument Type + Symbol " +
        "(+ Exchange MIC). Change one of those fields, or delete the duplicate " +
        "asset before retrying."
      );
    }
    return raw;
  };

  const handleError = (action: string, error: unknown) => {
    const friendly = friendlyServerMessage(extractMessage(error));
    toast({
      title: "Uh oh! Something went wrong.",
      description: friendly ?? `There was a problem ${action}.`,
      variant: "destructive",
    });
  };

  const updateAssetProfileMutation = useMutation({
    mutationFn: updateAssetProfile,
    onSuccess: (result) => {
      handleSuccess("Asset profile updated successfully.", result.id);
    },
    onError: (error) => {
      logger.error(`Error updating asset profile: ${error}`);
      handleError("updating the asset profile", error);
    },
  });

  const updateQuoteModeMutation = useMutation({
    mutationFn: ({ assetId, quoteMode }: { assetId: string; quoteMode: string }) =>
      updateQuoteMode(assetId, quoteMode),
    onSuccess: (result) => {
      handleSuccess("Asset quote mode updated successfully.", result.id);
    },
    onError: (error) => {
      logger.error(`Error updating asset quote mode: ${error}`);
      handleError("updating the asset quote mode", error);
    },
  });

  return {
    updateAssetProfileMutation,
    updateQuoteModeMutation,
  };
};
