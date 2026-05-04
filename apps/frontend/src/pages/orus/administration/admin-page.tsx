/**
 * Administration Page for Orus Integration
 * Collapsible sections for user management, invitations, permissions, integrations
 */
import { useState } from "react";
import { PageHeader } from "@/components/page";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui/components/ui/card";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@wealthfolio/ui/components/ui/collapsible";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@wealthfolio/ui/components/ui/table";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui/components/ui/select";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@wealthfolio/ui/components/ui/tabs";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Input } from "@wealthfolio/ui/components/ui/input";
import { Label } from "@wealthfolio/ui/components/ui/label";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Switch } from "@wealthfolio/ui/components/ui/switch";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import {
  ChevronDownIcon,
  ChevronUpIcon,
  TrashIcon,
  RefreshCwIcon,
  SendIcon,
  XIcon,
  InfoIcon,
  Loader2Icon,
  ShieldIcon,
  UsersIcon,
  MailIcon,
} from "lucide-react";
import {
  useOrusUsers,
  useUpdateUserRole,
  useDeleteUser,
  useInvitations,
  useSendInvitation,
  useCancelInvitation,
  useResendInvitation,
  useRolePermissions,
  useUpdatePermission,
  ROLE_OPTIONS,
  PERMISSION_RESOURCES,
  getRoleLabel,
  type UserRole,
} from "@/features/orus-integration/admin-hooks";

// =====================================================
// SECTION WRAPPER COMPONENT
// =====================================================

interface SectionProps {
  title: string;
  description?: string;
  icon: React.ReactNode;
  defaultOpen?: boolean;
  children: React.ReactNode;
}

function CollapsibleSection({ title, description, icon, defaultOpen = false, children }: SectionProps) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <Collapsible open={isOpen} onOpenChange={setIsOpen} className="border rounded-lg">
      <CollapsibleTrigger asChild>
        <div className="flex items-center justify-between p-4 cursor-pointer hover:bg-muted/50">
          <div className="flex items-center gap-3">
            {icon}
            <div>
              <h3 className="font-semibold">{title}</h3>
              {description && <p className="text-sm text-muted-foreground">{description}</p>}
            </div>
          </div>
          {isOpen ? <ChevronUpIcon className="h-5 w-5" /> : <ChevronDownIcon className="h-5 w-5" />}
        </div>
      </CollapsibleTrigger>
      <CollapsibleContent className="border-t">
        <div className="p-4">{children}</div>
      </CollapsibleContent>
    </Collapsible>
  );
}

// =====================================================
// SECTION 1: USER MANAGEMENT
// =====================================================

function UserManagementSection() {
  const { data: users, isLoading } = useOrusUsers();
  const updateRole = useUpdateUserRole();
  const deleteUser = useDeleteUser();
  const sendInvitation = useSendInvitation();

  const handleInviteUser = (email: string, role: UserRole) => {
    if (!email) return;
    sendInvitation.mutate({ email, role });
  };

  return (
    <CollapsibleSection
      title="Gestion des utilisateurs"
      description="Gérer les rôles et permissions des utilisateurs"
      icon={<UsersIcon className="h-5 w-5 text-blue-500" />}
    >
      {isLoading ? (
        <div className="space-y-2">
          {[1, 2, 3].map((i) => (
            <Skeleton key={i} className="h-12 w-full" />
          ))}
        </div>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Utilisateur</TableHead>
              <TableHead>Email</TableHead>
              <TableHead>Rôle</TableHead>
              <TableHead className="w-[150px]">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {users?.map((user) => (
              <TableRow key={user.user_id}>
                <TableCell className="font-medium">{user.full_name || "—"}</TableCell>
                <TableCell>{user.email}</TableCell>
                <TableCell>
                  <Select
                    value={user.role}
                    onValueChange={(value) => updateRole.mutate({ userId: user.user_id, role: value as UserRole })}
                    disabled={updateRole.isPending}
                  >
                    <SelectTrigger className="w-[160px]">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {ROLE_OPTIONS.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </TableCell>
                <TableCell>
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon"
                      title="Envoyer invitation"
                      onClick={() => handleInviteUser(user.email, user.role)}
                      disabled={sendInvitation.isPending}
                    >
                      {sendInvitation.isPending ? (
                        <Loader2Icon className="h-4 w-4 animate-spin" />
                      ) : (
                        <SendIcon className="h-4 w-4 text-primary" />
                      )}
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      title="Supprimer"
                      onClick={() => {
                        if (confirm("Êtes-vous sûr de vouloir supprimer cet utilisateur ?")) {
                          deleteUser.mutate(user.user_id);
                        }
                      }}
                      disabled={deleteUser.isPending}
                    >
                      <TrashIcon className="h-4 w-4 text-destructive" />
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            ))}
            {(!users || users.length === 0) && (
              <TableRow>
                <TableCell colSpan={4} className="text-center text-muted-foreground">
                  Aucun utilisateur
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      )}
    </CollapsibleSection>
  );
}

// =====================================================
// SECTION 2: USER INVITATIONS
// =====================================================

function InvitationsSection() {
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<UserRole>("viewer");
  const { data: invitations, isLoading } = useInvitations();
  const sendInvitation = useSendInvitation();
  const cancelInvitation = useCancelInvitation();
  const resendInvitation = useResendInvitation();

  const handleSend = () => {
    if (!email) return;
    sendInvitation.mutate({ email, role }, {
      onSuccess: () => {
        setEmail("");
        setRole("viewer");
      },
    });
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case "pending":
        return <Badge variant="outline" className="text-yellow-600">En attente</Badge>;
      case "accepted":
        return <Badge className="bg-green-600">Acceptée</Badge>;
      case "expired":
        return <Badge variant="secondary">Expirée</Badge>;
      case "cancelled":
        return <Badge variant="destructive">Annulée</Badge>;
      default:
        return <Badge variant="outline">{status}</Badge>;
    }
  };

  return (
    <CollapsibleSection
      title="Invitations"
      description="Inviter de nouveaux utilisateurs"
      icon={<MailIcon className="h-5 w-5 text-green-500" />}
    >
      {/* Invite Form */}
      <div className="flex gap-4 mb-6">
        <div className="flex-1">
          <Label htmlFor="invite-email" className="sr-only">Email</Label>
          <Input
            id="invite-email"
            type="email"
            placeholder="email@exemple.com"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
          />
        </div>
        <Select value={role} onValueChange={(v) => setRole(v as UserRole)}>
          <SelectTrigger className="w-[160px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {ROLE_OPTIONS.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Button onClick={handleSend} disabled={!email || sendInvitation.isPending}>
          {sendInvitation.isPending ? (
            <Loader2Icon className="h-4 w-4 animate-spin" />
          ) : (
            <SendIcon className="h-4 w-4" />
          )}
          <span className="ml-2">Inviter</span>
        </Button>
      </div>

      {/* Invitations List */}
      {isLoading ? (
        <div className="space-y-2">
          {[1, 2].map((i) => (
            <Skeleton key={i} className="h-12 w-full" />
          ))}
        </div>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Email</TableHead>
              <TableHead>Rôle</TableHead>
              <TableHead>Statut</TableHead>
              <TableHead>Invité par</TableHead>
              <TableHead>Date</TableHead>
              <TableHead className="w-[100px]">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {invitations?.map((inv) => (
              <TableRow key={inv.id}>
                <TableCell>{inv.email}</TableCell>
                <TableCell>{getRoleLabel(inv.role)}</TableCell>
                <TableCell>{getStatusBadge(inv.status)}</TableCell>
                <TableCell>{inv.invited_by_email || "—"}</TableCell>
                <TableCell>{new Date(inv.created_at).toLocaleDateString("fr-FR")}</TableCell>
                <TableCell className="flex gap-1">
                  {inv.status === "pending" && (
                    <>
                      <Button
                        variant="ghost"
                        size="icon"
                        title="Renvoyer"
                        onClick={() => resendInvitation.mutate(inv.id)}
                        disabled={resendInvitation.isPending}
                      >
                        <RefreshCwIcon className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        title="Annuler"
                        onClick={() => cancelInvitation.mutate(inv.id)}
                        disabled={cancelInvitation.isPending}
                      >
                        <XIcon className="h-4 w-4" />
                      </Button>
                    </>
                  )}
                </TableCell>
              </TableRow>
            ))}
            {(!invitations || invitations.length === 0) && (
              <TableRow>
                <TableCell colSpan={6} className="text-center text-muted-foreground">
                  Aucune invitation
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      )}
    </CollapsibleSection>
  );
}

// =====================================================
// SECTION 3: PERMISSIONS MANAGEMENT
// =====================================================

function PermissionsSection() {
  const [selectedRole, setSelectedRole] = useState<UserRole>("admin");
  const { data: permissions, isLoading } = useRolePermissions(selectedRole);
  const updatePermission = useUpdatePermission();

  const isAllowed = (resource: string, action: string): boolean => {
    const perm = permissions?.find((p) => p.resource === resource && p.action === action);
    return perm?.allowed ?? false;
  };

  const handleToggle = (resource: string, action: string) => {
    const currentValue = isAllowed(resource, action);
    updatePermission.mutate({
      role: selectedRole,
      resource,
      action,
      allowed: !currentValue,
    });
  };

  return (
    <CollapsibleSection
      title="Gestion des permissions"
      description="Configurer les accès par rôle"
      icon={<ShieldIcon className="h-5 w-5 text-purple-500" />}
    >
      <Tabs value={selectedRole} onValueChange={(v) => setSelectedRole(v as UserRole)} className="space-y-4">
        <TabsList>
          {ROLE_OPTIONS.filter((r) => r.value !== "superadmin").map((option) => (
            <TabsTrigger key={option.value} value={option.value}>
              {option.label}
            </TabsTrigger>
          ))}
        </TabsList>

        <TabsContent value={selectedRole}>
          {isLoading ? (
            <div className="space-y-2">
              {[1, 2, 3].map((i) => (
                <Skeleton key={i} className="h-10 w-full" />
              ))}
            </div>
          ) : (
            <div className="space-y-4">
              {PERMISSION_RESOURCES.map((res) => (
                <div key={res.resource} className="border rounded-lg p-4">
                  <h4 className="font-medium mb-3">{res.label}</h4>
                  <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-4">
                    {res.actions.map((action) => (
                      <div key={action} className="flex items-center gap-2">
                        <Switch
                          id={`${res.resource}-${action}`}
                          checked={isAllowed(res.resource, action)}
                          onCheckedChange={() => handleToggle(res.resource, action)}
                          disabled={updatePermission.isPending}
                        />
                        <Label htmlFor={`${res.resource}-${action}`} className="text-sm">
                          {action}
                        </Label>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </TabsContent>
      </Tabs>
    </CollapsibleSection>
  );
}

// =====================================================
// SECTION 4: IMPORTANT INFORMATION
// =====================================================

function InfoSection() {
  return (
    <CollapsibleSection
      title="Informations importantes"
      description="Description des rôles et leur niveau d'accès"
      icon={<InfoIcon className="h-5 w-5 text-blue-400" />}
    >
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {[
          { role: "Super Admin", desc: "Accès complet à toutes les fonctionnalités, gestion des utilisateurs et permissions" },
          { role: "Admin", desc: "Administration du système, gestion des utilisateurs, configuration des intégrations" },
          { role: "Portfolio Manager", desc: "Gestion complète du portefeuille, positions, rapports" },
          { role: "Trader", desc: "Exécution des ordres, suivi des positions, gestion du risque" },
          { role: "Analyste", desc: "Consultation et analyse des données, génération de rapports" },
          { role: "Lecteur", desc: "Accès en lecture seule au tableau de bord et aux données de base" },
        ].map((item) => (
          <Card key={item.role}>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm">{item.role}</CardTitle>
            </CardHeader>
            <CardContent>
              <CardDescription>{item.desc}</CardDescription>
            </CardContent>
          </Card>
        ))}
      </div>
    </CollapsibleSection>
  );
}

// =====================================================
// MAIN ADMIN PAGE
// =====================================================

export function AdminPage() {
  return (
    <div className="flex flex-col h-full">
      <PageHeader heading="Administration" text="Gestion des utilisateurs, permissions et intégrations" />

      <div className="flex-1 overflow-y-auto p-6 space-y-4">
        <UserManagementSection />
        <InvitationsSection />
        <PermissionsSection />
        <InfoSection />
      </div>
    </div>
  );
}

export default AdminPage;
