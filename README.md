# Mythos

**A platform in development for discovering tech products, understanding prices, and planning purchases in Tunisia.**

Mythos brings together product data from Tunisian retailers as the foundation for a broader shopping platform: product trackers and alerts, a marketplace, PC building tools, and more.

This repository contains the **Rust data collection and API backend** and a responsive
catalog website with light/dark themes, category navigation, product details, price-history
charts, and structured filters. Accounts and alerts are still on the roadmap.

To browse saved products without starting the scraper or Discord bot, first run
`npm ci` and `npm run build` inside `website/`, then run
`cargo run --locked -- --serve-only` from the root and open http://127.0.0.1:3000.

## What is here today

- Retailer adapters for stores including Mytek, Tunisianet, Spacenet, MegaPC, and others.
- HTTP and browser-based collection infrastructure using Reqwest and Playwright.
- Category-specific parsers and reference datasets for normalizing product information.
- Product records with retailer links, prices, availability, component identifiers, and change-history fields.
- An Axum API with search, price and stock filters, retailer selection, sorting, pagination, and experimental product grouping.
- A market overview and product price-history charts based on saved observations.
- JSON product exports, page and description caches, and scraping-cycle reports.

> [!IMPORTANT]
> Catalog-only mode loads saved products from `data/<Section>.json`. A fresh checkout without product data shows an empty catalog. Parser and grouping accuracy are still being refined; the website does not correct inconsistent product records or provide tracking and alert services.

## Platform roadmap

| Area | Direction |
| --- | --- |
| Product discovery | Richer comparisons and product insights beyond the current catalog |
| Graphs and analytics | Broader price and availability analysis beyond the current charts |
| Trackers | Watchlists, price-drop alerts, restock alerts, and advanced queries |
| PC builder | Tools for choosing components and planning a build |
| Marketplace | A marketplace alongside retailer product discovery |
| Product insights | Product reviews, scoring, and benchmark-informed comparisons |

These are planned capabilities, not a list of features already available in this repository. See [PROJECT_ROADMAP.md](PROJECT_ROADMAP.md) for the working development plan; priorities may change.

## How the backend is organized

| Location | Responsibility |
| --- | --- |
| [src/core/retailers](src/core/retailers) | Retailer-specific collection logic |
| [src/core/parsers](src/core/parsers) | Category parsing and normalization |
| [config/sections.json](config/sections.json) | Category rules, filters, and grouping fields |
| [config/datasets](config/datasets) | Reference data used during parsing |
| [src/core/scanner.rs](src/core/scanner.rs) | Collection, parsing, caching, and update cycles |
| [src/core/storage.rs](src/core/storage.rs) | In-memory product storage, updates, and JSON exports |
| [src/api](src/api) | HTTP routes, queries, filters, and responses |

The data flow is retailer adapters or cached pages → normalization and change detection → product storage → API responses. Products are exported to `data/<Section>.json`; cycle reports go to `reports/`.

## Local development

### Requirements

- A recent stable Rust toolchain supporting the Rust 2024 edition.
- Playwright's Chromium browser and its system dependencies, compatible with the `playwright-rs` version in [Cargo.toml](Cargo.toml), when running the scraper. Catalog-only mode does not initialize Chromium.
- Local development data if you want to exercise the current cached-page path.

```bash
git clone https://github.com/7azeemm/Mythos.git
cd Mythos
cargo build --locked
cargo run --locked
```

The Cargo package is still named `PriceTracker`; the project is now called **Mythos**. Run commands from the repository root so relative configuration and data paths resolve correctly.

The normal server binds to `0.0.0.0:3000`; catalog-only mode binds to `127.0.0.1:3000`. The website makes same-origin API requests, and permissive CORS is disabled. The catalog is read-only and has no account authentication. For deployment, use an HTTPS reverse proxy.

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

Parser coverage, grouping accuracy, data restoration, and durable history storage are still being refined. Charts need repeated observations to show a meaningful trend; alerts are not implemented yet.

When enabling live collection, review retailer access policies and use appropriate request limits. Treat collected prices and availability as observations, and verify them with the retailer before purchasing. Keep credentials and private operational data out of Git.

## Contributing

Useful contributions include reproducible parser failures, sanitized sample pages, normalization improvements, and focused API fixes. Discuss larger changes before implementing them so they fit the platform roadmap.
# Development price history

The homepage and `/statistics` use recorded price changes. To preview or add a repeatable month of sample changes to the local saved catalog, stop the server and run:

```powershell
python scripts/simulate_price_history.py
python scripts/simulate_price_history.py --apply
```

Restart the server with `--serve-only` to inspect the generated charts without scraping. The script backs up every changed file in `data/.price-history-backup`. Restore the original catalog with:

```powershell
python scripts/simulate_price_history.py --restore
```

The generated changes are for development only. A normal scan may overwrite the modified prices.
