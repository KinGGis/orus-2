/**
 * ShareholderProfileView - Profile/KYC tab content
 * Shows profile information and compliance status
 */
import { useState } from "react";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@wealthfolio/ui/components/ui/card";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Input } from "@wealthfolio/ui/components/ui/input";
import { Label } from "@wealthfolio/ui/components/ui/label";
import { Separator } from "@wealthfolio/ui/components/ui/separator";
import { EmptyPlaceholder, Icons } from "@wealthfolio/ui";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import {
  User,
  Mail,
  Phone,
  MapPin,
  CheckCircle2,
  AlertCircle,
  Save,
} from "lucide-react";

import { useShareholder } from "@/features/orus-integration";
import type { OrusShareholderComplianceInfo } from "@/features/orus-integration";

// Status badge component
function VerificationBadge({ isVerified }: { isVerified: boolean }) {
  if (isVerified) {
    return (
      <Badge variant="secondary" className="bg-green-500/10 text-green-600">
        <CheckCircle2 className="mr-1 h-3 w-3" />
        Vérifié
      </Badge>
    );
  }
  return (
    <Badge variant="secondary" className="bg-orange-500/10 text-orange-600">
      <AlertCircle className="mr-1 h-3 w-3" />
      Non vérifié
    </Badge>
  );
}

// Info row component
function InfoRow({
  icon: Icon,
  label,
  value,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  value: string | null | undefined;
}) {
  return (
    <div className="flex items-center gap-3 py-2">
      <div className="bg-muted flex h-8 w-8 items-center justify-center rounded-lg">
        <Icon className="text-muted-foreground h-4 w-4" />
      </div>
      <div className="flex-1">
        <p className="text-muted-foreground text-xs">{label}</p>
        <p className="text-sm font-medium">{value || "-"}</p>
      </div>
    </div>
  );
}

export function ShareholderProfileView() {
  const { user, complianceInfo, loading, updateComplianceInfo } = useShareholder();
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [formData, setFormData] = useState({
    first_name: "",
    last_name: "",
    phone: "",
    address_line1: "",
    address_line2: "",
    postal_code: "",
    city: "",
    country: "",
  });

  // Initialize form when compliance data loads
  const initForm = () => {
    if (complianceInfo) {
      setFormData({
        first_name: complianceInfo.first_name || "",
        last_name: complianceInfo.last_name || "",
        phone: complianceInfo.phone || "",
        address_line1: complianceInfo.address_line1 || "",
        address_line2: complianceInfo.address_line2 || "",
        postal_code: complianceInfo.postal_code || "",
        city: complianceInfo.city || "",
        country: complianceInfo.country || "",
      });
    }
  };

  const handleEdit = () => {
    initForm();
    setIsEditing(true);
  };

  const handleCancel = () => {
    setIsEditing(false);
  };

  const handleSave = async () => {
    setIsSaving(true);
    try {
      updateComplianceInfo(formData as Partial<OrusShareholderComplianceInfo>);
      setIsEditing(false);
    } catch (error) {
      console.error("Error updating profile:", error);
    } finally {
      setIsSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="space-y-4">
        <Card>
          <CardContent className="p-6">
            <div className="flex items-center justify-center">
              <Icons.Spinner className="text-muted-foreground h-8 w-8 animate-spin" />
            </div>
          </CardContent>
        </Card>
      </div>
    );
  }

  if (!user) {
    return (
      <div className="flex items-center justify-center py-16">
        <EmptyPlaceholder
          icon={<User className="text-muted-foreground h-10 w-10" />}
          title="Profil non trouvé"
          description="Veuillez vous connecter pour accéder à votre profil."
        />
      </div>
    );
  }

  // Build address string from components
  const fullAddress = complianceInfo
    ? [
        complianceInfo.address_line1,
        complianceInfo.address_line2,
        [complianceInfo.postal_code, complianceInfo.city].filter(Boolean).join(" "),
        complianceInfo.country,
      ]
        .filter(Boolean)
        .join(", ")
    : null;

  return (
    <div className="space-y-4">
      {/* Verification Status Card */}
      <Card className="p-3 sm:p-3.5">
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-3">
            <div className="bg-muted flex h-10 w-10 items-center justify-center rounded-lg">
              <User className="text-muted-foreground h-5 w-5" />
            </div>
            <div>
              <p className="text-sm font-medium">Statut de vérification</p>
              <p className="text-muted-foreground text-xs">
                {complianceInfo?.verified_at
                  ? `Vérifié le ${format(new Date(complianceInfo.verified_at), "dd MMM yyyy", { locale: fr })}`
                  : complianceInfo?.updated_at
                    ? `Dernière maj: ${format(new Date(complianceInfo.updated_at), "dd MMM yyyy", { locale: fr })}`
                    : "Aucune information enregistrée"}
              </p>
            </div>
          </div>
          <VerificationBadge isVerified={complianceInfo?.is_verified ?? false} />
        </div>
      </Card>

      {/* Profile Information */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        {/* Personal Info */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="flex items-center gap-2 text-base">
              <User className="h-5 w-5" />
              Informations personnelles
            </CardTitle>
            <CardDescription>Vos coordonnées de contact</CardDescription>
          </CardHeader>
          <CardContent>
            {isEditing ? (
              <div className="space-y-4">
                <div className="grid grid-cols-2 gap-4">
                  <div className="space-y-2">
                    <Label htmlFor="first_name">Prénom</Label>
                    <Input
                      id="first_name"
                      value={formData.first_name}
                      onChange={(e) =>
                        setFormData({ ...formData, first_name: e.target.value })
                      }
                      placeholder="Jean"
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="last_name">Nom</Label>
                    <Input
                      id="last_name"
                      value={formData.last_name}
                      onChange={(e) =>
                        setFormData({ ...formData, last_name: e.target.value })
                      }
                      placeholder="Dupont"
                    />
                  </div>
                </div>
                <div className="space-y-2">
                  <Label htmlFor="email">Email</Label>
                  <Input
                    id="email"
                    value={user.email || ""}
                    disabled
                    className="bg-muted"
                  />
                  <p className="text-muted-foreground text-xs">
                    L'email ne peut pas être modifié ici
                  </p>
                </div>
                <div className="space-y-2">
                  <Label htmlFor="phone">Téléphone</Label>
                  <Input
                    id="phone"
                    value={formData.phone}
                    onChange={(e) =>
                      setFormData({ ...formData, phone: e.target.value })
                    }
                    placeholder="+33 6 12 34 56 78"
                  />
                </div>
              </div>
            ) : (
              <div className="space-y-1">
                <InfoRow
                  icon={User}
                  label="Nom complet"
                  value={
                    complianceInfo?.first_name || complianceInfo?.last_name
                      ? `${complianceInfo.first_name || ""} ${complianceInfo.last_name || ""}`.trim()
                      : null
                  }
                />
                <Separator />
                <InfoRow icon={Mail} label="Email" value={user.email} />
                <Separator />
                <InfoRow icon={Phone} label="Téléphone" value={complianceInfo?.phone} />
              </div>
            )}
          </CardContent>
        </Card>

        {/* Address Info */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="flex items-center gap-2 text-base">
              <MapPin className="h-5 w-5" />
              Adresse
            </CardTitle>
            <CardDescription>Votre adresse postale</CardDescription>
          </CardHeader>
          <CardContent>
            {isEditing ? (
              <div className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="address_line1">Adresse</Label>
                  <Input
                    id="address_line1"
                    value={formData.address_line1}
                    onChange={(e) =>
                      setFormData({ ...formData, address_line1: e.target.value })
                    }
                    placeholder="123 rue Example"
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="address_line2">Complément</Label>
                  <Input
                    id="address_line2"
                    value={formData.address_line2}
                    onChange={(e) =>
                      setFormData({ ...formData, address_line2: e.target.value })
                    }
                    placeholder="Bâtiment A, Étage 2"
                  />
                </div>
                <div className="grid grid-cols-2 gap-4">
                  <div className="space-y-2">
                    <Label htmlFor="postal_code">Code postal</Label>
                    <Input
                      id="postal_code"
                      value={formData.postal_code}
                      onChange={(e) =>
                        setFormData({ ...formData, postal_code: e.target.value })
                      }
                      placeholder="75001"
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="city">Ville</Label>
                    <Input
                      id="city"
                      value={formData.city}
                      onChange={(e) =>
                        setFormData({ ...formData, city: e.target.value })
                      }
                      placeholder="Paris"
                    />
                  </div>
                </div>
                <div className="space-y-2">
                  <Label htmlFor="country">Pays</Label>
                  <Input
                    id="country"
                    value={formData.country}
                    onChange={(e) =>
                      setFormData({ ...formData, country: e.target.value })
                    }
                    placeholder="France"
                  />
                </div>
              </div>
            ) : (
              <div className="space-y-1">
                <InfoRow icon={MapPin} label="Adresse complète" value={fullAddress} />
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Action Buttons */}
      <div className="flex justify-end gap-2">
        {isEditing ? (
          <>
            <Button variant="outline" onClick={handleCancel} disabled={isSaving}>
              Annuler
            </Button>
            <Button onClick={handleSave} disabled={isSaving}>
              {isSaving ? (
                <Icons.Spinner className="mr-1.5 h-4 w-4 animate-spin" />
              ) : (
                <Save className="mr-1.5 h-4 w-4" />
              )}
              Enregistrer
            </Button>
          </>
        ) : (
          <Button variant="outline" onClick={handleEdit}>
            Modifier mes informations
          </Button>
        )}
      </div>
    </div>
  );
}

export default ShareholderProfileView;
