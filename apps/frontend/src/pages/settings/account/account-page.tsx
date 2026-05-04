import { useOrusAuth } from "@/features/orus-integration/orus-auth-context";
import { Avatar, AvatarFallback, AvatarImage } from "@wealthfolio/ui/components/ui/avatar";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Button } from "@wealthfolio/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui/components/ui/card";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Separator } from "@wealthfolio/ui/components/ui/separator";
import { useState } from "react";
import { useNavigate } from "react-router-dom";

export default function AccountPage() {
  const { user, profile, userRole, signOut, isAuthenticated } = useOrusAuth();
  const navigate = useNavigate();
  const [isSigningOut, setIsSigningOut] = useState(false);

  const handleSignOut = async () => {
    setIsSigningOut(true);
    try {
      await signOut();
      navigate("/login");
    } catch (error) {
      console.error("Error signing out:", error);
    } finally {
      setIsSigningOut(false);
    }
  };

  // Not authenticated - show login prompt
  if (!isAuthenticated) {
    return (
      <div className="space-y-6">
        <div>
          <h3 className="text-lg font-medium">Account</h3>
          <p className="text-muted-foreground text-sm">
            Sign in to access your account and sync your data across devices.
          </p>
        </div>
        <Separator />

        <Card>
          <CardHeader className="text-center">
            <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-primary/10">
              <Icons.User className="h-8 w-8 text-primary" />
            </div>
            <CardTitle>Sign in to your account</CardTitle>
            <CardDescription>
              Connect your account to sync data and access premium features.
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            <Button onClick={() => navigate("/login")} className="w-full">
              <Icons.ArrowRight className="mr-2 h-4 w-4" />
              Sign In
            </Button>
            <Button variant="outline" onClick={() => navigate("/signup")} className="w-full">
              Create Account
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  // Authenticated - show profile
  const initials = profile?.full_name
    ?.split(" ")
    .map((n) => n[0])
    .join("")
    .toUpperCase()
    .slice(0, 2) || user?.email?.slice(0, 2).toUpperCase() || "??";

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium">Account</h3>
        <p className="text-muted-foreground text-sm">
          Manage your account settings and profile information.
        </p>
      </div>
      <Separator />

      {/* Profile Card */}
      <Card>
        <CardHeader>
          <div className="flex items-center gap-4">
            <Avatar className="h-16 w-16">
              <AvatarImage src={profile?.avatar_url || undefined} alt={profile?.full_name || ""} />
              <AvatarFallback className="text-lg">{initials}</AvatarFallback>
            </Avatar>
            <div className="flex-1">
              <div className="flex items-center gap-2">
                <CardTitle className="text-xl">{profile?.full_name || "User"}</CardTitle>
                {userRole && (
                  <Badge variant={userRole === "admin" || userRole === "superadmin" ? "default" : "secondary"}>
                    {userRole}
                  </Badge>
                )}
              </div>
              <CardDescription>{user?.email}</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <Button variant="outline" onClick={() => navigate("/settings/account/profile")}>
            <Icons.Pencil className="mr-2 h-4 w-4" />
            Edit Profile
          </Button>
        </CardContent>
      </Card>

      {/* Account Details */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">Account Details</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <div className="text-muted-foreground text-sm">Email</div>
              <div className="font-medium">{user?.email || "-"}</div>
            </div>
            <div>
              <div className="text-muted-foreground text-sm">Phone</div>
              <div className="font-medium">{profile?.phone || "-"}</div>
            </div>
            <div>
              <div className="text-muted-foreground text-sm">Company</div>
              <div className="font-medium">{profile?.company || "-"}</div>
            </div>
            <div>
              <div className="text-muted-foreground text-sm">Job Title</div>
              <div className="font-medium">{profile?.job_title || "-"}</div>
            </div>
            <div>
              <div className="text-muted-foreground text-sm">Preferred Currency</div>
              <div className="font-medium">{profile?.preferred_currency || "EUR"}</div>
            </div>
            <div>
              <div className="text-muted-foreground text-sm">Language</div>
              <div className="font-medium">
                {profile?.preferred_language === "fr" ? "Français" : 
                 profile?.preferred_language === "en" ? "English" : 
                 profile?.preferred_language || "Français"}
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Security */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">Security</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <div className="font-medium">Password</div>
              <div className="text-muted-foreground text-sm">Change your password</div>
            </div>
            <Button variant="outline" size="sm" onClick={() => navigate("/settings/account/password")}>
              Change
            </Button>
          </div>
          <Separator />
          <div className="flex items-center justify-between">
            <div>
              <div className="font-medium">Sign out</div>
              <div className="text-muted-foreground text-sm">
                Sign out of your account on this device
              </div>
            </div>
            <Button
              variant="destructive"
              size="sm"
              onClick={handleSignOut}
              disabled={isSigningOut}
            >
              {isSigningOut ? (
                <>
                  <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                  Signing out...
                </>
              ) : (
                <>
                  <Icons.LogOut className="mr-2 h-4 w-4" />
                  Sign Out
                </>
              )}
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Danger Zone */}
      <Card className="border-destructive/50">
        <CardHeader>
          <CardTitle className="text-base text-destructive">Danger Zone</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between">
            <div>
              <div className="font-medium">Delete Account</div>
              <div className="text-muted-foreground text-sm">
                Permanently delete your account and all data
              </div>
            </div>
            <Button variant="outline" size="sm" className="border-destructive text-destructive hover:bg-destructive hover:text-destructive-foreground">
              Delete Account
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
