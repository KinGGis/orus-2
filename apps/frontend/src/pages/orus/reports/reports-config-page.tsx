/**
 * ReportsConfigPage - Manage automated report configurations
 */
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui/components/ui/card";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Input } from "@wealthfolio/ui/components/ui/input";
import { Label } from "@wealthfolio/ui/components/ui/label";
import { Textarea } from "@wealthfolio/ui/components/ui/textarea";
import { Switch } from "@wealthfolio/ui/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui/components/ui/select";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import {
  ArrowLeft,
  Plus,
  Pencil,
  Trash2,
  Loader2,
  Clock,
  FileText,
} from "lucide-react";

import {
  useReportConfigs,
  useCreateReportConfig,
  useUpdateReportConfig,
  useDeleteReportConfig,
  getReportTypeLabel,
  type ReportConfiguration,
} from "@/features/orus-integration/reports-hooks";

// =====================================================
// TYPES
// =====================================================

type ReportType = ReportConfiguration["report_type"];
type Frequency = ReportConfiguration["frequency"];

interface ConfigFormData {
  name: string;
  report_type: ReportType;
  frequency: Frequency;
  schedule_time: string;
  schedule_day: number;
  n8n_webhook_url: string;
  description: string;
  recipients: string;
  is_enabled: boolean;
}

const DEFAULT_FORM_DATA: ConfigFormData = {
  name: "",
  report_type: "performance",
  frequency: "monthly",
  schedule_time: "18:00",
  schedule_day: 1,
  n8n_webhook_url: "",
  description: "",
  recipients: "",
  is_enabled: true,
};

// =====================================================
// COMPONENTS
// =====================================================

function ConfigForm({
  initialData,
  onSubmit,
  onCancel,
  isSubmitting,
  isEditing = false,
}: {
  initialData?: ConfigFormData;
  onSubmit: (data: ConfigFormData) => void;
  onCancel: () => void;
  isSubmitting: boolean;
  isEditing?: boolean;
}) {
  const [formData, setFormData] = useState<ConfigFormData>(initialData || DEFAULT_FORM_DATA);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit(formData);
  };

  const updateField = <K extends keyof ConfigFormData>(field: K, value: ConfigFormData[K]) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4 p-4 border rounded-lg bg-muted/30">
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {/* Name */}
        <div className="space-y-2">
          <Label htmlFor="name">Nom du rapport *</Label>
          <Input
            id="name"
            value={formData.name}
            onChange={(e) => updateField("name", e.target.value)}
            placeholder="Rapport mensuel de performance"
            required
          />
        </div>

        {/* Report Type */}
        <div className="space-y-2">
          <Label htmlFor="report_type">Type de rapport *</Label>
          <Select
            value={formData.report_type}
            onValueChange={(v) => updateField("report_type", v as ReportType)}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="performance">Performance</SelectItem>
              <SelectItem value="allocation">Allocation</SelectItem>
              <SelectItem value="monthly">Mensuel</SelectItem>
              <SelectItem value="regulatory">Réglementaire</SelectItem>
              <SelectItem value="client_presentation">Présentation Client</SelectItem>
            </SelectContent>
          </Select>
        </div>

        {/* Frequency */}
        <div className="space-y-2">
          <Label htmlFor="frequency">Fréquence *</Label>
          <Select
            value={formData.frequency}
            onValueChange={(v) => updateField("frequency", v as Frequency)}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="daily">Quotidien</SelectItem>
              <SelectItem value="weekly">Hebdomadaire</SelectItem>
              <SelectItem value="monthly">Mensuel</SelectItem>
              <SelectItem value="quarterly">Trimestriel</SelectItem>
            </SelectContent>
          </Select>
        </div>

        {/* Schedule Time */}
        <div className="space-y-2">
          <Label htmlFor="schedule_time">Heure d'exécution</Label>
          <Input
            id="schedule_time"
            type="time"
            value={formData.schedule_time}
            onChange={(e) => updateField("schedule_time", e.target.value)}
          />
        </div>

        {/* Schedule Day (for weekly/monthly) */}
        {(formData.frequency === "weekly" || formData.frequency === "monthly") && (
          <div className="space-y-2">
            <Label htmlFor="schedule_day">
              {formData.frequency === "weekly" ? "Jour de la semaine (1-7)" : "Jour du mois (1-31)"}
            </Label>
            <Input
              id="schedule_day"
              type="number"
              min={1}
              max={formData.frequency === "weekly" ? 7 : 31}
              value={formData.schedule_day}
              onChange={(e) => updateField("schedule_day", parseInt(e.target.value) || 1)}
            />
          </div>
        )}

        {/* N8N Webhook URL */}
        <div className="space-y-2 md:col-span-2">
          <Label htmlFor="n8n_webhook_url">URL Webhook n8n</Label>
          <Input
            id="n8n_webhook_url"
            value={formData.n8n_webhook_url}
            onChange={(e) => updateField("n8n_webhook_url", e.target.value)}
            placeholder="https://your-n8n-instance.com/webhook/..."
          />
        </div>

        {/* Recipients */}
        <div className="space-y-2 md:col-span-2">
          <Label htmlFor="recipients">Destinataires</Label>
          <Input
            id="recipients"
            value={formData.recipients}
            onChange={(e) => updateField("recipients", e.target.value)}
            placeholder="email1@example.com, email2@example.com"
          />
        </div>

        {/* Description */}
        <div className="space-y-2 md:col-span-2">
          <Label htmlFor="description">Description</Label>
          <Textarea
            id="description"
            value={formData.description}
            onChange={(e) => updateField("description", e.target.value)}
            placeholder="Description de la configuration..."
            rows={2}
          />
        </div>

        {/* Enable toggle */}
        <div className="flex items-center gap-3 md:col-span-2">
          <Switch
            id="is_enabled"
            checked={formData.is_enabled}
            onCheckedChange={(checked) => updateField("is_enabled", checked)}
          />
          <Label htmlFor="is_enabled">Activer immédiatement</Label>
        </div>
      </div>

      {/* Actions */}
      <div className="flex justify-end gap-2 pt-4 border-t">
        <Button type="button" variant="outline" onClick={onCancel}>
          Annuler
        </Button>
        <Button type="submit" disabled={isSubmitting || !formData.name}>
          {isSubmitting && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
          {isEditing ? "Mettre à jour" : "Créer"}
        </Button>
      </div>
    </form>
  );
}

function ConfigCard({
  config,
  onEdit,
  onDelete,
  onToggle,
}: {
  config: ReportConfiguration;
  onEdit: () => void;
  onDelete: () => void;
  onToggle: (enabled: boolean) => void;
}) {
  const getFrequencyLabel = (freq: string) => {
    const labels: Record<string, string> = {
      daily: "Quotidien",
      weekly: "Hebdomadaire",
      monthly: "Mensuel",
      quarterly: "Trimestriel",
    };
    return labels[freq] || freq;
  };

  return (
    <Card className="bg-card/50 border-border/50">
      <CardContent className="pt-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <FileText className="h-5 w-5 text-muted-foreground" />
            <div>
              <h4 className="font-medium">{config.name}</h4>
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <Badge variant="outline" className="text-xs">
                  {getReportTypeLabel(config.report_type)}
                </Badge>
                <span>•</span>
                <Clock className="h-3 w-3" />
                <span>{getFrequencyLabel(config.frequency)}</span>
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Switch checked={config.is_enabled} onCheckedChange={onToggle} />
            <Button variant="ghost" size="icon" onClick={onEdit}>
              <Pencil className="h-4 w-4" />
            </Button>
            <Button variant="ghost" size="icon" onClick={onDelete}>
              <Trash2 className="h-4 w-4 text-destructive" />
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

function ConfigListSkeleton() {
  return (
    <div className="space-y-4">
      {Array.from({ length: 3 }).map((_, i) => (
        <Skeleton key={i} className="h-20" />
      ))}
    </div>
  );
}

// =====================================================
// MAIN PAGE COMPONENT
// =====================================================

export default function ReportsConfigPage() {
  const navigate = useNavigate();
  const [showForm, setShowForm] = useState(false);
  const [editingConfig, setEditingConfig] = useState<ReportConfiguration | null>(null);

  // Hooks
  const { data: configs, isLoading } = useReportConfigs();
  const createConfig = useCreateReportConfig();
  const updateConfig = useUpdateReportConfig();
  const deleteConfig = useDeleteReportConfig();

  // Handlers
  const handleCreate = (data: ConfigFormData) => {
    createConfig.mutate(
      {
        name: data.name,
        report_type: data.report_type,
        frequency: data.frequency,
        schedule_time: data.schedule_time,
        schedule_day: data.schedule_day,
        n8n_webhook_url: data.n8n_webhook_url || undefined,
        description: data.description || undefined,
        recipients: data.recipients.split(",").map((r) => r.trim()).filter(Boolean),
        is_enabled: data.is_enabled,
      },
      {
        onSuccess: () => {
          setShowForm(false);
        },
      }
    );
  };

  const handleUpdate = (data: ConfigFormData) => {
    if (!editingConfig) return;
    updateConfig.mutate(
      {
        id: editingConfig.id,
        name: data.name,
        report_type: data.report_type,
        frequency: data.frequency,
        schedule_time: data.schedule_time,
        schedule_day: data.schedule_day,
        n8n_webhook_url: data.n8n_webhook_url || undefined,
        description: data.description || undefined,
        recipients: data.recipients.split(",").map((r) => r.trim()).filter(Boolean),
        is_enabled: data.is_enabled,
      },
      {
        onSuccess: () => {
          setEditingConfig(null);
        },
      }
    );
  };

  const handleDelete = (configId: string) => {
    if (confirm("Êtes-vous sûr de vouloir supprimer cette configuration ?")) {
      deleteConfig.mutate(configId);
    }
  };

  const handleToggle = (config: ReportConfiguration, enabled: boolean) => {
    updateConfig.mutate({ id: config.id, is_enabled: enabled });
  };

  const handleEdit = (config: ReportConfiguration) => {
    setEditingConfig(config);
    setShowForm(false);
  };

  return (
    <div className="container mx-auto py-6 space-y-6">
      {/* Header */}
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={() => navigate("/reports")}>
          <ArrowLeft className="h-5 w-5" />
        </Button>
        <div className="flex-1">
          <h1 className="text-2xl font-bold">Configuration des Rapports</h1>
          <p className="text-muted-foreground">Gérez l'automatisation de vos rapports</p>
        </div>
        {!showForm && !editingConfig && (
          <Button onClick={() => setShowForm(true)}>
            <Plus className="h-4 w-4 mr-2" />
            Nouvelle Configuration
          </Button>
        )}
      </div>

      {/* New Config Form */}
      {showForm && (
        <Card className="bg-card/50 border-border/50">
          <CardHeader>
            <CardTitle>Nouvelle Configuration</CardTitle>
            <CardDescription>Créez une configuration pour automatiser la génération</CardDescription>
          </CardHeader>
          <CardContent>
            <ConfigForm
              onSubmit={handleCreate}
              onCancel={() => setShowForm(false)}
              isSubmitting={createConfig.isPending}
            />
          </CardContent>
        </Card>
      )}

      {/* Edit Config Form */}
      {editingConfig && (
        <Card className="bg-card/50 border-border/50">
          <CardHeader>
            <CardTitle>Modifier la Configuration</CardTitle>
            <CardDescription>Mettez à jour les paramètres de "{editingConfig.name}"</CardDescription>
          </CardHeader>
          <CardContent>
            <ConfigForm
              initialData={{
                name: editingConfig.name,
                report_type: editingConfig.report_type,
                frequency: editingConfig.frequency,
                schedule_time: editingConfig.schedule_time,
                schedule_day: editingConfig.schedule_day || 1,
                n8n_webhook_url: editingConfig.n8n_webhook_url || "",
                description: editingConfig.description || "",
                recipients: editingConfig.recipients?.join(", ") || "",
                is_enabled: editingConfig.is_enabled,
              }}
              onSubmit={handleUpdate}
              onCancel={() => setEditingConfig(null)}
              isSubmitting={updateConfig.isPending}
              isEditing
            />
          </CardContent>
        </Card>
      )}

      {/* Configurations List */}
      <Card className="bg-card/50 border-border/50">
        <CardHeader>
          <CardTitle>Configurations existantes</CardTitle>
          <CardDescription>
            {configs?.length || 0} configuration(s) enregistrée(s)
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <ConfigListSkeleton />
          ) : configs?.length === 0 ? (
            <div className="text-center py-12 text-muted-foreground">
              <FileText className="h-12 w-12 mx-auto mb-4 opacity-50" />
              <p>Aucune configuration automatique</p>
              <p className="text-sm">Créez une configuration pour automatiser vos rapports</p>
            </div>
          ) : (
            <div className="space-y-3">
              {configs?.map((config) => (
                <ConfigCard
                  key={config.id}
                  config={config}
                  onEdit={() => handleEdit(config)}
                  onDelete={() => handleDelete(config.id)}
                  onToggle={(enabled) => handleToggle(config, enabled)}
                />
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
