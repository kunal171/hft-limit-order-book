
-- Password credentials are separate because system users may not log in.
CREATE TABLE user_credentials (
    -- PRIMARY KEY creates a one-to-one user-to-credential relationship.
    user_id UUID PRIMARY KEY
        REFERENCES users(id)
        ON DELETE CASCADE,

    -- Store an Argon2 password hash, never the original password.
    password_hash TEXT NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);