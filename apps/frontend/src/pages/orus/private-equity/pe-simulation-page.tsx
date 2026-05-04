/**
 * PE Simulation Page
 * 4 tabs: Modèle, Données, Scénarios, Résultats
 * Based on orus-reference/src/pages/PESimulation.tsx
 */
import { useState, useEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import {
  ArrowLeft,
  Building2,
  Settings,
  Play,
  Plus,
  AlertTriangle,
  CheckCircle,
  Info,
  Sliders,
  FileSpreadsheet,
  BarChart3,
  Target,
  Calculator,
  Upload,
  Trash2,
  ChevronRight,
} from "lucide-react";
import {
  BarChart,
  Bar,
  AreaChart,
  Area,
  ComposedChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip as RechartsTooltip,
  ResponsiveContainer,
  Legend,
  Cell,
} from "recharts";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@wealthfolio/ui/components/ui/card";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@wealthfolio/ui/components/ui/tabs";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Skeleton } from "@wealthfolio/ui/components/ui/skeleton";
// Slider component - using Input for numeric values instead
// import { Slider } from "@wealthfolio/ui/components/ui/slider";
import { Input } from "@wealthfolio/ui/components/ui/input";
import { Label } from "@wealthfolio/ui/components/ui/label";
import { Textarea } from "@wealthfolio/ui/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@wealthfolio/ui/components/ui/select";
import { Switch } from "@wealthfolio/ui/components/ui/switch";
// Progress component not used
// import { Progress } from "@wealthfolio/ui/components/ui/progress";
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
import { Checkbox } from "@wealthfolio/ui/components/ui/checkbox";
// ScrollArea not used directly
// import { ScrollArea } from "@wealthfolio/ui/components/ui/scroll-area";
import { toast } from "sonner";

import {
  usePECompany,
  useUpdatePECompany,
} from "@/features/orus-integration";

// =====================================================
// TYPES
// =====================================================

interface RevenueSourceDriver {
  key: string;
  label: string;
  value: number;
  unit: string;
}

interface RevenueSource {
  name: string;
  current_amount: number;
  drivers: RevenueSourceDriver[];
}

interface CostBreakdownItem {
  name: string;
  amount: number;
}

interface CostStructure {
  monthly_base: number;
  breakdown: CostBreakdownItem[];
  marketing_annual: number;
  other_costs: CostBreakdownItem[];
}

interface CurrentMetrics {
  total_users: number;
  new_users_period: number;
  activation_rate: number;
  active_users: number;
  churn_rate: number;
  initial_cash: number;
  cac: number;
}

interface FunnelStage {
  name: string;
  conversion_rate: number;
}

interface Scenario {
  id: string;
  name: string;
  horizon: number;
  description?: string;
  drivers: Record<string, number>;
}

interface SimulationResult {
  forecasts: Array<{
    month: number;
    revenue: number;
    ebitda: number;
    cash: number;
    customers: number;
    activeUsers: number;
    arpu: number;
    cac: number;
    ltv: number;
    runway: number;
    charges: number;
  }>;
  unitEconomics: {
    arpu: number;
    ltv: number;
    cac: number;
    ltvCacRatio: number;
    paybackMonths: number;
    runway: number;
  };
}

// =====================================================
// BUSINESS MODEL TEMPLATES
// =====================================================

const BUSINESS_MODEL_TEMPLATES = {
  saas: {
    name: "SaaS / Abonnement",
    description: "Revenus récurrents via abonnements mensuels ou annuels",
    icon: "💳",
    kpis: ["MRR", "ARR", "Churn Rate", "Net Revenue Retention", "LTV/CAC"],
    defaultDrivers: { churn_rate: 3, activation_rate: 40, variable_cost_ratio: 20 },
  },
  marketplace: {
    name: "Marketplace",
    description: "Commission sur transactions entre acheteurs et vendeurs",
    icon: "🏪",
    kpis: ["GMV", "Take Rate", "Active Sellers", "Active Buyers", "AOV"],
    defaultDrivers: { take_rate: 15, activation_rate: 30, variable_cost_ratio: 10 },
  },
  fintech: {
    name: "Fintech / Wallet",
    description: "Services financiers, paiements, épargne",
    icon: "🏦",
    kpis: ["AUM", "Revenue per User", "Transaction Volume", "Active Rate"],
    defaultDrivers: { take_rate: 1.5, activation_rate: 45, variable_cost_ratio: 15 },
  },
  ecommerce: {
    name: "E-commerce / Retail",
    description: "Vente de produits en ligne avec marge brute",
    icon: "🛒",
    kpis: ["GMV", "Average Order Value", "Repeat Purchase Rate", "CAC"],
    defaultDrivers: { variable_cost_ratio: 60, activation_rate: 25, churn_rate: 5 },
  },
  service: {
    name: "Service / Conseil",
    description: "Prestations de services et consulting",
    icon: "💼",
    kpis: ["Billable Hours", "Average Daily Rate", "Utilization Rate"],
    defaultDrivers: { variable_cost_ratio: 70, activation_rate: 60, churn_rate: 2 },
  },
  custom: {
    name: "Personnalisé",
    description: "Modèle entièrement configurable selon vos besoins",
    icon: "⚙️",
    kpis: [],
    defaultDrivers: {},
  },
};

// =====================================================
// HELPER FUNCTIONS
// =====================================================

const formatCurrency = (value: number, currency = "EUR"): string => {
  return new Intl.NumberFormat("fr-FR", {
    style: "currency",
    currency,
    maximumFractionDigits: 0,
  }).format(value);
};

const formatNumber = (value: number): string => {
  return new Intl.NumberFormat("fr-FR").format(value);
};

const formatPercent = (value: number): string => {
  return `${value.toFixed(1)}%`;
};

// =====================================================
// BUSINESS MODEL SELECTOR
// =====================================================

function BusinessModelSelector({
  value,
  onChange,
  isUpdating = false,
}: {
  value: string;
  onChange: (value: string) => void;
  isUpdating?: boolean;
}) {
  return (
    <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
      {Object.entries(BUSINESS_MODEL_TEMPLATES).map(([key, template]) => {
        const isSelected = value === key;

        return (
          <Card
            key={key}
            className={`cursor-pointer transition-all duration-200 hover:scale-[1.02] ${
              isSelected
                ? "border-primary bg-primary/10 ring-1 ring-primary/30"
                : "bg-card/50 border-border/50 hover:border-primary/40 hover:bg-card/80"
            } ${isUpdating ? "opacity-50 pointer-events-none" : ""}`}
            onClick={() => onChange(key)}
          >
            <CardContent className="pt-5 pb-5">
              <div className="flex items-start gap-3">
                <div
                  className={`p-2.5 rounded-lg transition-colors ${
                    isSelected
                      ? "bg-primary/20 text-primary"
                      : "bg-muted text-muted-foreground"
                  }`}
                >
                  <span className="text-lg">{template.icon}</span>
                </div>
                <div className="flex-1 min-w-0">
                  <p
                    className={`font-semibold text-sm ${
                      isSelected ? "text-primary" : "text-foreground"
                    }`}
                  >
                    {template.name}
                  </p>
                  <p className="text-xs text-muted-foreground mt-1 line-clamp-2">
                    {template.description}
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}

// =====================================================
// CURRENT METRICS CONFIG
// =====================================================

function CurrentMetricsConfig({
  metrics,
  onChange,
}: {
  metrics: CurrentMetrics;
  onChange: (metrics: CurrentMetrics) => void;
}) {
  const update = (field: keyof CurrentMetrics, value: number) => {
    const updated = { ...metrics, [field]: value };
    if (field === "total_users" || field === "activation_rate") {
      const users = field === "total_users" ? value : updated.total_users;
      const rate = field === "activation_rate" ? value : updated.activation_rate;
      updated.active_users = Math.round(users * (rate / 100));
    }
    onChange(updated);
  };

  const fields = [
    { key: "total_users" as const, label: "Utilisateurs totaux", unit: "", step: 100 },
    { key: "activation_rate" as const, label: "Taux d'activation", unit: "%", step: 1 },
    { key: "churn_rate" as const, label: "Churn mensuel", unit: "%", step: 0.1 },
    { key: "cac" as const, label: "CAC", unit: "€", step: 1 },
    { key: "initial_cash" as const, label: "Cash disponible", unit: "€", step: 1000 },
  ];

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <CardTitle className="text-base flex items-center gap-2">
          <Calculator className="h-5 w-5" />
          Métriques Actuelles
        </CardTitle>
        <CardDescription>
          Renseignez les métriques actuelles de l'entreprise
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-4">
          {fields.map((f) => (
            <div key={f.key} className="space-y-1.5">
              <Label className="text-xs">{f.label}</Label>
              <div className="flex items-center gap-1">
                <Input
                  type="number"
                  value={metrics[f.key] || 0}
                  onChange={(e) => update(f.key, parseFloat(e.target.value) || 0)}
                  className="h-9 text-right bg-background/50"
                  step={f.step}
                />
                {f.unit && (
                  <span className="text-xs text-muted-foreground w-6">{f.unit}</span>
                )}
              </div>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

// =====================================================
// REVENUE SOURCES CONFIG
// =====================================================

function RevenueSourcesConfig({
  sources,
  onChange,
}: {
  sources: RevenueSource[];
  onChange: (sources: RevenueSource[]) => void;
}) {
  const totalAmount = sources.reduce((sum, s) => sum + (s.current_amount || 0), 0);

  const addSource = () => {
    onChange([
      ...sources,
      { name: "", current_amount: 0, drivers: [] },
    ]);
  };

  const updateSource = (
    index: number,
    field: "name" | "current_amount",
    value: string | number
  ) => {
    const updated = [...sources];
    updated[index] = { ...updated[index], [field]: value };
    onChange(updated);
  };

  const removeSource = (index: number) => {
    onChange(sources.filter((_, i) => i !== index));
  };

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="text-base">Sources de revenus</CardTitle>
            <CardDescription>
              Définissez les différentes sources de revenus annuels
            </CardDescription>
          </div>
          {totalAmount > 0 && (
            <Badge variant="outline">Total: {formatCurrency(totalAmount)}</Badge>
          )}
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Source</TableHead>
              <TableHead className="text-right">Montant annuel</TableHead>
              <TableHead className="text-right">% du CA</TableHead>
              <TableHead className="text-right">Drivers</TableHead>
              <TableHead className="w-12"></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {sources.map((source, i) => {
              const pct = totalAmount > 0 ? (source.current_amount / totalAmount) * 100 : 0;
              return (
                <TableRow key={i}>
                  <TableCell>
                    <Input
                      placeholder="Nom de la source"
                      value={source.name}
                      onChange={(e) => updateSource(i, "name", e.target.value)}
                      className="h-8"
                    />
                  </TableCell>
                  <TableCell className="text-right">
                    <Input
                      type="number"
                      value={source.current_amount || 0}
                      onChange={(e) =>
                        updateSource(i, "current_amount", parseFloat(e.target.value) || 0)
                      }
                      className="h-8 w-32 text-right"
                    />
                  </TableCell>
                  <TableCell className="text-right">
                    <Badge variant="secondary">{formatPercent(pct)}</Badge>
                  </TableCell>
                  <TableCell className="text-right text-xs text-muted-foreground">
                    {source.drivers?.length || 0} driver(s)
                  </TableCell>
                  <TableCell>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => removeSource(i)}
                      className="h-8 w-8"
                    >
                      <Trash2 className="h-4 w-4 text-destructive" />
                    </Button>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
        <Button variant="outline" size="sm" onClick={addSource} className="w-full">
          <Plus className="h-4 w-4 mr-2" />
          Ajouter une source
        </Button>
      </CardContent>
    </Card>
  );
}

// =====================================================
// COST STRUCTURE CONFIG
// =====================================================

function CostStructureConfig({
  costStructure,
  onChange,
}: {
  costStructure: CostStructure;
  onChange: (cs: CostStructure) => void;
}) {
  const totalAnnualBase =
    costStructure.breakdown?.reduce((sum, item) => sum + (item.amount || 0), 0) || 0;
  const totalOther =
    costStructure.other_costs?.reduce((sum, item) => sum + (item.amount || 0), 0) || 0;
  const totalCharges =
    (costStructure.monthly_base || 0) * 12 +
    totalAnnualBase +
    (costStructure.marketing_annual || 0) +
    totalOther;

  const addBreakdown = () => {
    onChange({
      ...costStructure,
      breakdown: [...(costStructure.breakdown || []), { name: "", amount: 0 }],
    });
  };

  const updateBreakdown = (
    index: number,
    field: "name" | "amount",
    value: string | number
  ) => {
    const updated = [...(costStructure.breakdown || [])];
    updated[index] = { ...updated[index], [field]: value };
    onChange({ ...costStructure, breakdown: updated });
  };

  const removeBreakdown = (index: number) => {
    onChange({
      ...costStructure,
      breakdown: (costStructure.breakdown || []).filter((_, i) => i !== index),
    });
  };

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="text-base">Structure de coûts</CardTitle>
            <CardDescription>
              Détaillez les charges mensuelles et annuelles
            </CardDescription>
          </div>
          {totalCharges > 0 && (
            <Badge variant="outline">Total annuel: {formatCurrency(totalCharges)}</Badge>
          )}
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Poste</TableHead>
              <TableHead className="text-right">Montant mensuel</TableHead>
              <TableHead className="text-right">% charges</TableHead>
              <TableHead className="text-right">Type</TableHead>
              <TableHead className="w-12"></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {(costStructure.breakdown || []).map((item, i) => {
              const pct = totalCharges > 0 ? ((item.amount || 0) / totalCharges) * 100 : 0;
              return (
                <TableRow key={i}>
                  <TableCell>
                    <Input
                      placeholder="Ex: Salaires, Tech..."
                      value={item.name}
                      onChange={(e) => updateBreakdown(i, "name", e.target.value)}
                      className="h-8"
                    />
                  </TableCell>
                  <TableCell className="text-right">
                    <Input
                      type="number"
                      value={item.amount || 0}
                      onChange={(e) =>
                        updateBreakdown(i, "amount", parseFloat(e.target.value) || 0)
                      }
                      className="h-8 w-28 text-right"
                    />
                  </TableCell>
                  <TableCell className="text-right">
                    <Badge variant="secondary">{formatPercent(pct)}</Badge>
                  </TableCell>
                  <TableCell className="text-right">
                    <Badge variant="outline">Fixe</Badge>
                  </TableCell>
                  <TableCell>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => removeBreakdown(i)}
                      className="h-8 w-8"
                    >
                      <Trash2 className="h-4 w-4 text-destructive" />
                    </Button>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
        <Button variant="outline" size="sm" onClick={addBreakdown} className="w-full">
          <Plus className="h-4 w-4 mr-2" />
          Ajouter un poste
        </Button>
      </CardContent>
    </Card>
  );
}

// =====================================================
// FUNNEL CONFIG
// =====================================================

function FunnelConfig({
  stages,
  onChange,
}: {
  stages: FunnelStage[];
  onChange: (stages: FunnelStage[]) => void;
}) {
  const defaultStages: FunnelStage[] = [
    { name: "Visiteurs", conversion_rate: 100 },
    { name: "Inscrits", conversion_rate: 15 },
    { name: "Activés", conversion_rate: 45 },
    { name: "Payants", conversion_rate: 20 },
  ];

  const currentStages = stages.length > 0 ? stages : defaultStages;

  const updateStage = (
    index: number,
    field: "name" | "conversion_rate",
    value: string | number
  ) => {
    const updated = [...currentStages];
    updated[index] = { ...updated[index], [field]: value };
    onChange(updated);
  };

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <CardTitle className="text-base">Funnel de conversion</CardTitle>
        <CardDescription>
          Définissez les taux de conversion à chaque étape
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="flex items-center gap-2 flex-wrap">
          {currentStages.map((stage, i) => (
            <div key={i} className="flex items-center gap-2">
              <div className="rounded-lg border border-border/50 bg-background/30 px-4 py-3 text-center min-w-[100px]">
                <Input
                  value={stage.name}
                  onChange={(e) => updateStage(i, "name", e.target.value)}
                  className="h-6 text-xs text-center border-0 bg-transparent p-0"
                />
                <div className="flex items-center justify-center gap-1 mt-1">
                  <Input
                    type="number"
                    value={stage.conversion_rate}
                    onChange={(e) =>
                      updateStage(i, "conversion_rate", parseFloat(e.target.value) || 0)
                    }
                    className="h-6 w-12 text-center text-lg font-bold border-0 bg-transparent p-0"
                  />
                  <span className="text-lg font-bold">%</span>
                </div>
              </div>
              {i < currentStages.length - 1 && (
                <ChevronRight className="h-4 w-4 text-muted-foreground" />
              )}
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

// =====================================================
// SCENARIO MANAGER
// =====================================================

function ScenarioManager({
  scenarios,
  activeScenario,
  onSelectScenario,
  onCreateScenario,
  onDeleteScenario,
  horizonMonths,
  onHorizonChange,
  onRunSimulation,
  isSimulating,
}: {
  scenarios: Scenario[];
  activeScenario: Scenario | null;
  onSelectScenario: (scenario: Scenario) => void;
  onCreateScenario: (scenario: Omit<Scenario, "id">) => void;
  onDeleteScenario: (id: string) => void;
  horizonMonths: number;
  onHorizonChange: (months: number) => void;
  onRunSimulation: () => void;
  isSimulating: boolean;
}) {
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [newScenario, setNewScenario] = useState({
    name: "",
    horizon: 24,
    description: "",
  });

  const handleCreate = () => {
    onCreateScenario({
      name: newScenario.name || "Nouveau scénario",
      horizon: newScenario.horizon,
      description: newScenario.description,
      drivers: {},
    });
    setIsDialogOpen(false);
    setNewScenario({ name: "", horizon: 24, description: "" });
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium">Scénarios</h3>
        <Dialog open={isDialogOpen} onOpenChange={setIsDialogOpen}>
          <DialogTrigger asChild>
            <Button variant="outline" size="sm">
              <Plus className="h-4 w-4 mr-2" />
              Nouveau scénario
            </Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Créer un scénario</DialogTitle>
              <DialogDescription>
                Définissez les paramètres de votre nouveau scénario de simulation
              </DialogDescription>
            </DialogHeader>
            <div className="space-y-4 py-4">
              <div className="space-y-2">
                <Label>Nom</Label>
                <Input
                  placeholder="Ex: Scénario optimiste"
                  value={newScenario.name}
                  onChange={(e) =>
                    setNewScenario({ ...newScenario, name: e.target.value })
                  }
                />
              </div>
              <div className="space-y-2">
                <Label>Horizon (mois): {newScenario.horizon}</Label>
                <Input
                  type="range"
                  value={newScenario.horizon}
                  onChange={(e) =>
                    setNewScenario({ ...newScenario, horizon: parseInt(e.target.value) })
                  }
                  min={6}
                  max={60}
                  step={6}
                  className="w-full"
                />
              </div>
              <div className="space-y-2">
                <Label>Description</Label>
                <Textarea
                  placeholder="Notes sur ce scénario..."
                  value={newScenario.description}
                  onChange={(e) =>
                    setNewScenario({ ...newScenario, description: e.target.value })
                  }
                />
              </div>
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={() => setIsDialogOpen(false)}>
                Annuler
              </Button>
              <Button onClick={handleCreate}>Créer</Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </div>

      {/* Scenario list */}
      <div className="space-y-2">
        {scenarios.map((scenario) => (
          <Card
            key={scenario.id}
            className={`cursor-pointer transition-all ${
              activeScenario?.id === scenario.id
                ? "border-primary bg-primary/5"
                : "border-border/50 hover:border-primary/30"
            }`}
            onClick={() => onSelectScenario(scenario)}
          >
            <CardContent className="p-3 flex items-center justify-between">
              <div>
                <p className="font-medium text-sm">{scenario.name}</p>
                <p className="text-xs text-muted-foreground">
                  Horizon: {scenario.horizon} mois
                </p>
              </div>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8"
                onClick={(e) => {
                  e.stopPropagation();
                  onDeleteScenario(scenario.id);
                }}
              >
                <Trash2 className="h-4 w-4 text-destructive" />
              </Button>
            </CardContent>
          </Card>
        ))}
      </div>

      {/* Horizon slider */}
      <div className="space-y-2 pt-4 border-t">
        <Label>Horizon de simulation: {horizonMonths} mois</Label>
        <Input
          type="range"
          value={horizonMonths}
          onChange={(e) => onHorizonChange(parseInt(e.target.value))}
          min={6}
          max={60}
          step={6}
          className="w-full"
        />
      </div>

      {/* Run button */}
      <Button
        onClick={onRunSimulation}
        disabled={isSimulating || !activeScenario}
        className="w-full"
      >
        {isSimulating ? (
          <>
            <Settings className="h-4 w-4 mr-2 animate-spin" />
            Calcul en cours...
          </>
        ) : (
          <>
            <Play className="h-4 w-4 mr-2" />
            Lancer simulation
          </>
        )}
      </Button>
    </div>
  );
}

// =====================================================
// DRIVER PANEL
// =====================================================

function DriverPanel({
  drivers,
  onDriverChange,
  targetMode,
  onTargetModeChange,
  selectedDrivers,
  onDriverToggle,
}: {
  drivers: Record<string, number>;
  onDriverChange: (key: string, value: number) => void;
  targetMode: boolean;
  onTargetModeChange: (mode: boolean) => void;
  selectedDrivers: string[];
  onDriverToggle: (key: string) => void;
}) {
  const driverConfigs = [
    { key: "customer_growth", label: "Croissance clients", unit: "%", min: -20, max: 100 },
    { key: "activation_rate", label: "Taux d'activation", unit: "%", min: 0, max: 100 },
    { key: "cac", label: "CAC", unit: "€", min: 0, max: 500 },
    { key: "churn_rate", label: "Churn", unit: "%", min: 0, max: 30 },
    { key: "arpu", label: "ARPU", unit: "€", min: 0, max: 500 },
    { key: "fixed_costs", label: "Coûts fixes", unit: "€", min: 0, max: 100000 },
    { key: "variable_cost_ratio", label: "Coûts variables", unit: "%", min: 0, max: 100 },
  ];

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium">Drivers</h3>
        <div className="flex items-center gap-2">
          <Label className="text-xs">Mode Cible</Label>
          <Switch checked={targetMode} onCheckedChange={onTargetModeChange} />
        </div>
      </div>

      <div className="space-y-3">
        {driverConfigs.map((config) => {
          const isSelected = selectedDrivers.includes(config.key);

          return (
            <div key={config.key} className="space-y-2">
              <div className="flex items-center gap-2">
                <Checkbox
                  checked={isSelected}
                  onCheckedChange={() => onDriverToggle(config.key)}
                />
                <Label className="text-xs flex-1">{config.label}</Label>
                {isSelected && (
                  <div className="flex items-center gap-1">
                    <Input
                      type="number"
                      value={drivers[config.key] || 0}
                      onChange={(e) =>
                        onDriverChange(config.key, parseFloat(e.target.value) || 0)
                      }
                      className="w-20 h-7 text-right text-xs"
                    />
                    <span className="text-xs text-muted-foreground w-4">
                      {config.unit}
                    </span>
                  </div>
                )}
              </div>
              {isSelected && !targetMode && (
                <Input
                  type="range"
                  value={drivers[config.key] || 0}
                  onChange={(e) => onDriverChange(config.key, parseFloat(e.target.value))}
                  min={config.min}
                  max={config.max}
                  className="w-full"
                />
              )}
            </div>
          );
        })}
      </div>

      <Button variant="outline" size="sm" className="w-full">
        <Target className="h-4 w-4 mr-2" />
        Calibrer auto
      </Button>
    </div>
  );
}

// =====================================================
// KPI CARDS (RESULTS)
// =====================================================

function KPICards({ unitEconomics }: { unitEconomics: SimulationResult["unitEconomics"] }) {
  const metrics = [
    {
      label: "ARPU",
      value: formatCurrency(unitEconomics.arpu),
      status: "neutral",
    },
    {
      label: "LTV",
      value: formatCurrency(unitEconomics.ltv),
      status: "neutral",
    },
    {
      label: "CAC",
      value: formatCurrency(unitEconomics.cac),
      status: "neutral",
    },
    {
      label: "LTV/CAC",
      value: `${unitEconomics.ltvCacRatio.toFixed(1)}x`,
      status: unitEconomics.ltvCacRatio >= 3 ? "good" : "bad",
    },
    {
      label: "PAYBACK",
      value: `${unitEconomics.paybackMonths.toFixed(0)} mois`,
      status: unitEconomics.paybackMonths <= 12 ? "good" : "bad",
    },
    {
      label: "RUNWAY",
      value:
        unitEconomics.runway >= 999 ? "∞" : `${unitEconomics.runway.toFixed(0)} mois`,
      status:
        unitEconomics.runway >= 18 ? "good" : unitEconomics.runway >= 12 ? "warning" : "bad",
    },
  ];

  const getStatusColor = (status: string) => {
    switch (status) {
      case "good":
        return "text-emerald-500";
      case "warning":
        return "text-amber-500";
      case "bad":
        return "text-red-500";
      default:
        return "text-foreground";
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case "good":
        return <CheckCircle className="h-4 w-4 text-emerald-500" />;
      case "warning":
        return <AlertTriangle className="h-4 w-4 text-amber-500" />;
      case "bad":
        return <AlertTriangle className="h-4 w-4 text-red-500" />;
      default:
        return <Info className="h-4 w-4 text-muted-foreground" />;
    }
  };

  return (
    <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
      {metrics.map((metric) => (
        <Card key={metric.label} className="bg-card/50 border-border/50">
          <CardContent className="pt-4">
            <div className="flex items-center justify-between mb-2">
              <p className="text-xs text-muted-foreground uppercase">{metric.label}</p>
              {getStatusIcon(metric.status)}
            </div>
            <p className={`text-xl font-bold ${getStatusColor(metric.status)}`}>
              {metric.value}
            </p>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

// =====================================================
// CHARTS
// =====================================================

const CHART_COLORS = ["#3b82f6", "#10b981", "#f59e0b", "#ef4444", "#8b5cf6", "#ec4899"];

function RevenueEbitdaChart({
  forecasts,
}: {
  forecasts: SimulationResult["forecasts"];
}) {
  const data = forecasts.map((f, i) => ({
    month: `M${i + 1}`,
    revenue: f.revenue,
    ebitda: f.ebitda,
  }));

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <CardTitle className="text-base">Revenus & EBITDA</CardTitle>
      </CardHeader>
      <CardContent className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={data}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
            <XAxis dataKey="month" tick={{ fontSize: 10 }} stroke="hsl(var(--muted-foreground))" />
            <YAxis tick={{ fontSize: 10 }} stroke="hsl(var(--muted-foreground))" />
            <RechartsTooltip />
            <Legend />
            <Bar dataKey="revenue" fill={CHART_COLORS[0]} name="Revenus" radius={[4, 4, 0, 0]} />
            <Bar dataKey="ebitda" fill={CHART_COLORS[1]} name="EBITDA" radius={[4, 4, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}

function CashRunwayChart({
  forecasts,
}: {
  forecasts: SimulationResult["forecasts"];
}) {
  const data = forecasts.map((f, i) => ({
    month: `M${i + 1}`,
    cash: f.cash,
    runway: f.runway,
  }));

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <CardTitle className="text-base">Cash & Runway</CardTitle>
      </CardHeader>
      <CardContent className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={data}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
            <XAxis dataKey="month" tick={{ fontSize: 10 }} stroke="hsl(var(--muted-foreground))" />
            <YAxis tick={{ fontSize: 10 }} stroke="hsl(var(--muted-foreground))" />
            <RechartsTooltip />
            <Legend />
            <Area
              type="monotone"
              dataKey="cash"
              fill={CHART_COLORS[0]}
              fillOpacity={0.3}
              stroke={CHART_COLORS[0]}
              name="Cash"
            />
          </AreaChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}

function CustomersChart({
  forecasts,
}: {
  forecasts: SimulationResult["forecasts"];
}) {
  const data = forecasts.map((f, i) => ({
    month: `M${i + 1}`,
    customers: f.customers,
    activeUsers: f.activeUsers,
  }));

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <CardTitle className="text-base">Clients & Utilisateurs actifs</CardTitle>
      </CardHeader>
      <CardContent className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <ComposedChart data={data}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
            <XAxis dataKey="month" tick={{ fontSize: 10 }} stroke="hsl(var(--muted-foreground))" />
            <YAxis tick={{ fontSize: 10 }} stroke="hsl(var(--muted-foreground))" />
            <RechartsTooltip />
            <Legend />
            <Line
              type="monotone"
              dataKey="customers"
              stroke={CHART_COLORS[0]}
              strokeWidth={2}
              name="Clients"
              dot={false}
            />
            <Line
              type="monotone"
              dataKey="activeUsers"
              stroke={CHART_COLORS[1]}
              strokeWidth={2}
              name="Actifs"
              dot={false}
            />
          </ComposedChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}

function UnitEconomicsChart({
  forecasts,
}: {
  forecasts: SimulationResult["forecasts"];
}) {
  const data = forecasts
    .filter((_, i) => i % 3 === 0) // Every 3 months
    .map((f, i) => ({
      month: `M${i * 3 + 1}`,
      cac: f.cac,
      ltv: f.ltv,
      arpu: f.arpu * 12,
    }));

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <CardTitle className="text-base">Unit Economics</CardTitle>
      </CardHeader>
      <CardContent className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={data}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
            <XAxis dataKey="month" tick={{ fontSize: 10 }} stroke="hsl(var(--muted-foreground))" />
            <YAxis tick={{ fontSize: 10 }} stroke="hsl(var(--muted-foreground))" />
            <RechartsTooltip />
            <Legend />
            <Bar dataKey="cac" fill={CHART_COLORS[3]} name="CAC" radius={[4, 4, 0, 0]} />
            <Bar dataKey="ltv" fill={CHART_COLORS[1]} name="LTV" radius={[4, 4, 0, 0]} />
            <Bar dataKey="arpu" fill={CHART_COLORS[0]} name="ARPU (annuel)" radius={[4, 4, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}

function ARPUCACLTVChart({
  forecasts,
}: {
  forecasts: SimulationResult["forecasts"];
}) {
  const last = forecasts[forecasts.length - 1];
  const data = [
    { name: "ARPU", value: last?.arpu || 0 },
    { name: "CAC", value: last?.cac || 0 },
    { name: "LTV", value: last?.ltv || 0 },
  ];

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <CardTitle className="text-base">ARPU / CAC / LTV</CardTitle>
      </CardHeader>
      <CardContent className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={data} layout="vertical">
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
            <XAxis type="number" tick={{ fontSize: 10 }} stroke="hsl(var(--muted-foreground))" />
            <YAxis dataKey="name" type="category" tick={{ fontSize: 10 }} stroke="hsl(var(--muted-foreground))" />
            <RechartsTooltip />
            <Bar dataKey="value" radius={[0, 4, 4, 0]}>
              {data.map((_, index) => (
                <Cell key={`cell-${index}`} fill={CHART_COLORS[index]} />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}

function RevenueVsChargesChart({
  forecasts,
}: {
  forecasts: SimulationResult["forecasts"];
}) {
  const data = forecasts.map((f, i) => ({
    month: `M${i + 1}`,
    revenue: f.revenue,
    charges: f.charges,
  }));

  return (
    <Card className="bg-card/50 border-border/50">
      <CardHeader>
        <CardTitle className="text-base">Revenus vs Charges — Path to Profitability</CardTitle>
      </CardHeader>
      <CardContent className="h-64">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={data}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
            <XAxis dataKey="month" tick={{ fontSize: 10 }} stroke="hsl(var(--muted-foreground))" />
            <YAxis tick={{ fontSize: 10 }} stroke="hsl(var(--muted-foreground))" />
            <RechartsTooltip />
            <Legend />
            <Area
              type="monotone"
              dataKey="revenue"
              fill={CHART_COLORS[1]}
              fillOpacity={0.3}
              stroke={CHART_COLORS[1]}
              name="Revenus"
            />
            <Area
              type="monotone"
              dataKey="charges"
              fill={CHART_COLORS[3]}
              fillOpacity={0.3}
              stroke={CHART_COLORS[3]}
              name="Charges"
            />
          </AreaChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}

// =====================================================
// MAIN PAGE COMPONENT
// =====================================================

export default function PESimulationPage() {
  const { companyId } = useParams<{ companyId: string }>();
  const navigate = useNavigate();

  // Company data
  const { data: company, isLoading: isLoadingCompany } = usePECompany(companyId || "");
  const updateCompany = useUpdatePECompany();

  // Local state
  const [activeTab, setActiveTab] = useState("model");
  const [businessModel, setBusinessModel] = useState("fintech");
  const [subsector, setSubsector] = useState("");
  const [positioning, setPositioning] = useState("");
  const [stage, setStage] = useState("early");

  const [currentMetrics, setCurrentMetrics] = useState<CurrentMetrics>({
    total_users: 0,
    new_users_period: 0,
    activation_rate: 45,
    active_users: 0,
    churn_rate: 3,
    initial_cash: 0,
    cac: 15,
  });

  const [revenueSources, setRevenueSources] = useState<RevenueSource[]>([]);
  const [costStructure, setCostStructure] = useState<CostStructure>({
    monthly_base: 0,
    breakdown: [],
    marketing_annual: 0,
    other_costs: [],
  });
  const [funnelStages, setFunnelStages] = useState<FunnelStage[]>([]);

  const [scenarios, setScenarios] = useState<Scenario[]>([]);
  const [activeScenario, setActiveScenario] = useState<Scenario | null>(null);
  const [horizonMonths, setHorizonMonths] = useState(24);

  const [drivers, setDrivers] = useState<Record<string, number>>({
    customer_growth: 10,
    activation_rate: 45,
    cac: 15,
    churn_rate: 3,
    arpu: 5,
    fixed_costs: 5000,
    variable_cost_ratio: 25,
  });
  const [targetMode, setTargetMode] = useState(false);
  const [selectedDrivers, setSelectedDrivers] = useState<string[]>([
    "customer_growth",
    "activation_rate",
    "cac",
    "churn_rate",
    "arpu",
  ]);

  const [simulationResult, setSimulationResult] = useState<SimulationResult | null>(
    null
  );
  const [isSimulating, setIsSimulating] = useState(false);

  // Load company data
  useEffect(() => {
    if (company) {
      setBusinessModel((company as any).business_model || "fintech");
      setSubsector((company as any).sector || "");
      setPositioning(company.description || "");
      setStage((company as any).stage || "early");

      if ((company as any).current_metrics) {
        setCurrentMetrics((prev) => ({ ...prev, ...(company as any).current_metrics }));
      }
      if (Array.isArray((company as any).revenue_sources)) {
        setRevenueSources((company as any).revenue_sources);
      }
      if ((company as any).cost_structure) {
        setCostStructure((prev) => ({ ...prev, ...(company as any).cost_structure }));
      }
      if (Array.isArray((company as any).funnel_stages)) {
        setFunnelStages((company as any).funnel_stages);
      }
    }
  }, [company]);

  // Handlers
  const handleBusinessModelChange = async (model: string) => {
    setBusinessModel(model);
    if (companyId) {
      try {
        await updateCompany.mutateAsync({
          companyId,
          business_model: model,
        } as any);
        toast.success("Modèle mis à jour");
      } catch (error) {
        console.error("Error updating business model:", error);
      }
    }
  };

  const handleCreateScenario = (scenario: Omit<Scenario, "id">) => {
    const newScenario: Scenario = {
      ...scenario,
      id: crypto.randomUUID(),
    };
    setScenarios((prev) => [...prev, newScenario]);
    setActiveScenario(newScenario);
    toast.success("Scénario créé");
  };

  const handleDeleteScenario = (id: string) => {
    setScenarios((prev) => prev.filter((s) => s.id !== id));
    if (activeScenario?.id === id) {
      setActiveScenario(null);
    }
    toast.success("Scénario supprimé");
  };

  const handleDriverChange = (key: string, value: number) => {
    setDrivers((prev) => ({ ...prev, [key]: value }));
  };

  const handleDriverToggle = (key: string) => {
    setSelectedDrivers((prev) =>
      prev.includes(key) ? prev.filter((k) => k !== key) : [...prev, key]
    );
  };

  const runSimulation = () => {
    setIsSimulating(true);

    // Simulate calculation (in real app, this would call an API)
    setTimeout(() => {
      const forecasts: SimulationResult["forecasts"] = [];
      let cash = currentMetrics.initial_cash;
      let customers = currentMetrics.total_users;
      const growthRate = (drivers.customer_growth || 10) / 100;
      const churnRate = (drivers.churn_rate || 3) / 100;
      const arpu = drivers.arpu || 5;
      const cac = drivers.cac || 15;
      const fixedCosts = drivers.fixed_costs || 5000;
      const variableRatio = (drivers.variable_cost_ratio || 25) / 100;
      const activationRate = (drivers.activation_rate || 45) / 100;

      for (let m = 0; m < horizonMonths; m++) {
        const newCustomers = Math.floor(customers * growthRate / 12);
        const churnedCustomers = Math.floor(customers * churnRate);
        customers = customers + newCustomers - churnedCustomers;
        const activeUsers = Math.floor(customers * activationRate);

        const revenue = activeUsers * arpu;
        const variableCosts = revenue * variableRatio;
        const totalCosts = fixedCosts + variableCosts + newCustomers * cac;
        const ebitda = revenue - totalCosts;

        cash = cash + ebitda;

        const ltv = arpu * 12 / (churnRate || 0.01);
        const runway = ebitda < 0 ? Math.max(0, cash / Math.abs(ebitda)) : 999;

        forecasts.push({
          month: m + 1,
          revenue,
          ebitda,
          cash,
          customers,
          activeUsers,
          arpu,
          cac,
          ltv,
          runway,
          charges: totalCosts,
        });
      }

      const last = forecasts[forecasts.length - 1];
      const ltv = arpu * 12 / (churnRate || 0.01);

      setSimulationResult({
        forecasts,
        unitEconomics: {
          arpu,
          ltv,
          cac,
          ltvCacRatio: ltv / (cac || 1),
          paybackMonths: cac / (arpu || 1),
          runway: last?.runway || 0,
        },
      });

      setIsSimulating(false);
      setActiveTab("results");
      toast.success("Simulation terminée");
    }, 1500);
  };

  // Summary stats for Données tab
  const totalRevenue = revenueSources.reduce((sum, s) => sum + (s.current_amount || 0), 0);
  const totalCosts =
    (costStructure.monthly_base || 0) * 12 +
    (costStructure.breakdown?.reduce((sum, b) => sum + (b.amount || 0), 0) || 0) +
    (costStructure.marketing_annual || 0) +
    (costStructure.other_costs?.reduce((sum, c) => sum + (c.amount || 0), 0) || 0);

  if (isLoadingCompany) {
    return (
      <div className="container mx-auto p-6 space-y-4">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-12 w-full" />
        <div className="grid grid-cols-2 gap-4">
          <Skeleton className="h-64" />
          <Skeleton className="h-64" />
        </div>
      </div>
    );
  }

  return (
    <div className="container mx-auto p-4 sm:p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={() => navigate(-1)}>
          <ArrowLeft className="h-5 w-5" />
        </Button>
        <div className="flex-1">
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Building2 className="h-6 w-6" />
            Simulation PE — {company?.name || "Entreprise"}
          </h1>
          <p className="text-muted-foreground text-sm">
            Modélisez et projetez les performances financières
          </p>
        </div>
      </div>

      {/* Main content */}
      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList className="grid w-full grid-cols-4">
          <TabsTrigger value="model" className="flex items-center gap-2">
            <Settings className="h-4 w-4" />
            <span className="hidden sm:inline">Modèle</span>
          </TabsTrigger>
          <TabsTrigger value="data" className="flex items-center gap-2">
            <FileSpreadsheet className="h-4 w-4" />
            <span className="hidden sm:inline">Données</span>
          </TabsTrigger>
          <TabsTrigger value="scenarios" className="flex items-center gap-2">
            <Sliders className="h-4 w-4" />
            <span className="hidden sm:inline">Scénarios</span>
          </TabsTrigger>
          <TabsTrigger value="results" className="flex items-center gap-2">
            <BarChart3 className="h-4 w-4" />
            <span className="hidden sm:inline">Résultats</span>
          </TabsTrigger>
        </TabsList>

        {/* TAB 1: MODÈLE */}
        <TabsContent value="model" className="space-y-6">
          {/* Business model selector */}
          <div className="space-y-4">
            <h2 className="text-lg font-semibold">Sélecteur de modèle d'affaires</h2>
            <BusinessModelSelector
              value={businessModel}
              onChange={handleBusinessModelChange}
              isUpdating={updateCompany.isPending}
            />
          </div>

          {/* Profil détaillé */}
          <Card className="bg-card/50 border-border/50">
            <CardHeader>
              <CardTitle className="text-base">Profil Détaillé</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div className="space-y-2">
                  <Label>Sous-secteur</Label>
                  <Input
                    placeholder="Ex: Mobile money, B2B SaaS..."
                    value={subsector}
                    onChange={(e) => setSubsector(e.target.value)}
                  />
                </div>
                <div className="space-y-2 md:col-span-2">
                  <Label>Positionnement</Label>
                  <Textarea
                    placeholder="Décrivez le positionnement marché..."
                    value={positioning}
                    onChange={(e) => setPositioning(e.target.value)}
                    rows={2}
                  />
                </div>
              </div>
              <div className="space-y-2">
                <Label>Stade</Label>
                <Select value={stage} onValueChange={setStage}>
                  <SelectTrigger className="w-48">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="pre-revenue">Pré-revenue</SelectItem>
                    <SelectItem value="early">Early stage</SelectItem>
                    <SelectItem value="growth">Growth</SelectItem>
                    <SelectItem value="scale">Scale</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </CardContent>
          </Card>

          {/* KPIs clés */}
          <Card className="bg-card/50 border-border/50">
            <CardHeader>
              <CardTitle className="text-base">KPIs Clés</CardTitle>
              <CardDescription>
                KPIs pertinents pour le modèle{" "}
                {BUSINESS_MODEL_TEMPLATES[businessModel as keyof typeof BUSINESS_MODEL_TEMPLATES]?.name}
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="flex flex-wrap gap-2">
                {(
                  BUSINESS_MODEL_TEMPLATES[businessModel as keyof typeof BUSINESS_MODEL_TEMPLATES]
                    ?.kpis || []
                ).map((kpi) => (
                  <Badge key={kpi} variant="secondary">
                    {kpi}
                  </Badge>
                ))}
              </div>
            </CardContent>
          </Card>

          {/* Métriques actuelles */}
          <CurrentMetricsConfig metrics={currentMetrics} onChange={setCurrentMetrics} />
        </TabsContent>

        {/* TAB 2: DONNÉES */}
        <TabsContent value="data" className="space-y-6">
          {/* Synthèse */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <Card className="bg-card/50 border-border/50">
              <CardContent className="pt-4">
                <p className="text-xs text-muted-foreground uppercase">Utilisateurs totaux</p>
                <p className="text-2xl font-bold mt-1">
                  {formatNumber(currentMetrics.total_users)}
                </p>
              </CardContent>
            </Card>
            <Card className="bg-card/50 border-border/50">
              <CardContent className="pt-4">
                <p className="text-xs text-muted-foreground uppercase">Nouveaux / période</p>
                <p className="text-2xl font-bold mt-1">
                  {formatNumber(currentMetrics.new_users_period)}
                </p>
              </CardContent>
            </Card>
            <Card className="bg-card/50 border-border/50">
              <CardContent className="pt-4">
                <p className="text-xs text-muted-foreground uppercase">CA Annuel</p>
                <p className="text-2xl font-bold mt-1">{formatCurrency(totalRevenue)}</p>
              </CardContent>
            </Card>
            <Card className="bg-card/50 border-border/50">
              <CardContent className="pt-4">
                <p className="text-xs text-muted-foreground uppercase">Charges annuelles</p>
                <p className="text-2xl font-bold mt-1">{formatCurrency(totalCosts)}</p>
              </CardContent>
            </Card>
          </div>

          {/* Sources de revenus */}
          <RevenueSourcesConfig sources={revenueSources} onChange={setRevenueSources} />

          {/* Structure de coûts */}
          <CostStructureConfig costStructure={costStructure} onChange={setCostStructure} />

          {/* Funnel */}
          <FunnelConfig stages={funnelStages} onChange={setFunnelStages} />

          {/* Import button */}
          <Button variant="outline" className="w-full">
            <Upload className="h-4 w-4 mr-2" />
            Importer des données
          </Button>
        </TabsContent>

        {/* TAB 3: SCÉNARIOS */}
        <TabsContent value="scenarios" className="space-y-6">
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* Left: Scenario manager */}
            <Card className="bg-card/50 border-border/50">
              <CardContent className="pt-6">
                <ScenarioManager
                  scenarios={scenarios}
                  activeScenario={activeScenario}
                  onSelectScenario={setActiveScenario}
                  onCreateScenario={handleCreateScenario}
                  onDeleteScenario={handleDeleteScenario}
                  horizonMonths={horizonMonths}
                  onHorizonChange={setHorizonMonths}
                  onRunSimulation={runSimulation}
                  isSimulating={isSimulating}
                />
              </CardContent>
            </Card>

            {/* Right: Driver panel */}
            <Card className="bg-card/50 border-border/50">
              <CardContent className="pt-6">
                <DriverPanel
                  drivers={drivers}
                  onDriverChange={handleDriverChange}
                  targetMode={targetMode}
                  onTargetModeChange={setTargetMode}
                  selectedDrivers={selectedDrivers}
                  onDriverToggle={handleDriverToggle}
                />
              </CardContent>
            </Card>
          </div>
        </TabsContent>

        {/* TAB 4: RÉSULTATS */}
        <TabsContent value="results" className="space-y-6">
          {simulationResult ? (
            <>
              {/* KPI cards */}
              <KPICards unitEconomics={simulationResult.unitEconomics} />

              {/* Charts grid */}
              <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                <RevenueEbitdaChart forecasts={simulationResult.forecasts} />
                <CashRunwayChart forecasts={simulationResult.forecasts} />
                <CustomersChart forecasts={simulationResult.forecasts} />
                <UnitEconomicsChart forecasts={simulationResult.forecasts} />
                <ARPUCACLTVChart forecasts={simulationResult.forecasts} />
                <RevenueVsChargesChart forecasts={simulationResult.forecasts} />
              </div>
            </>
          ) : (
            <Card className="bg-card/50 border-border/50">
              <CardContent className="py-16 text-center">
                <BarChart3 className="h-16 w-16 mx-auto text-muted-foreground mb-4" />
                <h3 className="text-lg font-semibold mb-2">Aucune simulation</h3>
                <p className="text-muted-foreground mb-4">
                  Créez un scénario et lancez une simulation pour voir les résultats
                </p>
                <Button onClick={() => setActiveTab("scenarios")}>
                  <Sliders className="h-4 w-4 mr-2" />
                  Aller aux scénarios
                </Button>
              </CardContent>
            </Card>
          )}
        </TabsContent>
      </Tabs>
    </div>
  );
}
