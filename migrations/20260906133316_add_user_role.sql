-- Role controls what the user is allowed to do.
-- Admin can create markets. Trader can place orders later.
ALTER TABLE users
ADD COLUMN role TEXT NOT NULL DEFAULT 'trader'
CHECK (role IN ('admin', 'trader', 'system'));