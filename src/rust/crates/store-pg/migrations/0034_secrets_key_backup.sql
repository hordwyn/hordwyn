-- OPS-4 (DEC-620): when an admin confirmed that this key version is backed up. NULL = not
-- yet; Admin > Overview shows the reminder while the active version has no confirmation.
-- Additive, one nullable column.
ALTER TABLE secrets_keys ADD COLUMN backup_confirmed_at timestamptz;
