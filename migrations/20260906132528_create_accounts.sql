
-- Accounts represent trading portfolios owned by users.
-- One user can have multiple accounts later.

CREATE TABLE accounts (
    id UUID PRIMARY KEY,

    -- Account Owner
    user_id UUID NOT NULL REFERENCES users(id),

    -- Example: main, test, strategy-1.
    name TEXT NOT NULL,

    -- active, suspended, closed.
    status TEXT NOT NULL CHECK (status IN ('active', 'suspended', 'closed')),

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- A user should not have two accounts with the same name.
    UNIQUE (user_id, name)
);