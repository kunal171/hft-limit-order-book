-- Sessions authenticate users after a successful login.
CREATE TABLE sessions (
    id UUID PRIMARY KEY,

    user_id UUID NOT NULL
        REFERENCES users(id)
        ON DELETE CASCADE,

    -- Store only a hash of the bearer token. The raw token is returned
    -- to the client once and must never be stored in the database.
    token_hash BYTEA NOT NULL UNIQUE,

    expires_at TIMESTAMPTZ NOT NULL,

    -- A non-NULL value means logout or administrative revocation.
    revoked_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CHECK (expires_at > created_at)
);

-- Supports revoking or listing every session belonging to one user.
CREATE INDEX sessions_user_id_idx ON sessions (user_id);

-- Supports deleting expired sessions without scanning the entire table.
CREATE INDEX sessions_active_expiry_idx
ON sessions (expires_at)
WHERE revoked_at IS NULL;