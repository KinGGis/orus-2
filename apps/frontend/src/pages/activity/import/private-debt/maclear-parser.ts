import * as XLSX from "xlsx";

/** A loan of the platform's loan list, as sent to `POST /private-debt/import`. */
export interface ImportedLoan {
  projectId: string;
  borrower: string;
  purchaseDate: string;
  nextPaymentDate: string | null;
  /** Annual rate, in percent. */
  rate: string;
  loyaltyBonus: string | null;
  projectType: string | null;
  totalMonths: number;
  remainingMonths: number;
  countryCode: string | null;
  status: string | null;
  nominal: string;
  currency: string;
}

/** A line of the account statement; `amount` is signed. */
export interface ImportedTransaction {
  timestamp: string;
  kind: string;
  projectId: string | null;
  amount: string;
  currency: string;
}

export interface ParseResult<T> {
  rows: T[];
  errors: string[];
}

type Row = Record<string, string>;

/** Rows of the first sheet, with trimmed, BOM-free headers and text cells. */
export function readSheet(data: ArrayBuffer): Row[] {
  const workbook = XLSX.read(new Uint8Array(data), { type: "array" });
  const sheet = workbook.Sheets[workbook.SheetNames[0] ?? ""];
  if (!sheet) return [];
  const raw = XLSX.utils.sheet_to_json<Record<string, unknown>>(sheet, {
    raw: false,
    defval: "",
  });
  return raw.map((row) => {
    const clean: Row = {};
    for (const [key, value] of Object.entries(row)) {
      clean[key.replace(/\uFEFF/g, "").trim()] = typeof value === "string" ? value.trim() : "";
    }
    return clean;
  });
}

/** "dd.mm.yyyy[ HH:MM:SS]" → "yyyy-mm-dd" and the time, if any. */
function parseDate(value: string): { date: string; time: string | null } | null {
  const match = /^(\d{1,2})\.(\d{1,2})\.(\d{4})(?:\s+(\d{1,2}):(\d{2})(?::(\d{2}))?)?$/.exec(
    value.trim(),
  );
  if (!match) return null;
  const [, d, m, y, hh, mm, ss] = match;
  const date = `${y}-${m.padStart(2, "0")}-${d.padStart(2, "0")}`;
  const time = hh ? `${hh.padStart(2, "0")}:${mm}:${ss ?? "00"}` : null;
  return { date, time };
}

/** "+1,234.56", "12.5 %" → "1234.56", "12.5"; null when not a number. */
export function parseNumber(value: string): string | null {
  let text = value.replace(/[\s%+\u00A0]/g, "");
  if (text.includes(",") && text.includes(".")) text = text.replace(/,/g, "");
  else text = text.replace(",", ".");
  return text !== "" && !Number.isNaN(Number(text)) ? text : null;
}

function nonEmpty(value: string | undefined): string | null {
  const text = (value ?? "").trim();
  return text === "" || text === "-" || text === "/" ? null : text;
}

/** Account statement: Date, Details, Amount, Currency. */
export function parseStatement(rows: Row[]): ParseResult<ImportedTransaction> {
  const result: ParseResult<ImportedTransaction> = { rows: [], errors: [] };
  rows.forEach((row, index) => {
    const line = index + 2;
    const details = row.Details ?? "";
    if (!row.Date && !details) return;
    const date = parseDate(row.Date ?? "");
    const amount = parseNumber(row.Amount ?? "");
    if (!date || amount === null || !details) {
      result.errors.push(`Relevé, ligne ${line} : date, détail ou montant illisible`);
      return;
    }
    const project = / (PJ\d+)\b/.exec(details);
    result.rows.push({
      timestamp: `${date.date}T${date.time ?? "00:00:00"}`,
      kind: (project ? details.slice(0, project.index) : details).trim(),
      projectId: project ? project[1] : null,
      amount,
      currency: (row.Currency ?? "").toUpperCase(),
    });
  });
  return result;
}

/** Loan list: Project/ID, Date of purchase, Next payment, Interest rate, … */
export function parseLoans(rows: Row[]): ParseResult<ImportedLoan> {
  const result: ParseResult<ImportedLoan> = { rows: [], errors: [] };
  rows.forEach((row, index) => {
    const line = index + 2;
    const project = row["Project/ID"] ?? "";
    if (!project) return;
    const separator = project.lastIndexOf("/");
    const projectId = project.slice(separator + 1).trim();
    const borrower = project.slice(0, Math.max(separator, 0)).trim();
    const purchase = parseDate(row["Date of purchase"] ?? "");
    const nextPayment = parseDate((row["Next payment"] ?? "").split("/")[0] ?? "");
    const rate = parseNumber(row["Interest rate"] ?? "");
    const [total, remaining] = (row["Loan period"] ?? "")
      .split("/")
      .map((part) => Number.parseInt(part.trim(), 10));
    const nominal = parseNumber(row["Invested amount"] ?? "");
    if (
      !/^PJ\d+$/.test(projectId) ||
      !purchase ||
      rate === null ||
      !Number.isFinite(total) ||
      !Number.isFinite(remaining) ||
      nominal === null
    ) {
      result.errors.push(`Conditions, ligne ${line} (${project}) : données illisibles`);
      return;
    }
    const loyalty = nonEmpty(row["Loyalty bonus"]);
    result.rows.push({
      projectId,
      borrower: borrower || projectId,
      purchaseDate: purchase.date,
      nextPaymentDate: nextPayment?.date ?? null,
      rate,
      loyaltyBonus: loyalty === null ? null : parseNumber(loyalty),
      projectType: nonEmpty(row["Project type"]),
      totalMonths: total,
      remainingMonths: remaining,
      countryCode: nonEmpty((row.Country ?? "").split("/")[0])?.toUpperCase() ?? null,
      status: nonEmpty(row.Status),
      nominal,
      currency: (row.Currency ?? "").toUpperCase(),
    });
  });
  return result;
}
