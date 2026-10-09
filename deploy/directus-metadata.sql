\set ON_ERROR_STOP on
-- Run as the application table owner. Directus owns its own system tables.
-- Grant full row access and schema inspection to every application table.
GRANT USAGE ON SCHEMA public TO flagship_directus;
SELECT format('GRANT SELECT, INSERT, UPDATE, DELETE, REFERENCES ON TABLE %I.%I TO flagship_directus', schemaname, tablename)
FROM pg_tables
WHERE schemaname = 'public' AND tableowner = current_user
\gexec

SELECT format('GRANT USAGE, SELECT, UPDATE ON SEQUENCE %I.%I TO flagship_directus', namespace.nspname, sequence.relname)
FROM pg_class sequence
JOIN pg_namespace namespace ON namespace.oid = sequence.relnamespace
WHERE namespace.nspname = 'public' AND sequence.relkind = 'S'
  AND sequence.relowner = (SELECT oid FROM pg_roles WHERE rolname = current_user)
\gexec

-- Future migrations should expose their tables and identity sequences as well.
ALTER DEFAULT PRIVILEGES IN SCHEMA public
  GRANT SELECT, INSERT, UPDATE, DELETE, REFERENCES ON TABLES TO flagship_directus;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
  GRANT USAGE, SELECT, UPDATE ON SEQUENCES TO flagship_directus;
