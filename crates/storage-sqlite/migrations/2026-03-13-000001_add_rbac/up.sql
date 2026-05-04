-- RBAC Migration: Users and Roles for DFC Fund Management
-- Adds multi-user support with role-based access control

-- ============================================================================
-- USERS TABLE
-- ============================================================================
-- Core user identity table. WF was single-user; DFC requires multi-user.
-- id uses TEXT (UUID string) for SQLite compatibility.

CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    email TEXT UNIQUE NOT NULL,
    full_name TEXT,
    avatar_url TEXT,
    preferred_currency TEXT DEFAULT 'EUR',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_login_at TEXT,

    CHECK (is_active IN (0, 1))
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_is_active ON users(is_active);

-- ============================================================================
-- USER_ROLES TABLE
-- ============================================================================
-- Links users to roles. Multiple roles per user supported.
-- Role hierarchy (implied, not enforced in DB):
--   superadmin > admin > portfolio_manager > trader > analyst > viewer

CREATE TABLE user_roles (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    role TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by TEXT,

    CONSTRAINT fk_user_roles_user FOREIGN KEY (user_id)
        REFERENCES users(id) ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT fk_user_roles_created_by FOREIGN KEY (created_by)
        REFERENCES users(id) ON DELETE SET NULL ON UPDATE CASCADE,

    CHECK (role IN ('superadmin', 'admin', 'portfolio_manager', 'trader', 'analyst', 'viewer')),

    UNIQUE(user_id, role)
);

CREATE INDEX idx_user_roles_user_id ON user_roles(user_id);
CREATE INDEX idx_user_roles_role ON user_roles(role);
