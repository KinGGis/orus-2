-- Private debt: expected cash-flow schedules and floating-rate fixings.
--
-- A private loan is held as a BOND asset whose terms (rate type, index, margin,
-- PIK, amortisation, valuation method…) live in wf_assets.metadata->'bond'.
-- What the terms alone cannot hold is the schedule they imply and how each
-- expected flow was actually settled: that is what monitoring a loan book is
-- about (what is due, what was paid, what is late).

CREATE TABLE wf_debt_schedule_flows (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id UUID NOT NULL REFERENCES wf_accounts(id) ON DELETE CASCADE,
    asset_id UUID NOT NULL REFERENCES wf_assets(id) ON DELETE CASCADE,
    flow_date DATE NOT NULL,
    -- INTEREST_CASH is paid out, INTEREST_PIK is capitalised into principal,
    -- PRINCIPAL is a (partial) repayment, FEE is charged to the lender.
    flow_type TEXT NOT NULL,
    -- Start of the accrual period the flow pays for; NULL for principal/fees.
    period_start DATE,
    -- Amounts are positive, in the asset currency.
    expected_amount NUMERIC NOT NULL,
    currency TEXT NOT NULL,
    -- Annual rate the interest was computed with (fixing + margin, floored).
    rate_applied NUMERIC,
    -- Outstanding principal at period start, so a flow can be re-derived.
    principal_before NUMERIC,
    status TEXT NOT NULL DEFAULT 'EXPECTED',
    received_amount NUMERIC,
    received_date DATE,
    -- The activity that settled the flow, once reconciled.
    activity_id UUID REFERENCES wf_activities(id) ON DELETE SET NULL,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT wf_debt_schedule_flows_unique UNIQUE (account_id, asset_id, flow_date, flow_type),
    CONSTRAINT wf_debt_schedule_flows_type_check
        CHECK (flow_type IN ('INTEREST_CASH', 'INTEREST_PIK', 'PRINCIPAL', 'FEE')),
    CONSTRAINT wf_debt_schedule_flows_status_check
        CHECK (status IN ('EXPECTED', 'RECEIVED', 'PARTIAL', 'LATE', 'WAIVED'))
);

-- Upcoming / overdue flows across the book.
CREATE INDEX idx_wf_debt_schedule_flows_due
ON wf_debt_schedule_flows(flow_date, status);

-- One loan's schedule.
CREATE INDEX idx_wf_debt_schedule_flows_asset
ON wf_debt_schedule_flows(asset_id, flow_date);

CREATE TABLE wf_rate_fixings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- e.g. EURIBOR_3M, ESTR, SOFR.
    index_code TEXT NOT NULL,
    fixing_date DATE NOT NULL,
    -- Annual rate as a decimal (0.0215 for 2.15%).
    rate NUMERIC NOT NULL,
    source TEXT NOT NULL DEFAULT 'MANUAL',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT wf_rate_fixings_unique UNIQUE (index_code, fixing_date)
);

ALTER TABLE wf_debt_schedule_flows ENABLE ROW LEVEL SECURITY;
ALTER TABLE wf_rate_fixings ENABLE ROW LEVEL SECURITY;
