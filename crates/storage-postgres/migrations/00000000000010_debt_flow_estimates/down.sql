ALTER TABLE wf_debt_schedule_flows DROP CONSTRAINT wf_debt_schedule_flows_status_check;

UPDATE wf_debt_schedule_flows
SET status = 'RECEIVED', updated_at = now()
WHERE status IN ('ESTIMATED', 'CONFIRMED');

ALTER TABLE wf_debt_schedule_flows ADD CONSTRAINT wf_debt_schedule_flows_status_check
    CHECK (status IN ('EXPECTED', 'RECEIVED', 'PARTIAL', 'LATE', 'WAIVED'));
