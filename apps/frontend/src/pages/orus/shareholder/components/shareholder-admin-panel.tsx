/**
 * ShareholderAdminPanel - Admin panel for managing shareholders
 * Based on Orus ShareholderManagementPanel.tsx
 */
import { useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@wealthfolio/ui/components/ui/card";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Input } from "@wealthfolio/ui/components/ui/input";
import { Label } from "@wealthfolio/ui/components/ui/label";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Textarea } from "@wealthfolio/ui/components/ui/textarea";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@wealthfolio/ui/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@wealthfolio/ui/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui/components/ui/select";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@wealthfolio/ui/components/ui/accordion";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@wealthfolio/ui/components/ui/collapsible";
import { formatCurrency } from "@/lib/format";
import { format } from "date-fns";
// date-fns locale removed - using default format
import {
  Users,
  Edit,
  FileText,
  Send,
  Eye,
  Download,
  UserPlus,
  AlertCircle,
  Check,
  ChevronDown,
} from "lucide-react";

import { useShareholderAdmin, DOCUMENT_TYPES } from "@/features/orus-integration";
import type { OrusShareholderWithProfile, OrusShareholderParticipation, OrusShareholderDocumentRequest, OrusShareholderDocument } from "@/features/orus-integration";

type DocumentType = typeof DOCUMENT_TYPES[number];

// =====================================================
// Edit Participation Dialog
// =====================================================

interface EditParticipationDialogProps {
  shareholder: OrusShareholderWithProfile;
  onSave: (data: Partial<OrusShareholderParticipation> & { id: string }) => void;
}

function EditParticipationDialog({ shareholder, onSave }: EditParticipationDialogProps) {
  const [open, setOpen] = useState(false);
  const [formData, setFormData] = useState({
    shares_count: shareholder.participation.shares_count,
    share_value: shareholder.participation.share_value,
    currency: shareholder.participation.currency,
    investment_date: shareholder.participation.investment_date || "",
    notes: shareholder.participation.notes || "",
  });

  const handleSave = () => {
    onSave({ ...formData, id: shareholder.participation.id });
    setOpen(false);
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="ghost" size="icon">
          <Edit className="h-4 w-4" />
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Modifier la participation</DialogTitle>
          <DialogDescription>
            {shareholder.profile?.full_name || shareholder.profile?.email}
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4 py-4">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label>Nombre de parts</Label>
              <Input
                type="number"
                value={formData.shares_count}
                onChange={(e) =>
                  setFormData((prev) => ({ ...prev, shares_count: parseFloat(e.target.value) }))
                }
              />
            </div>
            <div className="space-y-2">
              <Label>Valeur par part</Label>
              <Input
                type="number"
                step="0.01"
                value={formData.share_value}
                onChange={(e) =>
                  setFormData((prev) => ({ ...prev, share_value: parseFloat(e.target.value) }))
                }
              />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label>Devise</Label>
              <Select
                value={formData.currency}
                onValueChange={(value) => setFormData((prev) => ({ ...prev, currency: value }))}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="EUR">EUR</SelectItem>
                  <SelectItem value="USD">USD</SelectItem>
                  <SelectItem value="GBP">GBP</SelectItem>
                  <SelectItem value="XOF">XOF</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label>Date d'investissement</Label>
              <Input
                type="date"
                value={formData.investment_date}
                onChange={(e) =>
                  setFormData((prev) => ({ ...prev, investment_date: e.target.value }))
                }
              />
            </div>
          </div>
          <div className="space-y-2">
            <Label>Notes</Label>
            <Textarea
              value={formData.notes}
              onChange={(e) => setFormData((prev) => ({ ...prev, notes: e.target.value }))}
              placeholder="Notes internes..."
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)}>
            Annuler
          </Button>
          <Button onClick={handleSave}>Enregistrer</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================
// Request Document Dialog
// =====================================================

interface RequestDocumentDialogProps {
  shareholder: OrusShareholderWithProfile;
  onRequest: (data: { userId: string; documentType: string; description?: string; dueDate?: string }) => void;
}

function RequestDocumentDialog({ shareholder, onRequest }: RequestDocumentDialogProps) {
  const [open, setOpen] = useState(false);
  const [docType, setDocType] = useState("");
  const [description, setDescription] = useState("");
  const [dueDate, setDueDate] = useState("");

  const handleRequest = () => {
    onRequest({
      userId: shareholder.participation.user_id,
      documentType: docType,
      description: description || undefined,
      dueDate: dueDate || undefined,
    });
    setOpen(false);
    setDocType("");
    setDescription("");
    setDueDate("");
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline" size="sm">
          <Send className="mr-2 h-4 w-4" />
          Demander
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Demander un document</DialogTitle>
          <DialogDescription>
            Envoyer une demande à{" "}
            {shareholder.profile?.full_name || shareholder.profile?.email}
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4 py-4">
          <div className="space-y-2">
            <Label>Type de document</Label>
            <Select value={docType} onValueChange={setDocType}>
              <SelectTrigger>
                <SelectValue placeholder="Sélectionner..." />
              </SelectTrigger>
              <SelectContent>
                {DOCUMENT_TYPES.map((type: DocumentType) => (
                  <SelectItem key={type.value} value={type.value}>
                    {type.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-2">
            <Label>Description (optionnel)</Label>
            <Textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Précisez les informations nécessaires..."
            />
          </div>
          <div className="space-y-2">
            <Label>Date limite (optionnel)</Label>
            <Input type="date" value={dueDate} onChange={(e) => setDueDate(e.target.value)} />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)}>
            Annuler
          </Button>
          <Button onClick={handleRequest} disabled={!docType}>
            <Send className="mr-2 h-4 w-4" />
            Envoyer
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================
// Add Shareholder Dialog
// =====================================================

interface AddShareholderDialogProps {
  onAdd: (data: Omit<OrusShareholderParticipation, 'id' | 'total_value' | 'created_at' | 'updated_at'>) => void;
}

function AddShareholderDialog({ onAdd }: AddShareholderDialogProps) {
  const [open, setOpen] = useState(false);
  const [formData, setFormData] = useState({
    userId: "",
    shares_count: 0,
    share_value: 100,
    currency: "EUR",
    investment_date: "",
    notes: "",
  });

  const handleAdd = () => {
    if (!formData.userId) return;

    onAdd({
      user_id: formData.userId,
      shares_count: formData.shares_count,
      share_value: formData.share_value,
      currency: formData.currency,
      investment_date: formData.investment_date || null,
      last_valuation_date: new Date().toISOString().split('T')[0],
      notes: formData.notes || null,
    });

    setOpen(false);
    setFormData({
      userId: "",
      shares_count: 0,
      share_value: 100,
      currency: "EUR",
      investment_date: "",
      notes: "",
    });
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>
          <UserPlus className="mr-2 h-4 w-4" />
          Ajouter
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Ajouter un actionnaire</DialogTitle>
          <DialogDescription>
            Entrez l'ID utilisateur et les informations de participation
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4 py-4">
          <div className="space-y-2">
            <Label>User ID *</Label>
            <Input
              value={formData.userId}
              onChange={(e) => setFormData((prev) => ({ ...prev, userId: e.target.value }))}
              placeholder="UUID de l'utilisateur"
            />
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label>Nombre de parts</Label>
              <Input
                type="number"
                value={formData.shares_count}
                onChange={(e) =>
                  setFormData((prev) => ({
                    ...prev,
                    shares_count: parseFloat(e.target.value) || 0,
                  }))
                }
              />
            </div>
            <div className="space-y-2">
              <Label>Valeur par part</Label>
              <Input
                type="number"
                step="0.01"
                value={formData.share_value}
                onChange={(e) =>
                  setFormData((prev) => ({
                    ...prev,
                    share_value: parseFloat(e.target.value) || 0,
                  }))
                }
              />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label>Devise</Label>
              <Select
                value={formData.currency}
                onValueChange={(value) => setFormData((prev) => ({ ...prev, currency: value }))}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="EUR">EUR</SelectItem>
                  <SelectItem value="USD">USD</SelectItem>
                  <SelectItem value="GBP">GBP</SelectItem>
                  <SelectItem value="XOF">XOF</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label>Date d'investissement</Label>
              <Input
                type="date"
                value={formData.investment_date}
                onChange={(e) =>
                  setFormData((prev) => ({ ...prev, investment_date: e.target.value }))
                }
              />
            </div>
          </div>
          <div className="space-y-2">
            <Label>Notes (optionnel)</Label>
            <Textarea
              value={formData.notes}
              onChange={(e) => setFormData((prev) => ({ ...prev, notes: e.target.value }))}
              placeholder="Notes internes..."
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)}>
            Annuler
          </Button>
          <Button onClick={handleAdd} disabled={!formData.userId}>
            Ajouter
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// =====================================================
// Main Admin Panel Component
// =====================================================

export function ShareholderAdminPanel() {
  const [isOpen, setIsOpen] = useState(false);
  const {
    shareholders,
    loading,
    isAdmin,
    isSuperadmin,
    updateParticipation,
    createParticipation,
    requestDocument,
    validateDocumentRequest,
    getDocumentUrl,
  } = useShareholderAdmin();

  const handleDownloadDocument = async (filePath: string) => {
    const url = await getDocumentUrl(filePath);
    if (url) {
      window.open(url, "_blank");
    }
  };

  if (!isAdmin) return null;

  const totalValue = shareholders.reduce(
    (sum: number, s: OrusShareholderWithProfile) => sum + (s.participation.total_value || 0),
    0
  );
  const totalShares = shareholders.reduce(
    (sum: number, s: OrusShareholderWithProfile) => sum + s.participation.shares_count,
    0
  );

  return (
    <Collapsible open={isOpen} onOpenChange={setIsOpen}>
      <Card>
        <CollapsibleTrigger asChild>
          <CardHeader className="cursor-pointer rounded-t-lg transition-colors hover:bg-muted/30">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Users className="h-5 w-5" />
                <div>
                  <CardTitle className="text-left">Gestion des Actionnaires</CardTitle>
                  <CardDescription className="text-left">
                    {shareholders.length} actionnaire(s) • {formatCurrency(totalValue, "EUR")} total
                    • {totalShares.toLocaleString()} parts
                  </CardDescription>
                </div>
              </div>
              <div className="flex items-center gap-2">
                {isSuperadmin && (
                  <div onClick={(e) => e.stopPropagation()}>
                    <AddShareholderDialog onAdd={createParticipation} />
                  </div>
                )}
                <ChevronDown
                  className={`text-muted-foreground h-5 w-5 transition-transform duration-200 ${
                    isOpen ? "rotate-180" : ""
                  }`}
                />
              </div>
            </div>
          </CardHeader>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <CardContent>
            {loading ? (
              <div className="py-8 text-center">Chargement...</div>
            ) : shareholders.length === 0 ? (
              <div className="text-muted-foreground py-8 text-center">
                <Users className="mx-auto mb-4 h-12 w-12 opacity-50" />
                <p>Aucun actionnaire enregistré</p>
                {isSuperadmin && (
                  <div className="mt-4">
                    <AddShareholderDialog onAdd={createParticipation} />
                  </div>
                )}
              </div>
            ) : (
              <div className="space-y-4">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Actionnaire</TableHead>
                      <TableHead className="text-right">Parts</TableHead>
                      <TableHead className="text-right">Valeur/Part</TableHead>
                      <TableHead className="text-right">Total</TableHead>
                      <TableHead>Statut</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {shareholders.map((shareholder: OrusShareholderWithProfile) => (
                      <TableRow key={shareholder.participation.id}>
                        <TableCell>
                          <div>
                            <p className="font-medium">
                              {shareholder.profile?.full_name || "N/A"}
                            </p>
                            <p className="text-muted-foreground text-sm">
                              {shareholder.profile?.email}
                            </p>
                          </div>
                        </TableCell>
                        <TableCell className="text-right font-mono">
                          {shareholder.participation.shares_count.toLocaleString()}
                        </TableCell>
                        <TableCell className="text-right font-mono">
                          {formatCurrency(
                            shareholder.participation.share_value,
                            shareholder.participation.currency
                          )}
                        </TableCell>
                        <TableCell className="text-right font-mono font-medium">
                          {formatCurrency(
                            shareholder.participation.total_value,
                            shareholder.participation.currency
                          )}
                        </TableCell>
                        <TableCell>
                          <div className="flex items-center gap-2">
                            {(shareholder.pendingRequests || 0) > 0 ? (
                              <Badge variant="destructive">
                                <AlertCircle className="mr-1 h-3 w-3" />
                                {shareholder.pendingRequests} en attente
                              </Badge>
                            ) : (
                              <Badge variant="secondary">
                                <Check className="mr-1 h-3 w-3" />
                                Conforme
                              </Badge>
                            )}
                          </div>
                          {/* Pending requests with validate buttons */}
                          {shareholder.documentRequests &&
                            shareholder.documentRequests.filter((r: OrusShareholderDocumentRequest) => r.status === "pending")
                              .length > 0 && (
                              <div className="mt-2 space-y-1">
                                {shareholder.documentRequests
                                  .filter((r: OrusShareholderDocumentRequest) => r.status === "pending")
                                  .map((req: OrusShareholderDocumentRequest) => (
                                    <div key={req.id} className="flex items-center gap-2">
                                      <span className="text-muted-foreground max-w-[120px] truncate text-xs">
                                        {DOCUMENT_TYPES.find(
                                          (t: DocumentType) => t.value === req.document_type
                                        )?.label || req.document_type}
                                      </span>
                                      <Button
                                        variant="outline"
                                        size="sm"
                                        className="h-6 px-2 text-xs"
                                        onClick={() => validateDocumentRequest({ requestId: req.id, status: 'fulfilled' })}
                                      >
                                        <Check className="mr-1 h-3 w-3" />
                                        Valider
                                      </Button>
                                    </div>
                                  ))}
                              </div>
                            )}
                        </TableCell>
                        <TableCell className="text-right">
                          <div className="flex items-center justify-end gap-2">
                            <EditParticipationDialog
                              shareholder={shareholder}
                              onSave={updateParticipation}
                            />
                            <RequestDocumentDialog
                              shareholder={shareholder}
                              onRequest={requestDocument}
                            />
                          </div>
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>

                {/* Superadmin: Detailed view with compliance info and documents */}
                {isSuperadmin && (
                  <div className="mt-6">
                    <h3 className="mb-4 font-medium">Détails Compliance (Superadmin)</h3>
                    <Accordion type="single" collapsible className="space-y-2">
                      {shareholders.map((shareholder: OrusShareholderWithProfile) => (
                        <AccordionItem
                          key={shareholder.participation.id}
                          value={shareholder.participation.id}
                        >
                          <AccordionTrigger>
                            <div className="flex items-center gap-4">
                              <span>
                                {shareholder.profile?.full_name || shareholder.profile?.email}
                              </span>
                              {shareholder.documents && shareholder.documents.length > 0 && (
                                <Badge variant="outline">
                                  {shareholder.documents.length} document(s)
                                </Badge>
                              )}
                            </div>
                          </AccordionTrigger>
                          <AccordionContent>
                            <div className="grid grid-cols-1 gap-6 p-4 md:grid-cols-2">
                              {/* Compliance Info */}
                              <div className="space-y-4">
                                <h4 className="flex items-center gap-2 font-medium">
                                  <Eye className="h-4 w-4" />
                                  Informations personnelles
                                </h4>
                                {shareholder.complianceInfo ? (
                                  <div className="space-y-2 text-sm">
                                    <p>
                                      <strong>Nom:</strong>{" "}
                                      {shareholder.complianceInfo.last_name}{" "}
                                      {shareholder.complianceInfo.first_name}
                                    </p>
                                    <p>
                                      <strong>Adresse:</strong>{" "}
                                      {shareholder.complianceInfo.address_line1}
                                    </p>
                                    {shareholder.complianceInfo.address_line2 && (
                                      <p>{shareholder.complianceInfo.address_line2}</p>
                                    )}
                                    <p>
                                      {shareholder.complianceInfo.postal_code}{" "}
                                      {shareholder.complianceInfo.city},{" "}
                                      {shareholder.complianceInfo.country}
                                    </p>
                                    <p>
                                      <strong>Email:</strong> {shareholder.complianceInfo.email}
                                    </p>
                                    <p>
                                      <strong>Téléphone:</strong>{" "}
                                      {shareholder.complianceInfo.phone || "-"}
                                    </p>
                                  </div>
                                ) : (
                                  <p className="text-muted-foreground text-sm">
                                    Aucune information renseignée
                                  </p>
                                )}
                              </div>

                              {/* Documents */}
                              <div className="space-y-4">
                                <h4 className="flex items-center gap-2 font-medium">
                                  <FileText className="h-4 w-4" />
                                  Documents
                                </h4>
                                {shareholder.documents && shareholder.documents.length > 0 ? (
                                  <div className="space-y-2">
                                    {shareholder.documents.map((doc: OrusShareholderDocument) => (
                                      <div
                                        key={doc.id}
                                        className="flex items-center justify-between rounded border p-2"
                                      >
                                        <div>
                                          <p className="text-sm font-medium">{doc.document_name}</p>
                                          <p className="text-muted-foreground text-xs">
                                            {DOCUMENT_TYPES.find(
                                              (t: DocumentType) => t.value === doc.document_type
                                            )?.label || doc.document_type}{" "}
                                            •{" "}
                                            {doc.uploaded_at
                                              ? format(new Date(doc.uploaded_at), "dd/MM/yyyy")
                                              : "-"}
                                          </p>
                                        </div>
                                        <Button
                                          variant="ghost"
                                          size="icon"
                                          onClick={() => handleDownloadDocument(doc.file_path)}
                                        >
                                          <Download className="h-4 w-4" />
                                        </Button>
                                      </div>
                                    ))}
                                  </div>
                                ) : (
                                  <p className="text-muted-foreground text-sm">Aucun document</p>
                                )}
                              </div>
                            </div>
                          </AccordionContent>
                        </AccordionItem>
                      ))}
                    </Accordion>
                  </div>
                )}
              </div>
            )}
          </CardContent>
        </CollapsibleContent>
      </Card>
    </Collapsible>
  );
}

export default ShareholderAdminPanel;
