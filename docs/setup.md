# Development setup

## Prerequisites

- Rust stable with Rust 2024 edition support (Rust 1.85 or newer).
- Node.js 22.12 or newer and npm. Node 22 LTS is recommended because the setup launcher initializes Directus, which requires Node 22.
- PostgreSQL installed and running locally. The launcher expects a local `flagship` database and role.
- Chromium and its system libraries for browser-backed retailer adapters; the setup command attempts to install the browser.
- Git and the platform shell used by the launcher (PowerShell/cmd on Windows, POSIX shell on Linux).

## Configure

1. Copy `.env.example` to `.env` and replace the sample database and service values. Do not commit `.env`.
2. Set `DATABASE_URL` to a local PostgreSQL URL for database `flagship`, user `flagship`, and ensure its password matches `FLAGSHIP_DB_PASSWORD` (URL-encode special characters in the URL).
3. The backend validates `APP_PUBLIC_URL` and SMTP settings at startup. For local development, the launcher sets the local app origin and local-development mode. Use working SMTP credentials only when testing email delivery.
4. Google OAuth and Discord are optional. Google requires all three `GOOGLE_*` values; Discord requires a bot token, guild, and the relevant channel IDs.
5. The current `setup` launcher also initializes Directus and therefore requires `DIRECTUS_SECRET`, `DIRECTUS_DB_PASSWORD`, `DIRECTUS_ADMIN_EMAIL`, `DIRECTUS_ADMIN_PASSWORD`, and `DIRECTUS_ADMIN_TOKEN`, even if you do not plan to keep using the staff interface. Use strong local-only values. Directus retains its own license (MSCL); it is not under the project MIT license.

## Environment variables

All secrets belong in the ignored root `.env` or a secure deployment environment, never in source control.

| Variable | Purpose |
|---|---|
| `DATABASE_URL` | PostgreSQL URL; local launcher expects `flagship` role/database on localhost. |
| `FLAGSHIP_DB_PASSWORD` | Password for the application database role; must match the URL password. |
| `POSTGRES_ADMIN_USER`, `POSTGRES_ADMIN_PASSWORD` | Admin credentials used by local database setup; password can be entered interactively if omitted. |
| `APP_PUBLIC_URL` | Canonical HTTPS site origin in production; launcher overrides it with the local origin for dev. |
| `BIND_ADDRESS` | Backend listen address; defaults to `127.0.0.1:3000`. |
| `FLAGSHIP_WEB_DIR` | Built frontend directory served by the Rust backend; normally `website/dist`. |
| `RUST_LOG` | Rust tracing filter, for example `info` or a module-specific filter. |
| `ENABLE_SCANNING` | Set to `true` to schedule retailer scans; local launcher default is `false`. |
| `TRUSTED_PROXY_IPS` | Comma-separated trusted proxy IP addresses for forwarded client addresses. |
| `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM` | Required backend email delivery configuration for account and alert email. |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `GOOGLE_REDIRECT_URL` | Optional Google sign-in; configure all three together. |
| `DISCORD_TOKEN`, `DISCORD_GUILD_ID`, `DISCORD_ADDED_PRODUCT_CHANNEL_ID`, `DISCORD_EDITED_PRODUCT_CHANNEL_ID`, `DISCORD_REMOVED_PRODUCT_CHANNEL_ID`, `DISCORD_SCAN_CHANNEL_ID`, `DISCORD_ALERT_CHANNEL_ID` | Production Discord integration. Channel and guild values are numeric Discord IDs. |
| `DISCORD_DEV_TOKEN`, `DISCORD_DEV_GUILD_ID`, `DISCORD_DEV_ADDED_PRODUCT_CHANNEL_ID`, `DISCORD_DEV_EDITED_PRODUCT_CHANNEL_ID`, `DISCORD_DEV_REMOVED_PRODUCT_CHANNEL_ID`, `DISCORD_DEV_SCAN_CHANNEL_ID`, `DISCORD_DEV_ALERT_CHANNEL_ID` | Separate development bot configuration; do not reuse the production bot token. |
| `ENABLE_DISCORD`, `ENABLE_DISCORD_DEV` | Explicitly enable or disable the corresponding Discord integration. |
| `DIRECTUS_SECRET`, `DIRECTUS_DB_PASSWORD`, `DIRECTUS_ADMIN_EMAIL`, `DIRECTUS_ADMIN_PASSWORD`, `DIRECTUS_ADMIN_TOKEN` | Directus instance and setup credentials required by the current setup flow. |
| `DIRECTUS_PORT`, `DIRECTUS_PUBLIC_URL` | Optional Directus bind port and public URL; defaults to port 8055 and localhost. |
| `LOCAL_APP_PUBLIC_URL` | Optional local Vite origin override; must be loopback on port 5173. |
| `POSTGRES_BIN_DIR`, `POSTGRES_SERVICE` | Optional Windows PostgreSQL tool directory and service-name overrides. |
| `API_PROXY_TARGET`, `FLAGSHIP_APP_ORIGIN` | Optional Vite development API target and canonical origin overrides. |

The exact values and safe placeholders are listed in [`.env.example`](../.env.example). The launcher reads the root `.env`; Vite also reads its own environment for local proxy/origin overrides.

## Install and run

On Windows:

```bat
start.bat setup
start.bat dev
```

On Linux:

```sh
sh start.sh setup
sh start.sh dev
```

`setup` creates/configures the local database roles, applies migrations, installs website and Directus dependencies, and initializes Directus. Run `start.bat --help` or `sh start.sh --help` to see the launcher modes. `dev` starts the Rust API and Vite frontend. The default frontend address is `http://127.0.0.1:5173`; the API listens on `127.0.0.1:3000`.

The local launcher disables scraping by default. Set `ENABLE_SCANNING=true` only when you intentionally want outbound requests to configured retailer sites. Review retailer policies and adapt rate/concurrency behavior before use.

## Production mode

The repository contains deployment-specific scripts rather than a generic production installer. Production configuration requires an HTTPS `APP_PUBLIC_URL`, valid SMTP settings, PostgreSQL, a built frontend, and appropriate proxy trust configuration. Review `deploy/Caddyfile`, `deploy/*.service`, and `deploy/install-retailer-services.sh`; they include host-specific paths, account assumptions, and network services. Do not expose Directus or run the scraper in production without independently reviewing access controls and retailer terms.

## Troubleshooting

- Database connection failures: confirm PostgreSQL is running, the database/role exist, and the URL password matches `FLAGSHIP_DB_PASSWORD`.
- Frontend package failures: use Node 22.12 or newer and run setup again; lockfiles are committed.
- Email configuration errors: backend startup requires SMTP fields even if email features will not be used. Configure a test SMTP provider or adapt local-only validation in a fork.
- Empty catalog: a clean clone has no product database. Running the scanner is optional and does not guarantee successful imports.
- Broken retailer integrations: site markup/access behavior changes over time; the adapter code may need substantial updates and is not actively maintained.
