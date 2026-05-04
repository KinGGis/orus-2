import { getDynamicNavItems, subscribeToNavigationUpdates } from "@/addons/addons-runtime-context";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { useEffect, useState, useMemo } from "react";
import { useModuleVisibility } from "@/features/orus-integration";

export interface NavLink {
  title: string;
  href: string;
  icon?: React.ReactNode;
  keywords?: string[];
  label?: string; // Optional descriptive label for launcher/search
  moduleKey?: string; // Optional key to check module visibility
}

export interface NavigationProps {
  primary: NavLink[];
  secondary?: NavLink[];
  addons?: NavLink[];
}

const staticNavigation: NavigationProps = {
  primary: [
    {
      icon: <Icons.Dashboard className="size-6" />,
      title: "Dashboard",
      href: "/dashboard",
      keywords: ["home", "overview", "summary"],
      label: "View Dashboard",
    },
    {
      icon: <Icons.Insight className="size-6" />,
      title: "Insights",
      href: "/insights",
      keywords: ["insights", "Analytics"],
      label: "View Insights",
    },
    {
      icon: <Icons.Holdings className="size-6" />,
      title: "Holdings",
      href: "/holdings",
      keywords: ["Holdings", "portfolio", "assets", "positions", "stocks"],
      label: "View Holdings",
    },
    {
      icon: <Icons.Activity className="size-6" />,
      title: "Activities",
      href: "/activities",
      keywords: ["transactions", "trades", "history"],
      label: "View Activities",
    },
    {
      icon: <Icons.Sparkles className="size-6" />,
      title: "Assistant",
      href: "/assistant",
      keywords: ["ai", "assistant", "chat", "help", "ask"],
      label: "AI Assistant",
    },
  ],
  secondary: [
    {
      icon: <Icons.Briefcase className="size-6" />,
      title: "Private Equity",
      href: "/private-equity",
      keywords: ["pe", "private equity", "investments", "companies"],
      label: "Private Equity",
      moduleKey: "privateEquity",
    },
    {
      icon: <Icons.Receipt className="size-6" />,
      title: "Accounting",
      href: "/accounting",
      keywords: ["accounting", "journal", "entries", "bookkeeping"],
      label: "Accounting",
      moduleKey: "accounting",
    },
    {
      icon: <Icons.Users className="size-6" />,
      title: "Shareholders",
      href: "/shareholders",
      keywords: ["shareholders", "ownership", "equity", "participations"],
      label: "Shareholders",
      moduleKey: "shareholder",
    },
    {
      icon: <Icons.FileText className="size-6" />,
      title: "Reports",
      href: "/reports",
      keywords: ["reports", "reporting", "gamma", "presentation", "pdf"],
      label: "Reports & Presentations",
      moduleKey: "reports",
    },
    {
      icon: <Icons.Shield className="size-6" />,
      title: "Administration",
      href: "/administration",
      keywords: ["admin", "administration", "users", "permissions", "ibkr", "revolut"],
      label: "Administration",
      moduleKey: "admin",
    },
    {
      icon: <Icons.Settings className="size-6" />,
      title: "Settings",
      href: "/settings",
      keywords: ["preferences", "config", "configuration"],
      // Settings is always visible
    },
  ],
};

export function useNavigation() {
  const [dynamicItems, setDynamicItems] = useState<NavigationProps["addons"]>([]);
  const { modules, loading, userRole } = useModuleVisibility();

  // Subscribe to navigation updates from addons
  useEffect(() => {
    const updateDynamicItems = () => {
      const itemsFromRuntime = getDynamicNavItems();
      setDynamicItems(itemsFromRuntime);
    };

    // Initial load
    updateDynamicItems();

    // Subscribe to updates
    const unsubscribe = subscribeToNavigationUpdates(updateDynamicItems);

    return () => {
      unsubscribe();
    };
  }, []);

  // Filter secondary navigation based on module visibility
  const filteredSecondary = useMemo(() => {
    // If loading or superadmin, show all items
    if (loading || userRole === "superadmin") {
      return staticNavigation.secondary;
    }

    return (staticNavigation.secondary || []).filter((item) => {
      // Items without moduleKey are always visible
      if (!item.moduleKey) return true;

      // Check module visibility
      const moduleKeyMap: Record<string, keyof typeof modules> = {
        privateEquity: "privateEquity",
        accounting: "accounting",
        shareholder: "shareholder",
        reports: "reports",
        admin: "admin",
      };

      const key = moduleKeyMap[item.moduleKey];
      return key ? modules[key] : true;
    });
  }, [modules, loading, userRole]);

  // Combine static navigation items with addons grouped separately
  const navigation: NavigationProps = {
    primary: staticNavigation.primary,
    secondary: filteredSecondary,
    addons: dynamicItems,
  };

  return navigation;
}

export function isPathActive(pathname: string, href: string): boolean {
  if (!href) {
    return false;
  }

  const ensureLeadingSlash = href.startsWith("/") ? href : `/${href}`;
  const normalize = (value: string) => {
    if (value.length > 1 && value.endsWith("/")) {
      return value.slice(0, -1);
    }
    return value;
  };

  const normalizedHref = normalize(ensureLeadingSlash);
  const normalizedPath = normalize(pathname);

  if (normalizedHref === "/") {
    return normalizedPath === "/";
  }

  // Dashboard and Net Worth are grouped together
  if (normalizedHref === "/dashboard") {
    return (
      normalizedPath === "/" || normalizedPath === "/dashboard" || normalizedPath === "/net-worth"
    );
  }

  return normalizedPath === normalizedHref || normalizedPath.startsWith(`${normalizedHref}/`);
}
