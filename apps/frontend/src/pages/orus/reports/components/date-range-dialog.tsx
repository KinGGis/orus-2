/**
 * DateRangeDialog - Dialog for selecting report date range
 */
import { useState } from "react";
import { format, subMonths, subDays } from "date-fns";
import { fr } from "date-fns/locale";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@wealthfolio/ui/components/ui/dialog";
import { Button } from "@wealthfolio/ui/components/ui/button";
import { Label } from "@wealthfolio/ui/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@wealthfolio/ui/components/ui/radio-group";
import { Calendar } from "@wealthfolio/ui/components/ui/calendar";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@wealthfolio/ui/components/ui/popover";
import { CalendarIcon, Loader2 } from "lucide-react";
import { cn } from "@wealthfolio/ui";

import type { DateRangeParams } from "@/features/orus-integration/reports-hooks";

type PeriodType = DateRangeParams["period_type"];

interface DateRangeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onGenerate: (dateRange: DateRangeParams) => void;
  isGenerating?: boolean;
}

const PERIOD_OPTIONS: { value: PeriodType; label: string; description: string }[] = [
  { value: "quarterly", label: "Trimestriel", description: "3 derniers mois" },
  { value: "semi-annual", label: "Semestriel", description: "6 derniers mois" },
  { value: "annual", label: "Annuel", description: "12 derniers mois" },
  { value: "all-time", label: "All-time", description: "Depuis le début" },
  { value: "custom", label: "Plage de dates personnalisée", description: "Sélectionnez vos dates" },
];

export function DateRangeDialog({
  open,
  onOpenChange,
  onGenerate,
  isGenerating = false,
}: DateRangeDialogProps) {
  const [periodType, setPeriodType] = useState<PeriodType>("quarterly");
  const [startDate, setStartDate] = useState<Date | undefined>(undefined);
  const [endDate, setEndDate] = useState<Date | undefined>(undefined);

  const handleGenerate = () => {
    const dateRange: DateRangeParams = {
      period_type: periodType,
    };

    if (periodType === "custom") {
      if (startDate) {
        dateRange.start_date = format(startDate, "yyyy-MM-dd");
      }
      if (endDate) {
        dateRange.end_date = format(endDate, "yyyy-MM-dd");
      }
    } else {
      // Calculate dates based on period type
      const today = new Date();
      dateRange.end_date = format(today, "yyyy-MM-dd");

      switch (periodType) {
        case "quarterly":
          dateRange.start_date = format(subMonths(today, 3), "yyyy-MM-dd");
          break;
        case "semi-annual":
          dateRange.start_date = format(subMonths(today, 6), "yyyy-MM-dd");
          break;
        case "annual":
          dateRange.start_date = format(subMonths(today, 12), "yyyy-MM-dd");
          break;
        case "all-time":
          // No start date for all-time
          break;
      }
    }

    onGenerate(dateRange);
  };

  const isValidSelection = periodType !== "custom" || (startDate && endDate);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Sélectionner la période du rapport</DialogTitle>
          <DialogDescription>
            Choisissez la période pour votre rapport de performance
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          <RadioGroup
            value={periodType}
            onValueChange={(value) => setPeriodType(value as PeriodType)}
            className="space-y-2"
          >
            {PERIOD_OPTIONS.map((option) => (
              <div
                key={option.value}
                className={cn(
                  "flex items-center space-x-3 rounded-lg border p-3 transition-colors",
                  periodType === option.value
                    ? "border-primary bg-primary/5"
                    : "border-border hover:bg-muted/50"
                )}
              >
                <RadioGroupItem value={option.value} id={option.value} />
                <Label htmlFor={option.value} className="flex-1 cursor-pointer">
                  <div className="font-medium">{option.label}</div>
                  <div className="text-xs text-muted-foreground">{option.description}</div>
                </Label>
              </div>
            ))}
          </RadioGroup>

          {/* Custom date pickers */}
          {periodType === "custom" && (
            <div className="grid grid-cols-2 gap-4 pt-2">
              <div className="space-y-2">
                <Label>Date de début</Label>
                <Popover>
                  <PopoverTrigger asChild>
                    <Button
                      variant="outline"
                      className={cn(
                        "w-full justify-start text-left font-normal",
                        !startDate && "text-muted-foreground"
                      )}
                    >
                      <CalendarIcon className="mr-2 h-4 w-4" />
                      {startDate ? format(startDate, "PPP", { locale: fr }) : "Sélectionner"}
                    </Button>
                  </PopoverTrigger>
                  <PopoverContent className="w-auto p-0" align="start">
                    <Calendar
                      mode="single"
                      selected={startDate}
                      onSelect={setStartDate}
                      disabled={(date) => date > new Date() || (endDate ? date > endDate : false)}
                      locale={fr}
                    />
                  </PopoverContent>
                </Popover>
              </div>
              <div className="space-y-2">
                <Label>Date de fin</Label>
                <Popover>
                  <PopoverTrigger asChild>
                    <Button
                      variant="outline"
                      className={cn(
                        "w-full justify-start text-left font-normal",
                        !endDate && "text-muted-foreground"
                      )}
                    >
                      <CalendarIcon className="mr-2 h-4 w-4" />
                      {endDate ? format(endDate, "PPP", { locale: fr }) : "Sélectionner"}
                    </Button>
                  </PopoverTrigger>
                  <PopoverContent className="w-auto p-0" align="start">
                    <Calendar
                      mode="single"
                      selected={endDate}
                      onSelect={setEndDate}
                      disabled={(date) =>
                        date > new Date() || (startDate ? date < startDate : false)
                      }
                      locale={fr}
                    />
                  </PopoverContent>
                </Popover>
              </div>
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Annuler
          </Button>
          <Button onClick={handleGenerate} disabled={!isValidSelection || isGenerating}>
            {isGenerating && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            Générer le rapport
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
