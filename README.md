# Flagship

**A platform in development for discovering tech products, understanding prices, and planning purchases in Tunisia.**

Flagship brings together product data from Tunisian retailers as the foundation for a broader shopping platform: product trackers and alerts, a marketplace, PC building tools, and more.

This repository contains the **Rust data collection and API backend** and a responsive
catalog website with light/dark themes, category navigation, product details, price-history
charts, structured filters, accounts, wishlists, multi-match email alerts, and free recurring new-low monitoring. The catalog activity page provides date-based browsing of committed additions, edits, and removals.

Configure the root `.env` using [the database schema and setup notes](DATABASE_SCHEMA.md),
then open `start.bat` on Windows. Choose **First-time setup** after installing
PostgreSQL and Node.js 22 LTS, then **Website + backend** for live editing at
http://127.0.0.1:5173 (or your configured loopback origin). Start Directus separately when you need the staff editor,
or choose **Everything**. The launcher waits for the backend to finish compiling
and pass its readiness check before starting the live website and Directus.
Local launches override `TRUSTED_PROXY_IPS` to empty because Vite and readiness
checks connect without an HTTPS reverse proxy; your production `.env` is unchanged.
The local launcher disables scanning and Discord unless `ENABLE_SCANNING=true`. Direct `cargo run` enables collection by default.
Directus exposes every database table with full read/write access to the configured
administrator and Catalog editor role. Roadmap definitions live in
`config/roadmap.json`; authenticated and guest hype votes remain in PostgreSQL.
JSON edits take effect on the next roadmap request. Keep feature IDs stable to
retain their vote totals.
Google sign-in attempts use a bounded ten-minute memory store for a single backend
process; restarting cancels unfinished sign-ins. The disabled page-cache mode,
app-state table, and unused retailer metadata table have been removed. Alert
events form a processing queue for relevant live watches and are cleaned after
evaluation; completed delivery jobs are removed.

## What is here today

- Retailer adapters for stores including Mytek, Tunisianet, Spacenet, MegaPC, and others.
- HTTP and browser-based collection infrastructure using Reqwest and Playwright.
- Category-specific parsers and reference datasets for normalizing product information.
- Product records with retailer links, prices, availability, component identifiers, and change-history fields.
- An Axum API with search, price and stock filters, retailer selection, sorting, pagination, and experimental product grouping.
- A market overview and product price-history charts based on saved observations.
- PostgreSQL-backed products, variants, caches, and scraping state, with Directus for staff curation.

> [!IMPORTANT]
> Configure PostgreSQL and Directus using [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md) before starting the backend. A fresh database shows an empty catalog; existing SQLite data is not imported. Parser and grouping accuracy are still being refined. Alerts depend on committed scan observations; enable automatic collection with `ENABLE_SCANNING=true` during normal startup. `--serve-only` disables scanning unless `ENABLE_SCANNING=true` explicitly enables it.

## Platform roadmap

| Area | Direction |
| --- | --- |
| Product discovery | Richer comparisons and product insights beyond the current catalog |
| Graphs and analytics | Broader price and availability analysis beyond the current charts |
| Trackers | Watchlists, price-drop alerts, restock alerts, and advanced queries |
| PC builder | Tools for choosing components and planning a build |
| Marketplace | A marketplace alongside retailer product discovery |
| Product insights | Product reviews, scoring, and benchmark-informed comparisons |

These are planned capabilities, not a list of features already available in this repository. Feature definitions and their current states live in [config/roadmap.json](config/roadmap.json); priorities may change.

## How the backend is organized

| Location | Responsibility |
| --- | --- |
| [src/core/catalog](src/core/catalog) | Products, variants, sections, and deal rules |
| [src/core/normalization](src/core/normalization) | Reference datasets and category parsing |
| [src/scraping/retailers](src/scraping/retailers) | Retailer-specific collection logic |
| [config/sections.json](config/sections.json) | Category rules, filters, and grouping fields |
| [config/datasets](config/datasets) | Reference data used during parsing |
| [src/scraping/scanner.rs](src/scraping/scanner.rs) | Collection, parsing, caching, and update cycles |
| [src/scraping/tracking](src/scraping/tracking) | Scan reports, metrics, caches, and scraper errors |
| [src/core/storage.rs](src/core/storage.rs) | One in-memory product catalog, staged updates, and publication after persistence |
| [src/core/catalog/revision.rs](src/core/catalog/revision.rs) | Shared revision used to invalidate derived catalog caches immediately |
| [src/database.rs](src/database.rs) | PostgreSQL initialization, durable runtime storage, and synchronization of Directus catalog edits and staff reviews |
| [src/core/catalog/overrides.rs](src/core/catalog/overrides.rs) | Replay URL-based manual edit history |
| [src/api/catalog/analytics.rs](src/api/catalog/analytics.rs) | Shared cached catalog snapshots for analytics |
| [src/roadmap.rs](src/roadmap.rs) | Feature definitions and durable hype votes |
| [src/api/catalog](src/api/catalog) | Catalog HTTP handlers, queries, filters, and responses |
| [src/api/middleware.rs](src/api/middleware.rs), [src/api/response_cache.rs](src/api/response_cache.rs) | Request logging, proxy handling, and public catalog response caching |
| [src/accounts.rs](src/accounts.rs), [src/accounts](src/accounts) | Account handlers and workflows, OAuth, passwords, email delivery, wishlists, and rate limits |
| [src/alerts.rs](src/alerts.rs), [src/alerts](src/alerts) | One alert feature module with separate rules, storage and quotas, evaluation, delivery, and HTTP handlers |
| [src/web.rs](src/web.rs), [src/web/pages.rs](src/web/pages.rs) | Website routing, HTML page handlers, and response headers |
| [src/web/seo.rs](src/web/seo.rs) | Page titles, descriptions, canonical URLs, social previews, structured data, and crawler-visible HTML |
| [src/web/assets.rs](src/web/assets.rs), [src/web/sitemaps.rs](src/web/sitemaps.rs) | Static assets, robots.txt, and XML sitemaps |
| [src/discord/embeds](src/discord/embeds) | Product, scan, and scraper-error message rendering |

The data flow is retailer adapters or cached pages → normalization and change detection → PostgreSQL products → API responses.

Product edits are staged under an update mutex while requests read the single committed catalog. After persistence succeeds, publication refreshes variants, preserves views recorded during the save, and advances the catalog revision. The revision is an in-process cache invalidation token; cached results remain valid until a scan, catalog edit, or configuration reload advances it. HTML routes `/product/{id}` and `/variant/{id}` use `web/pages.rs`; JSON detail endpoints `/api/products/{id}` and `/api/variants/{id}` use `api/catalog/details.rs`.

Account and alert operations each have one feature location, including their HTTP handling and application workflows. Catalog storage and manual overrides remain with the catalog; the directory changes preserve the existing runtime behavior.

Disabled retailer adapters and their commented registry entries are retained for future repairs. Only entries enabled in `src/scraping/retailers.rs` participate in scans.

Discord and Directus product edits are recorded transactionally in `product_overrides`,
keyed by the original product URL with timestamped field deltas. The table has no
foreign key to the catalog, so deleting products and scanning from scratch preserves
the corrections. Scraping and reparsing do not automatically replay old edits.
After rebuilding a catalog, run `cargo run --locked -- --apply-overrides` or
`start.bat apply-overrides` to restore matching corrections and rebuild variants.
The command exits afterward; it does not start the API, Discord, or Chromium.
Replay preserves fresh product IDs, unchanged component fields, and edit history;
unmatched URLs are reported, and invalid edits roll back the whole replay.
In Directus, edit product columns directly, including title and components.
The Details JSON editor holds notes, approval, images, and history. Product Overrides
shows saved changes. See [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md) for every table,
normalization rules, and local Google sign-in configuration. Directus
edits refresh the running website within the normal five-second polling interval.

## Local development

### Requirements

- A recent stable Rust toolchain supporting the Rust 2024 edition.
- Playwright's Chromium browser and its system dependencies, compatible with the `playwright-rs` version in [Cargo.toml](Cargo.toml), when running the scraper. Catalog-only mode does not initialize Chromium.

```bash
git clone https://github.com/7azeemm/Flagship.git
cd Flagship
cargo build --locked
cargo run --locked
```

The Cargo package and project are named **Flagship**. Run commands from the repository root so relative configuration and data paths resolve correctly.

Set `DATABASE_URL` to your PostgreSQL connection URL. The schema is defined by the ordered files in `migrations/`. It runs automatically at startup, or independently with `cargo run --locked -- --migrate-only`. See [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md) for native setup, Directus configuration, and testing. SQLite files are no longer used.

The server binds to `127.0.0.1:3000`. The website makes same-origin API requests, and permissive CORS is disabled. For deployment, use an HTTPS reverse proxy. The backend checks below cover formatting, linting, API requests, and database regressions.

### Account configuration

Accounts use server-side sessions and Argon2id password hashes. Email/password signups become accounts only after email confirmation; verified Gmail/Workspace identities can create accounts immediately. HTTPS and SMTP configuration are required at API startup. Production cookies are Secure; the local launcher allows HTTP loopback cookies. Recovery tokens are never returned in API responses. See [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md) for account tables and local sign-in configuration.

| Variable | Purpose |
| --- | --- |
| `APP_PUBLIC_URL` | Public HTTPS origin used to build email links |
| `TRUSTED_PROXY_IPS` | Exact proxy IPs allowed to supply an overwritten `X-Real-IP` header |
| `BIND_ADDRESS` | Listener address; defaults to `127.0.0.1:3000` |
| `DATABASE_URL` | PostgreSQL connection URL; configure in the root `.env` |
| `ENABLE_SCANNING=true` | Enable automatic collection, including launcher modes dev, all, and serve; off by default |
| `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM` | Authenticated STARTTLS mail delivery; port defaults to 587 |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `GOOGLE_REDIRECT_URL` | Google OpenID Connect; callback is `/api/auth/google/callback` |

Register the exact `GOOGLE_REDIRECT_URL` in Google Cloud. Do not commit any of these credentials. In production, all required SMTP and Google values should be injected by the service manager. Use an `SMTP_FROM` address on a domain you control, publish the SMTP provider's SPF and DKIM records, and add a DMARC record. Align the From domain with the authenticated SPF or DKIM domain; SMTP acceptance alone does not ensure inbox placement.

Back up PostgreSQL, including Directus system tables:

```bash
pg_dump --format=custom --file=backups/flagship.dump "$DATABASE_URL"
```

Test restoration with `pg_restore` against a separate staging database, start Flagship against that database, and verify accounts, the catalog, and alerts. Preserve Directus uploads and infrastructure configuration separately.

### Try the API

```bash
curl http://localhost:3000/info
curl 'http://localhost:3000/GPU/products?page=1&sort=price_desc&send_section_info=true'
curl 'http://localhost:3000/Laptop/products?search=Lenovo&stock=InStock'
```

Use the section identifiers returned by `/info`; they are case-sensitive. Product pages contain up to 60 products, or 60 groups when grouping is enabled.

| Query | Purpose |
| --- | --- |
| `search` | Match product titles or descriptions |
| `min_price`, `max_price` | Restrict the price range |
| `site`, `stock` | Filter retailer names and stock states |
| `page` | Select a page, starting at 1 |
| `sort=price_desc` | Sort by descending price; default is ascending |
| `send_section_info=true` | Include available filter and category metadata |
| `grouping_mode=true` | Return experimental product groups |
| `filters` | JSON object mapping filter keys to lists of accepted identifiers |

## Development notes

Parser coverage, grouping accuracy, data restoration, and durable history storage are still being refined. Charts need repeated observations to show a meaningful trend. The [alert feature](src/alerts) groups matching and validation in `rules.rs`, persistence and quotas in `storage.rs`, event processing in `evaluation.rs`, and email scheduling and delivery in `delivery.rs`. HTTP handlers stay alongside them in `handlers.rs`; [src/alerts.rs](src/alerts.rs) exposes the module's shared operations.

When enabling live collection, review retailer access policies and use appropriate request limits. Treat collected prices and availability as observations, and verify them with the retailer before purchasing. Keep credentials and private operational data out of Git.

## Contributing

Useful contributions include reproducible parser failures, sanitized sample pages, normalization improvements, and focused API fixes. Discuss larger changes before implementing them so they fit the platform roadmap.

### Deal evidence and catalog caching

`sort=newest` orders catalog items by listing addition time, newest first. Grouped variants use the newest offer matching the current filters; equal timestamps use stable item IDs.

Flagship deal rankings exclude retailer-advertised original prices. They prioritize offers with both a new recorded low and a competitor gap, then new lows, competitor gaps, and recent drops. Ties use evidence percentage, then stable identifiers (catalog non-deals fall back to price).

- New 30-day low: a strict new low across a complete 30-day recorded price history, reached within the last seven days.
- Recent price drop: at least 14 days of valid recorded history, a drop within seven days, and at least 5% below the time-weighted median price over the available last 30 days. A short price spike followed by a return to the typical price does not qualify.
- Below next-best offer: at least 10% below the cheapest offer from another retailer. Only positive, in-stock, non-removed listings in the same required/default-field variant count. Each retailer contributes its cheapest offer; tied cheapest retailers receive no exclusive-gap badge. The UI exposes the retailer count and matched fields.
- Grouped listings use configuration price history; standalone listings use their own history. Insufficient history does not prevent a current competitor comparison.

The public catalog router caches successful GET responses until the catalog revision changes, with 64 entries of at most 512 KiB each. Keys include the path and query; root and `/api` aliases share entries, and homepage/info ignore unused query parameters. Concurrent requests for a cached URL share one calculation. Catalog publication, section configuration reloads and scan completion invalidate the cache through a revision counter. Product and variant detail responses use no-store and count views on each detail request. Large responses stream through without being fully buffered or stored. Errors, cookie-setting responses, header-varying responses and responses marked private/no-store/no-cache are not cached. The diagnostic `X-Flagship-Cache` header reports HIT, MISS or BYPASS.

Homepage and statistics share a lightweight catalog snapshot until the same revision changes. It retains offer fields and price/stock history; descriptions, images arrays and unrelated history edits are omitted. Their main calculations run on the blocking pool after releasing the live catalog lock. Statistics caches category trends for each of the six supported ranges and global retailer comparisons independently of filters, search and pagination. Shared deal calculations also use a revision-aware single-flight cache. These caches have no time-based expiry: time-dependent results and cached view counts refresh with scans or other revision changes. Account, authentication, wishlist, alert and vote responses bypass the shared response cache. Alert-option metadata is cached by catalog revision, with authentication checked on every request.

Activity counts, cursor lookup and rows use one read transaction. Cursors must belong to the selected date and filters. The initial schema includes ordered activity indexes and an index for active-alert quotas. Roadmap vote totals aggregate each vote table before combining its results; visitor-specific vote state remains uncached.

### Backend checks

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Database tests read `DATABASE_URL` from the environment or root `.env`, create isolated temporary schemas, and drop them afterward. The configured role needs permission to create schemas. Tests cover catalog persistence, alert queues and quotas, cache expiry, concurrent session revocation, and single-use recovery tokens. HTTP tests start an ephemeral local router to verify query validation, JSON error responses, unsupported methods, body limits, and missing routes.

API errors have a consistent `error` object with `code`, `message`, and HTTP `status`, including errors raised by query and JSON extractors. Account routes keep their no-store policy; successful public catalog responses use the bounded revision-aware cache. Live retailer sites, Google sign-in, SMTP and Discord require separate integration checks.
