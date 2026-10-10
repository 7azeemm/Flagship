\set ON_ERROR_STOP on
-- The launcher selects setup, grants or reset explicitly. Running this file
-- without a mode fails, so ordinary setup can never enter the reset branch.
\if :{?flagship_mode}
\else
    DO $$ BEGIN RAISE EXCEPTION 'Set flagship_mode to setup, grants or reset.'; END $$;
\endif
SELECT :'flagship_mode'='setup' AS flagship_setup,
       :'flagship_mode'='grants' AS flagship_grants,
       :'flagship_mode'='reset' AS flagship_reset
\gset

\if :flagship_reset
    -- The launcher confirms this action first. Active connections prevent it;
    -- existing roles and their passwords survive the database reset.
    DROP DATABASE IF EXISTS flagship;
    \quit
\elif :flagship_setup
    \getenv flagship_password FLAGSHIP_DB_PASSWORD
    \getenv directus_password DIRECTUS_DB_PASSWORD
    -- Rerunning setup creates missing roles without rotating passwords.
    SELECT format('CREATE ROLE flagship LOGIN PASSWORD %L', :'flagship_password')
    WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='flagship') \gexec
    SELECT format('CREATE ROLE flagship_directus LOGIN PASSWORD %L', :'directus_password')
    WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='flagship_directus') \gexec
    SELECT 'CREATE DATABASE flagship OWNER flagship'
    WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname='flagship') \gexec
    \connect flagship
    REVOKE CREATE ON SCHEMA public FROM PUBLIC;
    GRANT USAGE, CREATE ON SCHEMA public TO flagship, flagship_directus;
    GRANT CONNECT ON DATABASE flagship TO flagship, flagship_directus;
    SET ROLE flagship;
\elif :flagship_grants
    -- Connected by the launcher as the application table owner.
\else
    DO $$ BEGIN RAISE EXCEPTION 'Unknown flagship_mode. Expected setup, grants or reset.'; END $$;
\endif

-- Directus owns its system tables; grant access to application-owned tables.
GRANT USAGE ON SCHEMA public TO flagship_directus;
SELECT format('GRANT SELECT, INSERT, UPDATE, DELETE, REFERENCES ON TABLE %I.%I TO flagship_directus', schemaname, tablename)
FROM pg_tables WHERE schemaname='public' AND tableowner=current_user
\gexec
SELECT format('GRANT USAGE, SELECT, UPDATE ON SEQUENCE %I.%I TO flagship_directus', namespace.nspname, sequence.relname)
FROM pg_class sequence
JOIN pg_namespace namespace ON namespace.oid=sequence.relnamespace
WHERE namespace.nspname='public' AND sequence.relkind='S'
  AND sequence.relowner=(SELECT oid FROM pg_roles WHERE rolname=current_user)
\gexec
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE, REFERENCES ON TABLES TO flagship_directus;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE, SELECT, UPDATE ON SEQUENCES TO flagship_directus;
\if :flagship_setup
    RESET ROLE;
\endif
