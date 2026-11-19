-- Flows the engine books from the schedule are estimates until the platform
-- statement confirms them: RECEIVED splits into ESTIMATED (generated activity,
-- counted in the valuation) and CONFIRMED (matched with a real activity).

ALTER TABLE wf_debt_schedule_flows DROP CONSTRAINT wf_debt_schedule_flows_status_check;

UPDATE wf_debt_schedule_flows
SET status = CASE WHEN notes = 'AUTO' THEN 'ESTIMATED' ELSE 'CONFIRMED' END,
    updated_at = now()
WHERE status = 'RECEIVED';

ALTER TABLE wf_debt_schedule_flows ADD CONSTRAINT wf_debt_schedule_flows_status_check
    CHECK (status IN ('EXPECTED', 'ESTIMATED', 'CONFIRMED', 'PARTIAL', 'LATE', 'WAIVED'));
