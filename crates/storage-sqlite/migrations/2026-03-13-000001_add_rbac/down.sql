-- RBAC Migration Rollback
-- Drops user_roles first (FK dependency), then users

DROP INDEX IF EXISTS idx_user_roles_role;
DROP INDEX IF EXISTS idx_user_roles_user_id;
DROP TABLE IF EXISTS user_roles;

DROP INDEX IF EXISTS idx_users_is_active;
DROP INDEX IF EXISTS idx_users_email;
DROP TABLE IF EXISTS users;
