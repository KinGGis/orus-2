import { orusSupabase } from "@/features/orus-integration/supabase-client";
import { useOrusAuth } from "@/features/orus-integration/orus-auth-context";
import { Button } from "@wealthfolio/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui/components/ui/card";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Input } from "@wealthfolio/ui/components/ui/input";
import { Label } from "@wealthfolio/ui/components/ui/label";
import { Separator } from "@wealthfolio/ui/components/ui/separator";
import { toast } from "@wealthfolio/ui/components/ui/use-toast";
import { useState } from "react";
import { useNavigate } from "react-router-dom";

export default function PasswordPage() {
  const { isAuthenticated } = useOrusAuth();
  const navigate = useNavigate();
  const [isSaving, setIsSaving] = useState(false);
  const [formData, setFormData] = useState({
    newPassword: "",
    confirmPassword: "",
  });
  const [errors, setErrors] = useState<{
    newPassword?: string;
    confirmPassword?: string;
  }>({});

  if (!isAuthenticated) {
    navigate("/settings/account");
    return null;
  }

  const validateForm = () => {
    const newErrors: typeof errors = {};

    if (formData.newPassword.length < 8) {
      newErrors.newPassword = "Password must be at least 8 characters";
    }

    if (formData.newPassword !== formData.confirmPassword) {
      newErrors.confirmPassword = "Passwords do not match";
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!validateForm()) return;

    setIsSaving(true);

    try {
      const { error } = await orusSupabase.auth.updateUser({
        password: formData.newPassword,
      });

      if (error) {
        toast({
          title: "Error",
          description: error.message,
          variant: "destructive",
        });
      } else {
        toast({
          title: "Password updated",
          description: "Your password has been changed successfully.",
          variant: "success",
        });
        navigate("/settings/account");
      }
    } catch (err) {
      toast({
        title: "Error",
        description: "Failed to update password.",
        variant: "destructive",
      });
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={() => navigate("/settings/account")}>
          <Icons.ArrowLeft className="h-4 w-4" />
        </Button>
        <div>
          <h3 className="text-lg font-medium">Change Password</h3>
          <p className="text-muted-foreground text-sm">
            Update your account password.
          </p>
        </div>
      </div>
      <Separator />

      <form onSubmit={handleSubmit} className="space-y-6">
        <Card>
          <CardHeader>
            <CardTitle className="text-base">New Password</CardTitle>
            <CardDescription>
              Choose a strong password with at least 8 characters.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="newPassword">New Password</Label>
              <Input
                id="newPassword"
                type="password"
                value={formData.newPassword}
                onChange={(e) => {
                  setFormData({ ...formData, newPassword: e.target.value });
                  setErrors({ ...errors, newPassword: undefined });
                }}
                placeholder="••••••••"
              />
              {errors.newPassword && (
                <p className="text-destructive text-sm">{errors.newPassword}</p>
              )}
            </div>
            <div className="space-y-2">
              <Label htmlFor="confirmPassword">Confirm New Password</Label>
              <Input
                id="confirmPassword"
                type="password"
                value={formData.confirmPassword}
                onChange={(e) => {
                  setFormData({ ...formData, confirmPassword: e.target.value });
                  setErrors({ ...errors, confirmPassword: undefined });
                }}
                placeholder="••••••••"
              />
              {errors.confirmPassword && (
                <p className="text-destructive text-sm">{errors.confirmPassword}</p>
              )}
            </div>
          </CardContent>
        </Card>

        {/* Actions */}
        <div className="flex justify-end gap-3">
          <Button type="button" variant="outline" onClick={() => navigate("/settings/account")}>
            Cancel
          </Button>
          <Button type="submit" disabled={isSaving}>
            {isSaving ? (
              <>
                <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                Updating...
              </>
            ) : (
              "Update Password"
            )}
          </Button>
        </div>
      </form>
    </div>
  );
}
