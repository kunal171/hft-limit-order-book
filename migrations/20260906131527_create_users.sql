-- Add migration script here

CREATE TABLE users(
    id UUID PRIMARY KEY,

    display_name TEXT NOT NULL,

    -- Optional because some system/API users may not have email.
    email TEXT UNIQUE,

    -- Keep this as TEXT + CHECK for now because user statuses may evolve.
    status TEXT NOT NULL CHECK (status IN ('active', 'suspended', 'closed')),

    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
)