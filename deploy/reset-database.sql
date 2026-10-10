\set ON_ERROR_STOP on
-- Used only after explicit confirmation by the loopback-only local launcher.
-- Refuse to drop while any other process is connected; roles/passwords survive.
DROP DATABASE IF EXISTS flagship;
-- The launcher's normal setup recreates the database and installs the schema.
