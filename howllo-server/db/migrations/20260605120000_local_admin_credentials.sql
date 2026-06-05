-- Local single-admin login.
--
-- The admin panel no longer depends on RooIAM OIDC. Instead, on first run the
-- operator sets a password (gated by ADMIN_BOOTSTRAP_KEY). The password is
-- stored only as an Argon2id hash. A successful login mints a short-lived JWT
-- signed with ROOIAM_JWT_SECRET, which the existing AuthenticatedUser +
-- require_admin path validates exactly as before. No auth checks are weakened.
--
-- This table holds a single row. Its presence means "bootstrapped".

CREATE TABLE admin_credentials (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- enforce at most one row
    CONSTRAINT admin_credentials_singleton CHECK (id = TRUE)
);
