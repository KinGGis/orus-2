import { describe, expect, it } from "vitest";
import { parseLoans, parseNumber, parseStatement } from "./maclear-parser";

describe("maclear-parser", () => {
  it("splits statement details into type and project", () => {
    const { rows, errors } = parseStatement([
      {
        Date: "17.11.2026 04:00:13",
        Details: "Interest payment PJ2262 VLAD",
        Amount: "+1.83",
        Currency: "eur",
      },
      { Date: "23.01.2025 10:00:00", Details: "Deposit", Amount: "+500.00", Currency: "EUR" },
      { Date: "bad", Details: "Investment PJ1 X", Amount: "-1", Currency: "EUR" },
    ]);
    expect(rows).toEqual([
      {
        timestamp: "2026-11-17T04:00:13",
        kind: "Interest payment",
        projectId: "PJ2262",
        amount: "1.83",
        currency: "EUR",
      },
      {
        timestamp: "2025-01-23T10:00:00",
        kind: "Deposit",
        projectId: null,
        amount: "500.00",
        currency: "EUR",
      },
    ]);
    expect(errors).toHaveLength(1);
  });

  it("reads loan terms", () => {
    const { rows, errors } = parseLoans([
      {
        "Project/ID": "Merchant LP / PJ1602",
        "Date of purchase": "06.04.2026",
        "Next payment": "06.12.2026 / 0.36",
        "Interest rate": "14.5%",
        "Loyalty bonus": "",
        "Project type": "Wholesale and services ",
        "Loan period": "13 / 6",
        Country: "GB / United Kingdom",
        Status: "Funded",
        "Invested amount": "30",
        Currency: "EUR",
      },
      {
        "Project/ID": "Repaid one / PJ509",
        "Date of purchase": "03.06.2025",
        "Next payment": "/",
        "Interest rate": "15",
        "Loan period": "9 / 0",
        "Invested amount": "114.25",
        Currency: "EUR",
      },
    ]);
    expect(errors).toEqual([]);
    expect(rows[0]).toMatchObject({
      projectId: "PJ1602",
      borrower: "Merchant LP",
      purchaseDate: "2026-04-06",
      nextPaymentDate: "2026-12-06",
      rate: "14.5",
      loyaltyBonus: null,
      projectType: "Wholesale and services",
      totalMonths: 13,
      remainingMonths: 6,
      countryCode: "GB",
    });
    expect(rows[1].nextPaymentDate).toBeNull();
  });

  it("normalises numbers", () => {
    expect(parseNumber("-1,234.56")).toBe("-1234.56");
    expect(parseNumber("12,5 %")).toBe("12.5");
    expect(parseNumber("n/a")).toBeNull();
  });
});
