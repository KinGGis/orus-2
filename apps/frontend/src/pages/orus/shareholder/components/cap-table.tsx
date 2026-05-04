/**
 * Cap Table Component - Shows all shareholders and their holdings
 * Admin view with full visibility of shareholder names and values
 */
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@wealthfolio/ui/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@wealthfolio/ui/components/ui/table";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Icons } from "@wealthfolio/ui/components/ui/icons";
import { formatCurrency } from "@/lib/format";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import type { OrusShareholderWithProfile } from "@/features/orus-integration";

interface CapTableProps {
  shareholders: OrusShareholderWithProfile[];
  currency: string;
  isAdmin?: boolean;
}

export function CapTable({ shareholders, currency, isAdmin = false }: CapTableProps) {
  // Calculate totals
  const totalShares = shareholders.reduce((sum, s) => sum + s.participation.shares_count, 0);
  const totalValue = shareholders.reduce((sum, s) => sum + (s.participation.total_value || 0), 0);

  // Sort by shares count descending
  const sortedShareholders = [...shareholders].sort(
    (a, b) => b.participation.shares_count - a.participation.shares_count
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base sm:text-lg">
          <Icons.FileText className="h-4 w-4 text-primary sm:h-5 sm:w-5" />
          Cap Table
        </CardTitle>
        <CardDescription>
          {shareholders.length} actionnaire{shareholders.length > 1 ? "s" : ""} •{" "}
          {totalShares.toLocaleString()} parts • {formatCurrency(totalValue, currency)}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-[200px]">Actionnaire</TableHead>
                <TableHead className="text-right">Parts</TableHead>
                <TableHead className="text-right">% Capital</TableHead>
                <TableHead className="text-right">Valeur/Part</TableHead>
                <TableHead className="text-right">Valeur Totale</TableHead>
                <TableHead className="text-right">Date d'entrée</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {sortedShareholders.map((shareholder, index) => {
                const ownershipPercent =
                  totalShares > 0
                    ? ((shareholder.participation.shares_count / totalShares) * 100).toFixed(2)
                    : "0";

                const displayName = isAdmin
                  ? shareholder.profile?.full_name ||
                    shareholder.profile?.email ||
                    "Utilisateur inconnu"
                  : `Actionnaire ${index + 1}`;

                return (
                  <TableRow key={shareholder.participation.id}>
                    <TableCell>
                      <div className="flex items-center gap-2">
                        <div className="bg-primary/10 flex h-8 w-8 items-center justify-center rounded-full">
                          <Icons.User className="text-primary h-4 w-4" />
                        </div>
                        <div>
                          <p className="font-medium">{displayName}</p>
                          {isAdmin && shareholder.profile?.email && shareholder.profile.full_name && (
                            <p className="text-muted-foreground text-xs">
                              {shareholder.profile.email}
                            </p>
                          )}
                        </div>
                      </div>
                    </TableCell>
                    <TableCell className="text-right font-medium">
                      {shareholder.participation.shares_count.toLocaleString()}
                    </TableCell>
                    <TableCell className="text-right">
                      <Badge variant="secondary" className="font-mono">
                        {ownershipPercent}%
                      </Badge>
                    </TableCell>
                    <TableCell className="text-right">
                      {formatCurrency(
                        shareholder.participation.share_value,
                        shareholder.participation.currency
                      )}
                    </TableCell>
                    <TableCell className="text-primary text-right font-semibold">
                      {formatCurrency(
                        shareholder.participation.total_value || 0,
                        shareholder.participation.currency
                      )}
                    </TableCell>
                    <TableCell className="text-muted-foreground text-right text-sm">
                      {shareholder.participation.investment_date
                        ? format(
                            new Date(shareholder.participation.investment_date),
                            "dd MMM yyyy",
                            { locale: fr }
                          )
                        : "-"}
                    </TableCell>
                  </TableRow>
                );
              })}
              {/* Totals row */}
              <TableRow className="bg-muted/30 font-semibold">
                <TableCell>Total</TableCell>
                <TableCell className="text-right">{totalShares.toLocaleString()}</TableCell>
                <TableCell className="text-right">
                  <Badge variant="default" className="font-mono">
                    100%
                  </Badge>
                </TableCell>
                <TableCell className="text-right">-</TableCell>
                <TableCell className="text-primary text-right">
                  {formatCurrency(totalValue, currency)}
                </TableCell>
                <TableCell className="text-right">-</TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>
  );
}

export default CapTable;
