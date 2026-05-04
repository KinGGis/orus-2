/**
 * Orus Database Types (simplified for Wealthfolio integration)
 * Based on the Orus Supabase schema
 */

export type Json = string | number | boolean | null | { [key: string]: Json | undefined } | Json[];

// =====================================================
// CORE ORUS TYPES
// =====================================================

export interface OrusPosition {
  id: string;
  date: string;
  instrument: string | null;
  instrument_id: string | null;
  quantity: number | null;
  price: number | null;
  market_value: number | null;
  currency: string | null;
  asset_class: string | null;
  asset_class_id: string | null;
  sector: string | null;
  country: string | null;
  status: string | null;
  source: string | null;
  portfolio_id: string | null;
  created_at: string | null;
  // Bond specific fields
  face_value: number | null;
  nominal_rate: number | null;
  maturity_date: string | null;
  issue_date: string | null;
  credit_rating: string | null;
  ytm: number | null;
  modified_duration: number | null;
  convexity: number | null;
  accrued_interest: number | null;
  clean_price: number | null;
  dirty_price: number | null;
  daily_variation_percent: number | null;
}

export interface OrusInstrument {
  id: string;
  symbol: string;
  name: string | null;
  currency: string | null;
  country: string | null;
  sector: string | null;
  asset_class_id: string | null;
  yahoo_symbol: string | null;
  is_active: boolean | null;
  created_at: string | null;
  updated_at: string | null;
}

export interface OrusAssetClass {
  id: string;
  name: string;
  description: string | null;
  parent_id: string | null;
  display_order: number | null;
  created_at: string | null;
}

export interface OrusTrade {
  id: string;
  date: string;
  position_id: string | null;
  instrument_id: string | null;
  trade_type: string;
  quantity: number;
  price: number;
  amount: number | null;
  currency: string | null;
  fees: number | null;
  notes: string | null;
  source: string | null;
  created_at: string | null;
}

export interface OrusProfile {
  id: string;
  email: string | null;
  full_name: string | null;
  avatar_url: string | null;
  preferred_currency: string | null;
  created_at: string | null;
  updated_at: string | null;
}

export interface OrusCashPosition {
  id: string;
  date: string;
  currency: string;
  amount: number;
  source: string | null;
  notes: string | null;
  created_at: string | null;
}

export interface OrusNav {
  id: string;
  date: string;
  nav_value: number | null;
  variation: number | null;
  comments: string | null;
  created_at: string | null;
}

export interface OrusPortfolioSummary {
  id: string;
  date: string;
  total_aum: number | null;
  cash_amount: number | null;
  invested_amount: number | null;
  daily_pnl: number | null;
  mtd_pnl: number | null;
  ytd_pnl: number | null;
  currency: string | null;
  created_at: string | null;
}

// =====================================================
// PRIVATE EQUITY TYPES
// =====================================================

export interface OrusPECompany {
  id: string;
  name: string;
  description: string | null;
  country: string | null;
  currency: string | null;
  founded_year: number | null;
  employees_count: number | null;
  business_model: string | null;
  is_cfo_managed: boolean | null;
  created_at: string | null;
}

export interface OrusPEInvestment {
  id: string;
  company_id: string;
  investment_date: string;
  amount: number;
  currency: string;
  ownership_percentage: number | null;
  investment_type: string | null;
  status: string | null;
  notes: string | null;
  created_at: string | null;
}

// =====================================================
// ACCOUNTING TYPES
// =====================================================

export interface OrusAccountingPeriod {
  id: string;
  name: string;
  start_date: string;
  end_date: string;
  entity_id: string;
  status: "draft" | "validated" | "closed";
  currency: string;
  notes: string | null;
  created_by: string | null;
  created_at: string;
  updated_at: string;
}

export interface OrusAccountingEntry {
  id: string;
  entry_date: string;
  account_code: string;
  description: string;
  debit: number | null;
  credit: number | null;
  currency: string;
  period_id: string;
  source_type: string | null;
  source_id: string | null;
  validated: boolean | null;
  ai_generated: boolean | null;
  ai_explanation: string | null;
  created_at: string;
}

export interface OrusChartOfAccounts {
  id: string;
  code: string;
  name: string;
  type: "asset" | "liability" | "equity" | "income" | "expense";
  parent_id: string | null;
  description: string | null;
  hmrc_box: string | null;
  is_active: boolean;
  sort_order: number | null;
}

// =====================================================
// REVOLUT TYPES
// =====================================================

export interface OrusRevolutAccount {
  id: string;
  revolut_account_id: string;
  name: string;
  currency: string;
  balance: number;
  state: string;
  last_synced_at: string | null;
  created_at: string | null;
  updated_at: string | null;
}

export interface OrusRevolutTransaction {
  id: string;
  revolut_transaction_id: string;
  revolut_account_id: string;
  type: string;
  state: string;
  currency: string;
  amount: number;
  description: string | null;
  merchant_name: string | null;
  merchant_category: string | null;
  completed_at: string | null;
  balance_after: number | null;
  created_at: string | null;
}

// =====================================================
// SHAREHOLDER TYPES
// =====================================================

export interface OrusShareholderParticipation {
  id: string;
  user_id: string;
  shares_count: number;
  share_value: number;
  total_value: number; // computed: shares_count * share_value
  currency: string;
  investment_date: string | null;
  last_valuation_date: string | null;
  notes: string | null;
  created_at: string | null;
  updated_at: string | null;
  // Joined from profiles
  profiles?: {
    full_name: string | null;
    email: string | null;
  };
}

export interface OrusShareholderComplianceInfo {
  id: string;
  user_id: string;
  first_name: string | null;
  last_name: string | null;
  address_line1: string | null;
  address_line2: string | null;
  postal_code: string | null;
  city: string | null;
  country: string | null;
  email: string | null;
  phone: string | null;
  is_verified: boolean;
  verified_at: string | null;
  created_at: string | null;
  updated_at: string | null;
}

export interface OrusShareholderDocument {
  id: string;
  user_id: string;
  document_type: string;
  document_name: string;
  file_path: string;
  file_size: number | null;
  mime_type: string | null;
  status: string;
  uploaded_at: string | null;
  reviewed_at: string | null;
  notes: string | null;
}

export interface OrusShareholderDocumentRequest {
  id: string;
  user_id: string;
  requested_by: string;
  document_type: string;
  description: string | null;
  due_date: string | null;
  status: string;
  created_at: string | null;
  fulfilled_at: string | null;
  fulfilled_document_id: string | null;
}

export interface OrusShareholderWithProfile {
  participation: OrusShareholderParticipation;
  profile: {
    id: string;
    email: string | null;
    full_name: string | null;
  } | null;
  complianceInfo?: OrusShareholderComplianceInfo | null;
  documents?: OrusShareholderDocument[];
  pendingRequests?: number;
  documentRequests?: OrusShareholderDocumentRequest[];
}

export type OrusUserRole = 'superadmin' | 'admin' | 'analyst' | 'trader' | 'portfolio_manager' | 'viewer' | null;

// =====================================================
// DATABASE SCHEMA TYPE
// =====================================================

export interface OrusDatabase {
  public: {
    Tables: {
      positions: {
        Row: OrusPosition;
        Insert: Partial<OrusPosition> & { date: string };
        Update: Partial<OrusPosition>;
      };
      instruments: {
        Row: OrusInstrument;
        Insert: Partial<OrusInstrument> & { symbol: string };
        Update: Partial<OrusInstrument>;
      };
      asset_classes: {
        Row: OrusAssetClass;
        Insert: Partial<OrusAssetClass> & { name: string };
        Update: Partial<OrusAssetClass>;
      };
      trades: {
        Row: OrusTrade;
        Insert: Partial<OrusTrade> & { date: string; trade_type: string; quantity: number; price: number };
        Update: Partial<OrusTrade>;
      };
      profiles: {
        Row: OrusProfile;
        Insert: Partial<OrusProfile>;
        Update: Partial<OrusProfile>;
      };
      cash_positions: {
        Row: OrusCashPosition;
        Insert: Partial<OrusCashPosition> & { date: string; currency: string; amount: number };
        Update: Partial<OrusCashPosition>;
      };
      navs: {
        Row: OrusNav;
        Insert: Partial<OrusNav> & { date: string };
        Update: Partial<OrusNav>;
      };
      portfolio_summary: {
        Row: OrusPortfolioSummary;
        Insert: Partial<OrusPortfolioSummary> & { date: string };
        Update: Partial<OrusPortfolioSummary>;
      };
      pe_companies: {
        Row: OrusPECompany;
        Insert: Partial<OrusPECompany> & { name: string };
        Update: Partial<OrusPECompany>;
      };
      pe_investments: {
        Row: OrusPEInvestment;
        Insert: Partial<OrusPEInvestment> & {
          company_id: string;
          investment_date: string;
          amount: number;
          currency: string;
        };
        Update: Partial<OrusPEInvestment>;
      };
      accounting_entries: {
        Row: OrusAccountingEntry;
        Insert: Partial<OrusAccountingEntry> & {
          entry_date: string;
          account_code: string;
          description: string;
          period_id: string;
        };
        Update: Partial<OrusAccountingEntry>;
      };
      chart_of_accounts: {
        Row: OrusChartOfAccounts;
        Insert: Partial<OrusChartOfAccounts> & { code: string; name: string; account_type: string };
        Update: Partial<OrusChartOfAccounts>;
      };
      shareholder_participations: {
        Row: OrusShareholderParticipation;
        Insert: Partial<OrusShareholderParticipation> & {
          user_id: string;
        };
        Update: Partial<OrusShareholderParticipation>;
      };
      shareholder_compliance_info: {
        Row: OrusShareholderComplianceInfo;
        Insert: Partial<OrusShareholderComplianceInfo> & { user_id: string };
        Update: Partial<OrusShareholderComplianceInfo>;
      };
      shareholder_documents: {
        Row: OrusShareholderDocument;
        Insert: Partial<OrusShareholderDocument> & {
          user_id: string;
          document_type: string;
          document_name: string;
          file_path: string;
        };
        Update: Partial<OrusShareholderDocument>;
      };
      shareholder_document_requests: {
        Row: OrusShareholderDocumentRequest;
        Insert: Partial<OrusShareholderDocumentRequest> & {
          user_id: string;
          requested_by: string;
          document_type: string;
        };
        Update: Partial<OrusShareholderDocumentRequest>;
      };
      user_roles: {
        Row: {
          id: string;
          user_id: string;
          role: OrusUserRole;
          created_at: string | null;
        };
        Insert: { user_id: string; role: string };
        Update: Partial<{ role: string }>;
      };
    };
    Views: Record<string, never>;
    Functions: Record<string, never>;
    Enums: Record<string, never>;
  };
}
