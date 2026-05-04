import { useOrusAuth } from "@/features/orus-integration/orus-auth-context";
import { Avatar, AvatarFallback, AvatarImage } from "@wealthfolio/ui/components/ui/avatar";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui/components/ui/select";
import { Separator } from "@wealthfolio/ui/components/ui/separator";
import { toast } from "@wealthfolio/ui/components/ui/use-toast";
import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";

const CURRENCIES = [
  { value: "EUR", label: "Euro (€)" },
  { value: "USD", label: "US Dollar ($)" },
  { value: "GBP", label: "British Pound (£)" },
  { value: "CHF", label: "Swiss Franc (CHF)" },
  { value: "CAD", label: "Canadian Dollar (C$)" },
  { value: "AUD", label: "Australian Dollar (A$)" },
  { value: "JPY", label: "Japanese Yen (¥)" },
];

const LANGUAGES = [
  { value: "fr", label: "Français" },
  { value: "en", label: "English" },
  { value: "de", label: "Deutsch" },
  { value: "es", label: "Español" },
];

export default function ProfilePage() {
  const { user, profile, updateProfile, isAuthenticated } = useOrusAuth();
  const navigate = useNavigate();
  const [isSaving, setIsSaving] = useState(false);
  const [formData, setFormData] = useState({
    full_name: "",
    phone: "",
    company: "",
    job_title: "",
    preferred_currency: "EUR",
    preferred_language: "fr",
  });

  useEffect(() => {
    if (profile) {
      setFormData({
        full_name: profile.full_name || "",
        phone: profile.phone || "",
        company: profile.company || "",
        job_title: profile.job_title || "",
        preferred_currency: profile.preferred_currency || "EUR",
        preferred_language: profile.preferred_language || "fr",
      });
    }
  }, [profile]);

  if (!isAuthenticated) {
    navigate("/settings/account");
    return null;
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsSaving(true);

    try {
      const { error } = await updateProfile(formData);
      if (error) {
        toast({
          title: "Error",
          description: error.message,
          variant: "destructive",
        });
      } else {
        toast({
          title: "Profile updated",
          description: "Your profile has been updated successfully.",
          variant: "success",
        });
        navigate("/settings/account");
      }
    } catch (err) {
      toast({
        title: "Error",
        description: "Failed to update profile.",
        variant: "destructive",
      });
    } finally {
      setIsSaving(false);
    }
  };

  const initials = formData.full_name
    ?.split(" ")
    .map((n) => n[0])
    .join("")
    .toUpperCase()
    .slice(0, 2) || user?.email?.slice(0, 2).toUpperCase() || "??";

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={() => navigate("/settings/account")}>
          <Icons.ArrowLeft className="h-4 w-4" />
        </Button>
        <div>
          <h3 className="text-lg font-medium">Edit Profile</h3>
          <p className="text-muted-foreground text-sm">
            Update your personal information.
          </p>
        </div>
      </div>
      <Separator />

      <form onSubmit={handleSubmit} className="space-y-6">
        {/* Avatar */}
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Profile Picture</CardTitle>
            <CardDescription>Your avatar is displayed across the application.</CardDescription>
          </CardHeader>
          <CardContent className="flex items-center gap-4">
            <Avatar className="h-20 w-20">
              <AvatarImage src={profile?.avatar_url || undefined} alt={formData.full_name} />
              <AvatarFallback className="text-xl">{initials}</AvatarFallback>
            </Avatar>
            <div className="text-muted-foreground text-sm">
              Avatar is managed by your email provider (Gravatar).
            </div>
          </CardContent>
        </Card>

        {/* Personal Information */}
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Personal Information</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="full_name">Full Name</Label>
                <Input
                  id="full_name"
                  value={formData.full_name}
                  onChange={(e) => setFormData({ ...formData, full_name: e.target.value })}
                  placeholder="John Doe"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="email">Email</Label>
                <Input
                  id="email"
                  value={user?.email || ""}
                  disabled
                  className="bg-muted"
                />
                <p className="text-muted-foreground text-xs">Email cannot be changed.</p>
              </div>
              <div className="space-y-2">
                <Label htmlFor="phone">Phone</Label>
                <Input
                  id="phone"
                  value={formData.phone}
                  onChange={(e) => setFormData({ ...formData, phone: e.target.value })}
                  placeholder="+33 6 12 34 56 78"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="company">Company</Label>
                <Input
                  id="company"
                  value={formData.company}
                  onChange={(e) => setFormData({ ...formData, company: e.target.value })}
                  placeholder="Acme Inc."
                />
              </div>
              <div className="space-y-2 sm:col-span-2">
                <Label htmlFor="job_title">Job Title</Label>
                <Input
                  id="job_title"
                  value={formData.job_title}
                  onChange={(e) => setFormData({ ...formData, job_title: e.target.value })}
                  placeholder="Software Engineer"
                />
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Preferences */}
        <Card>
          <CardHeader>
            <CardTitle className="text-base">Preferences</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="currency">Preferred Currency</Label>
                <Select
                  value={formData.preferred_currency}
                  onValueChange={(value) => setFormData({ ...formData, preferred_currency: value })}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="Select currency" />
                  </SelectTrigger>
                  <SelectContent>
                    {CURRENCIES.map((currency) => (
                      <SelectItem key={currency.value} value={currency.value}>
                        {currency.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-2">
                <Label htmlFor="language">Language</Label>
                <Select
                  value={formData.preferred_language}
                  onValueChange={(value) => setFormData({ ...formData, preferred_language: value })}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="Select language" />
                  </SelectTrigger>
                  <SelectContent>
                    {LANGUAGES.map((lang) => (
                      <SelectItem key={lang.value} value={lang.value}>
                        {lang.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
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
                Saving...
              </>
            ) : (
              "Save Changes"
            )}
          </Button>
        </div>
      </form>
    </div>
  );
}
