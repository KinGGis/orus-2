/**
 * ShareholderDocumentsView - Documents tab content
 * Handles document upload and management
 */
import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@wealthfolio/ui/components/ui/card";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { ScrollArea } from "@wealthfolio/ui/components/ui/scroll-area";
import { Separator } from "@wealthfolio/ui/components/ui/separator";
import { EmptyPlaceholder, Icons } from "@wealthfolio/ui";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogFooter,
  DialogClose,
} from "@wealthfolio/ui/components/ui/dialog";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import {
  FileText,
  Upload,
  Clock,
  CheckCircle2,
  XCircle,
  ExternalLink,
  AlertCircle,
  Plus,
  Bell,
} from "lucide-react";

import {
  useShareholder,
  DOCUMENT_TYPES,
} from "@/features/orus-integration";
import type { OrusShareholderDocument, OrusShareholderDocumentRequest } from "@/features/orus-integration";

type DocumentType = (typeof DOCUMENT_TYPES)[number];

const getStatusBadge = (status: string) => {
  switch (status) {
    case "approved":
    case "valid":
      return (
        <Badge variant="secondary" className="bg-green-500/10 text-green-600">
          <CheckCircle2 className="mr-1 h-3 w-3" />
          Validé
        </Badge>
      );
    case "rejected":
      return (
        <Badge variant="secondary" className="bg-red-500/10 text-red-600">
          <XCircle className="mr-1 h-3 w-3" />
          Refusé
        </Badge>
      );
    case "pending":
      return (
        <Badge variant="secondary" className="bg-orange-500/10 text-orange-600">
          <Clock className="mr-1 h-3 w-3" />
          En attente
        </Badge>
      );
    default:
      return (
        <Badge variant="secondary">
          <Clock className="mr-1 h-3 w-3" />
          {status}
        </Badge>
      );
  }
};

// Document card component
function DocumentCard({
  document,
  onView,
}: {
  document: OrusShareholderDocument;
  onView: (doc: OrusShareholderDocument) => void;
}) {
  const typeLabel =
    DOCUMENT_TYPES.find((t: DocumentType) => t.value === document.document_type)?.label ||
    document.document_type;

  return (
    <div className="flex items-center justify-between rounded-lg border p-3">
      <div className="flex items-center gap-3">
        <div className="bg-muted flex h-10 w-10 items-center justify-center rounded-lg">
          <FileText className="text-muted-foreground h-5 w-5" />
        </div>
        <div>
          <p className="text-sm font-medium">{typeLabel}</p>
          <p className="text-muted-foreground text-xs">
            Envoyé le {document.uploaded_at ? format(new Date(document.uploaded_at), "dd MMM yyyy", { locale: fr }) : "-"}
          </p>
        </div>
      </div>
      <div className="flex items-center gap-2">
        {getStatusBadge(document.status)}
        {document.file_path && (
          <Button variant="ghost" size="sm" onClick={() => onView(document)}>
            <ExternalLink className="h-4 w-4" />
          </Button>
        )}
      </div>
    </div>
  );
}

// Document request card component
function DocumentRequestCard({
  request,
  onUpload,
  isUploading,
}: {
  request: OrusShareholderDocumentRequest;
  onUpload: (requestId: string) => void;
  isUploading: boolean;
}) {
  const typeLabel =
    DOCUMENT_TYPES.find((t: DocumentType) => t.value === request.document_type)?.label ||
    request.document_type;

  return (
    <div className="flex items-center justify-between rounded-lg border border-orange-500/30 bg-orange-500/5 p-3">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-orange-500/10">
          <AlertCircle className="h-5 w-5 text-orange-500" />
        </div>
        <div>
          <p className="text-sm font-medium text-orange-600">{typeLabel}</p>
          <p className="text-muted-foreground text-xs">
            Demandé le {request.created_at ? format(new Date(request.created_at), "dd MMM yyyy", { locale: fr }) : "-"}
            {request.due_date && (
              <span> • Avant le {format(new Date(request.due_date), "dd MMM yyyy", { locale: fr })}</span>
            )}
          </p>
          {request.description && (
            <p className="text-muted-foreground mt-1 text-xs italic">{request.description}</p>
          )}
        </div>
      </div>
      <Button
        variant="default"
        size="sm"
        onClick={() => onUpload(request.id)}
        disabled={isUploading}
      >
        <Upload className="mr-1.5 h-4 w-4" />
        Envoyer
      </Button>
    </div>
  );
}

export function ShareholderDocumentsView() {
  const { documents, documentRequests, participation, uploadDocument, getDocumentUrl, loading } = useShareholder();
  const [uploading, setUploading] = useState<string | null>(null);
  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const [selectedDocType, setSelectedDocType] = useState<string>("");
  const [selectedFile, setSelectedFile] = useState<File | null>(null);

  // Filter pending requests
  const pendingRequests = documentRequests.filter(
    (r: OrusShareholderDocumentRequest) => r.status === "pending",
  );

  // Group documents by status
  const validDocuments = documents.filter(
    (d: OrusShareholderDocument) => d.status === "approved" || d.status === "valid",
  );
  const pendingDocuments = documents.filter(
    (d: OrusShareholderDocument) => d.status === "pending",
  );
  const rejectedDocuments = documents.filter(
    (d: OrusShareholderDocument) => d.status === "rejected",
  );

  const handleUpload = async (requestId: string) => {
    if (!participation) return;

    // Create file input and trigger click
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".pdf,.jpg,.jpeg,.png";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;

      setUploading(requestId);
      try {
        const request = documentRequests.find(
          (r: OrusShareholderDocumentRequest) => r.id === requestId,
        );
        if (!request) return;

        await uploadDocument(file, request.document_type);
      } catch (error) {
        console.error("Error uploading document:", error);
      } finally {
        setUploading(null);
      }
    };
    input.click();
  };

  // Proactive document upload (not in response to a request)
  const handleProactiveUpload = async () => {
    if (!selectedFile || !selectedDocType || !participation) return;

    setUploading("proactive");
    try {
      await uploadDocument(selectedFile, selectedDocType);
      setIsAddDialogOpen(false);
      setSelectedFile(null);
      setSelectedDocType("");
    } catch (error) {
      console.error("Error uploading document:", error);
    } finally {
      setUploading(null);
    }
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      setSelectedFile(file);
    }
  };

  const handleViewDocument = async (doc: OrusShareholderDocument) => {
    if (doc.file_path) {
      const url = await getDocumentUrl(doc.file_path);
      if (url) {
        window.open(url, "_blank");
      }
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

  return (
    <div className="space-y-4">
      {/* Pending Requests Section - Alert Style */}
      {pendingRequests.length > 0 && (
        <Card className="border-orange-500/50 bg-orange-500/5">
          <CardHeader className="pb-3">
            <CardTitle className="flex items-center gap-2 text-base text-orange-600">
              <Bell className="h-5 w-5" />
              Documents demandés par l'administrateur
            </CardTitle>
            <CardDescription>
              Veuillez soumettre les documents suivants pour compléter votre dossier
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              {pendingRequests.map((request: OrusShareholderDocumentRequest) => (
                <DocumentRequestCard
                  key={request.id}
                  request={request}
                  onUpload={handleUpload}
                  isUploading={uploading === request.id}
                />
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Documents Section with Add Button */}
      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <div>
              <CardTitle className="flex items-center gap-2 text-base">
                <FileText className="h-5 w-5" />
                Mes documents
              </CardTitle>
              <CardDescription>
                Historique de vos documents KYC
              </CardDescription>
            </div>
            <Dialog open={isAddDialogOpen} onOpenChange={setIsAddDialogOpen}>
              <DialogTrigger asChild>
                <Button variant="outline" size="sm">
                  <Plus className="mr-1.5 h-4 w-4" />
                  Ajouter un document
                </Button>
              </DialogTrigger>
              <DialogContent>
                <DialogHeader>
                  <DialogTitle>Ajouter un document</DialogTitle>
                  <DialogDescription>
                    Sélectionnez le type de document et téléchargez votre fichier
                  </DialogDescription>
                </DialogHeader>
                <div className="space-y-4 py-4">
                  <div className="space-y-2">
                    <label className="text-sm font-medium">Type de document</label>
                    <Select value={selectedDocType} onValueChange={setSelectedDocType}>
                      <SelectTrigger>
                        <SelectValue placeholder="Sélectionner un type" />
                      </SelectTrigger>
                      <SelectContent>
                        {DOCUMENT_TYPES.map((docType: DocumentType) => (
                          <SelectItem key={docType.value} value={docType.value}>
                            {docType.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">Fichier</label>
                    <div className="flex items-center gap-2">
                      <input
                        type="file"
                        accept=".pdf,.jpg,.jpeg,.png"
                        onChange={handleFileSelect}
                        className="hidden"
                        id="file-upload"
                      />
                      <label
                        htmlFor="file-upload"
                        className="bg-muted hover:bg-muted/80 flex h-10 flex-1 cursor-pointer items-center justify-center rounded-md border border-dashed px-4 text-sm transition-colors"
                      >
                        {selectedFile ? (
                          <span className="truncate">{selectedFile.name}</span>
                        ) : (
                          <span className="text-muted-foreground">
                            <Upload className="mr-2 inline h-4 w-4" />
                            Choisir un fichier (PDF, JPG, PNG)
                          </span>
                        )}
                      </label>
                    </div>
                  </div>
                </div>
                <DialogFooter>
                  <DialogClose asChild>
                    <Button variant="outline">Annuler</Button>
                  </DialogClose>
                  <Button
                    onClick={handleProactiveUpload}
                    disabled={!selectedFile || !selectedDocType || uploading === "proactive"}
                  >
                    {uploading === "proactive" ? (
                      <>
                        <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" />
                        Envoi...
                      </>
                    ) : (
                      <>
                        <Upload className="mr-2 h-4 w-4" />
                        Envoyer
                      </>
                    )}
                  </Button>
                </DialogFooter>
              </DialogContent>
            </Dialog>
          </div>
        </CardHeader>
        <CardContent>
          {documents.length === 0 ? (
            <div className="flex items-center justify-center py-8">
              <EmptyPlaceholder
                icon={<FileText className="text-muted-foreground h-10 w-10" />}
                title="Aucun document"
                description="Cliquez sur « Ajouter un document » pour envoyer vos premiers documents."
              />
            </div>
          ) : (
            <ScrollArea className="h-[400px] pr-4">
              <div className="space-y-4">
                {/* Valid documents */}
                {validDocuments.length > 0 && (
                  <div className="space-y-2">
                    <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
                      Documents validés
                    </p>
                    {validDocuments.map((doc: OrusShareholderDocument) => (
                      <DocumentCard key={doc.id} document={doc} onView={handleViewDocument} />
                    ))}
                  </div>
                )}

                {validDocuments.length > 0 && (pendingDocuments.length > 0 || rejectedDocuments.length > 0) && (
                  <Separator />
                )}

                {/* Pending documents */}
                {pendingDocuments.length > 0 && (
                  <div className="space-y-2">
                    <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
                      En cours de validation
                    </p>
                    {pendingDocuments.map((doc: OrusShareholderDocument) => (
                      <DocumentCard key={doc.id} document={doc} onView={handleViewDocument} />
                    ))}
                  </div>
                )}

                {pendingDocuments.length > 0 && rejectedDocuments.length > 0 && <Separator />}

                {/* Rejected documents */}
                {rejectedDocuments.length > 0 && (
                  <div className="space-y-2">
                    <p className="text-muted-foreground text-xs font-medium uppercase tracking-wider">
                      Documents refusés
                    </p>
                    {rejectedDocuments.map((doc: OrusShareholderDocument) => (
                      <DocumentCard key={doc.id} document={doc} onView={handleViewDocument} />
                    ))}
                  </div>
                )}
              </div>
            </ScrollArea>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

export default ShareholderDocumentsView;
