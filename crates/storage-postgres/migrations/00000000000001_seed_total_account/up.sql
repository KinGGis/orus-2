INSERT INTO wf_accounts (
    id,
    name,
    account_type,
    currency,
    is_default,
    is_active,
    is_archived,
    tracking_mode,
    created_at,
    updated_at
)
VALUES (
    '81edc1c4-ee7e-451c-be90-413cc912c3a4',
    'Total Portfolio',
    'AGGREGATE',
    'EUR',
    false,
    false,
    true,
    'NOT_SET',
    now(),
    now()
)
ON CONFLICT (id) DO UPDATE
SET
    name = EXCLUDED.name,
    account_type = EXCLUDED.account_type,
    is_default = EXCLUDED.is_default,
    is_active = EXCLUDED.is_active,
    is_archived = EXCLUDED.is_archived,
    tracking_mode = EXCLUDED.tracking_mode,
    updated_at = now();