import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { useToast } from "@wealthfolio/ui/components/ui/use-toast";
import { useEffect } from "react";
import { useNavigate } from "react-router-dom";

export default function SnapTradeCallbackPage() {
  const { toast } = useToast();
  const navigate = useNavigate();

  useEffect(() => {
    toast({
      title: "Succès",
      description: "Courtier connecté avec succès!",
    });
    // Redirect to settings after a short delay
    const timer = setTimeout(() => {
      navigate("/settings/snaptrade");
    }, 1500);
    return () => clearTimeout(timer);
  }, [toast, navigate]);

  return (
    <div className="flex min-h-screen flex-col items-center justify-center">
      <Icons.Spinner className="text-muted-foreground h-12 w-12 animate-spin" />
      <p className="text-muted-foreground mt-4">Redirection en cours...</p>
    </div>
  );
}
