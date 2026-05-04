-- No-op down migration. We can't safely "un-fix" the FK references
-- without recreating the (dropped) `assets_old_pre_ytm` table, and
-- there is no operational reason to revert this repair.
SELECT 1;
