import { useState, useEffect } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@wealthfolio/ui/components/ui/card";
import { Switch } from "@wealthfolio/ui/components/ui/switch";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Input } from "@wealthfolio/ui/components/ui/input";
import { Label } from "@wealthfolio/ui/components/ui/label";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { Separator } from "@wealthfolio/ui/components/ui/separator";
import { Alert, AlertDescription } from "@wealthfolio/ui/components/ui/alert";
import { SettingsHeader } from "../settings-header";
import { orusSupabase } from "@/features/orus-integration";

interface OrusModuleConfig {
  privateEquity: boolean;
  accounting: boolean;
  shareholder: boolean;
}

interface ConnectionStatus {
  connected: boolean;
  lastChecked: Date | null;
  error: string | null;
}

const DEFAULT_CONFIG: OrusModuleConfig = {
  privateEquity: true,
  accounting: true,
  shareholder: true,
};

function ModuleCard({
  title,
  description,
  icon,
  enabled,
  onToggle,
}: {
  title: string;
  description: string;
  icon: React.ReactNode;
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
}) {
  return (
    <Card className={enabled ? "" : "opacity-60"}>
      <CardContent className="flex items-center justify-between p-4">
        <div className="flex items-center gap-4">
          <div className="text-muted-foreground">{icon}</div>
          <div>
            <p className="font-medium">{title}</p>
            <p className="text-muted-foreground text-sm">{description}</p>
          </div>
        </div>
        <Switch checked={enabled} onCheckedChange={onToggle} />
      </CardContent>
    </Card>
  );
}

export default function OrusSettingsPage() {
  const [config, setConfig] = useState<OrusModuleConfig>(DEFAULT_CONFIG);
  const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>({
    connected: false,
    lastChecked: null,
    error: null,
  });
  const [isTesting, setIsTesting] = useState(false);

  // Load config from localStorage on mount
  useEffect(() => {
    const savedConfig = localStorage.getItem("orus-module-config");
    if (savedConfig) {
      try {
        setConfig(JSON.parse(savedConfig));
      } catch {
        // Ignore parse errors
      }
    }
  }, []);

  // Save config to localStorage when it changes
  useEffect(() => {
    localStorage.setItem("orus-module-config", JSON.stringify(config));
  }, [config]);

  const testConnection = async () => {
    setIsTesting(true);
    setConnectionStatus((prev) => ({ ...prev, error: null }));

    try {
      // Test the Supabase connection by making a simple query
      const { error } = await orusSupabase.from("profiles").select("id").limit(1);

      if (error) {
        throw error;
      }

      setConnectionStatus({
        connected: true,
        lastChecked: new Date(),
        error: null,
      });
    } catch (err) {
      setConnectionStatus({
        connected: false,
        lastChecked: new Date(),
        error: (err as Error).message,
      });
    } finally {
      setIsTesting(false);
    }
  };

  const handleModuleToggle = (module: keyof OrusModuleConfig) => (enabled: boolean) => {
    setConfig((prev) => ({ ...prev, [module]: enabled }));
  };

  return (
    <div className="space-y-6">
      <SettingsHeader
        heading="Orus Integration"
        text="Configure Orus modules and connection settings"
      />

      {/* Connection Status */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icons.CloudSync2 className="h-5 w-5" />
            Connection Status
          </CardTitle>
          <CardDescription>
            Connect to your Orus Supabase database to access extended modules
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div
                className={`h-3 w-3 rounded-full ${
                  connectionStatus.connected ? "bg-green-500" : "bg-red-500"
                }`}
              />
              <div>
                <p className="font-medium">
                  {connectionStatus.connected ? "Connected" : "Not Connected"}
                </p>
                {connectionStatus.lastChecked && (
                  <p className="text-muted-foreground text-sm">
                    Last checked: {connectionStatus.lastChecked.toLocaleTimeString()}
                  </p>
                )}
              </div>
            </div>
            <Button onClick={testConnection} disabled={isTesting} variant="outline" size="sm">
              {isTesting ? (
                <>
                  <Icons.Loader className="mr-2 h-4 w-4 animate-spin" />
                  Testing...
                </>
              ) : (
                <>
                  <Icons.Refresh className="mr-2 h-4 w-4" />
                  Test Connection
                </>
              )}
            </Button>
          </div>

          {connectionStatus.error && (
            <Alert variant="destructive">
              <Icons.AlertCircle className="h-4 w-4" />
              <AlertDescription>{connectionStatus.error}</AlertDescription>
            </Alert>
          )}

          <Separator />

          <div className="space-y-3">
            <Label>Supabase URL</Label>
            <Input
              value="https://bujfwfqmfsgaibnmuvml.supabase.co"
              readOnly
              disabled
              className="font-mono text-sm"
            />
            <p className="text-muted-foreground text-xs">
              The Supabase URL is pre-configured and cannot be changed.
            </p>
          </div>
        </CardContent>
      </Card>

      {/* Module Configuration */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Icons.Blocks className="h-5 w-5" />
            Orus Modules
          </CardTitle>
          <CardDescription>
            Enable or disable Orus modules to customize your experience
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <ModuleCard
            title="Private Equity"
            description="Track PE investments, companies, and portfolio performance"
            icon={<Icons.Briefcase className="h-6 w-6" />}
            enabled={config.privateEquity}
            onToggle={handleModuleToggle("privateEquity")}
          />

          <ModuleCard
            title="Accounting"
            description="Journal entries, chart of accounts, and financial statements"
            icon={<Icons.Receipt className="h-6 w-6" />}
            enabled={config.accounting}
            onToggle={handleModuleToggle("accounting")}
          />

          <ModuleCard
            title="Shareholder Management"
            description="Track shareholders, ownership distribution, and participations"
            icon={<Icons.Users className="h-6 w-6" />}
            enabled={config.shareholder}
            onToggle={handleModuleToggle("shareholder")}
          />
        </CardContent>
      </Card>

      {/* Info Card */}
      <Card className="border-blue-200 bg-blue-50 dark:border-blue-900 dark:bg-blue-950/30">
        <CardContent className="flex items-start gap-4 p-4">
          <Icons.InfoCircle className="h-5 w-5 text-blue-600 shrink-0 mt-0.5" />
          <div className="space-y-1">
            <p className="font-medium text-blue-900 dark:text-blue-100">About Orus Integration</p>
            <p className="text-sm text-blue-800 dark:text-blue-200">
              Orus modules extend Wealthfolio with additional portfolio management features. 
              Data is stored in the Orus Supabase database and synchronized in real-time.
              Disabling a module will hide its navigation entry but won't delete any data.
            </p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
