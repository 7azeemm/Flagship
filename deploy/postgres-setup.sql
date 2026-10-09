\set ON_ERROR_STOP on
\getenv flagship_password FLAGSHIP_DB_PASSWORD
\getenv directus_password DIRECTUS_DB_PASSWORD

-- Only create missing roles; rerunning setup never rotates existing passwords.
SELECT format('CREATE ROLE flagship LOGIN PASSWORD %L', :'flagship_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'flagship') \gexec
SELECT format('CREATE ROLE flagship_directus LOGIN PASSWORD %L', :'directus_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'flagship_directus') \gexec
SELECT 'CREATE DATABASE flagship OWNER flagship'
WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = 'flagship') \gexec

\connect flagship
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT USAGE, CREATE ON SCHEMA public TO flagship, flagship_directus;
GRANT CONNECT ON DATABASE flagship TO flagship, flagship_directus;

-- Apply grants as the application owner, including defaults for future tables.
SET ROLE flagship;
\ir directus-metadata.sql
RESET ROLE;
