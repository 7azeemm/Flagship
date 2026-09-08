# Variant System Roadmap

This document tracks the normalized variant system, mixed catalog API, charts,
engagement counters, and the later features that depend on stable product and
variant targets.

Status legend:

- `[x]` implemented
- `[-]` partially implemented
- `[ ]` pending

## Phase 1 — Core data model

- [X]  Add `Variant` and `VariantSnapshot`.
- [X]  Add `VariantStorage` backed by `state/variants.json`.
- [X]  Give every variant a stable public UUID.
- [X]  Keep a normalized internal key for finding an existing variant.
- [X]  Store variant fields as `HashMap<String, String>`.
- [X]  Build variant values from product `components`, not `filter_ids`.
- [X]  Accept and migrate the earlier array-based variant field representation.
- [X]  Generate the variant display name by joining values with `|`.
- [X]  Make variants own their `product_ids`.
- [X]  Keep variant IDs out of the persisted `Product` model.
- [X]  Build runtime indexes for variant keys and product memberships.
- [X]  Store current price, availability, retailer counts, views, and lifecycle timestamps.
- [X]  Store chart snapshots inside each variant.

## Phase 2 — Variant generation and synchronization

- [X]  Generate every configured required/optional field combination.
- [X]  Reuse an existing variant when its normalized key matches.
- [X]  Create a new UUID only when no matching variant exists.
- [X]  Rebuild memberships from current products on startup.
- [X]  Synchronize variants after committed catalog scans.
- [X]  Synchronize variants after manual product replacement, reparsing, and removal.
- [X]  Preserve inactive variants, IDs, history, and views.
- [X]  Select the cheapest in-stock member as the current lowest-price product.
- [X]  Fall back to another member for the representative image when nothing is in stock.
- [ ]  Record history only for sections and retailer scopes successfully scanned.

## Phase 3 — Mixed catalog API

- [X]  Replace separate product/group arrays with an ordered tagged `items` list.
- [X]  Add `ProductSummary` and `VariantSummary` API models.
- [X]  Apply search and filters before grouping.
- [X]  Return products missing selected grouping fields as standalone products.
- [X]  Return a variant with only one stored product as a product card.
- [X]  Combine variants and standalone products before sorting and pagination.
- [X]  Return both `total_items` and underlying `total_products`.
- [X]  Keep global search product-only.
- [X]  Avoid cloning full variants once per member during catalog construction.

## Phase 4 — Detail endpoints and views

- [X]  Add `GET /api/products/{id}`.
- [X]  Add `GET /api/variants/{id}`.
- [X]  Increment product views only on a single-product detail request.
- [X]  Increment variant views only on a single-variant detail request.
- [X]  Resolve a product chart through its most specific available variant.
- [X]  Return no product chart when a required grouping value is missing.
- [X]  Return sorted current offers with variant details.
- [X]  Exclude internal variant keys, raw membership IDs, and raw history from variant details.
- [X]  Mark detail responses `Cache-Control: no-store` so view requests reach the server.
- [X]  Preserve product views when scanner results replace product records.

- [-] Return the full core `Product` from product/offer details.

- [ ]  Add a dedicated public `ProductDetails` DTO that exposes only intentional fields.
- [ ]  Decide whether repeated opens in one browser session should count once or every time.
- [ ]  Add optional bot and crawler filtering.

## Phase 5 — Chart API

- [X]  Keep charts bound to variants.
- [X]  Let product detail responses reuse their matching variant chart.
- [X]  Support `range=30d|90d|180d|1y|all`.
- [X]  Support `interval=auto|scan|day|week`.
- [X]  Default to a 90-day range and automatic interval.
- [X]  Return lowest available price, offer count, and in-stock count points.
- [X]  Return current price, period low, all-time low, and period average.
- [X]  Return UTC timestamps and `TND` currency metadata.
- [X]  Preserve unavailable prices as `null`.

- [-] Aggregate daily and weekly responses by keeping the last snapshot per bucket.

- [ ]  Confirm whether daily price should mean closing price, minimum, or average.
- [ ]  Limit the maximum number of returned chart points.
- [ ]  Add custom `from` and `to` ranges if the preset ranges become insufficient.
- [ ]  Add chart tests for gaps, intervals, boundaries, and empty histories.

## Phase 6 — Chart frontend

- [X]  Fetch product and variant details lazily when a card is opened.
- [X]  Parse optional variant chart data at the frontend boundary.

- [-] Render an initial lowest-price line.
- [-] Show current availability counts.

- [ ]  Add accessible chart axes, dates, labels, and tooltips.
- [ ]  Render availability history as a real chart rather than only latest-state text.
- [ ]  Add 30-day, 90-day, 180-day, one-year, and all-time controls.
- [ ]  Add scan/day/week interval controls only if users need manual control.
- [ ]  Show current, period-low, average, and all-time-low summary cards.
- [ ]  Preserve the variant name as the detail-page/dialog title.
- [ ]  Add visible loading and error states for detail requests.
- [ ]  Cancel obsolete detail requests when a dialog closes or another item opens.

## Phase 7 — Persistence and performance

- [X]  Enable Brotli and gzip response compression.
- [X]  Remove synchronous view persistence from detail response latency.

- [-] Batch product view writes with a 10-second write-behind window; variant views wait for the next normal variant save.

- [X]  Serialize product and variant saves without holding their live storage locks.
- [X]  Prevent overlapping saves from writing snapshots out of order.
- [X]  Save the large variant store using compact JSON.
- [X]  Inspect the current store: roughly 9,400 variants and 19.6 MB before compaction.
- [ ]  Benchmark list, product-detail, and variant-detail latency after restarting the server.
- [ ]  Verify `Content-Encoding: br` and gzip fallback against running API responses.
- [ ]  Flush pending view writes during graceful shutdown.
- [ ]  Use atomic temporary-file replacement to prevent partial/corrupt JSON after a crash.
- [ ]  Partition variants by section if the single file becomes costly.
- [ ]  Consider storing high-frequency view counters in a small sidecar or database.
- [ ]  Move variants/history to a database when JSON rewrite cost is no longer acceptable.

## Phase 8 — History retention

- [X]  Replace hourly snapshots with change-only variant events.
- [X]  Record inactive/removed state once when an active variant disappears.
- [X]  Use deterministic cheapest-product selection to prevent fake change events.
- [ ]  Define a safety retention limit before genuine change history grows significantly.
- [ ]  Keep recent change events at full resolution.
- [ ]  Downsample older snapshots into daily points.
- [ ]  Downsample oldest snapshots into weekly points.
- [X]  Deduplicate unchanged price, cheapest-product, availability, offer, and retailer state.
- [ ]  Preserve all-time-low facts when old raw points are removed.
- [ ]  Add a migration path for existing variant history.

## Phase 9 — Engagement and statistics

- [X]  Add lifetime views to products and variants.
- [ ]  Track retailer-button clicks.
- [ ]  Record which product/retailer was selected from a variant.
- [ ]  Add daily product and variant analytics.
- [ ]  Add a separate statistics endpoint.
- [ ]  Calculate trending products and variants.
- [ ]  Calculate click-through rates without storing raw IP addresses.
- [ ]  Add basic abuse and duplicate-event protection.

## Phase 10 — Alerts, bookmarks, and wishlists

- [ ]  Add a shared target type supporting product and variant IDs.
- [ ]  Create alert storage owned by users rather than embedding private alerts in variants.
- [ ]  Add price-drop alerts.
- [ ]  Add restock alerts.
- [ ]  Add product and variant bookmarks.
- [ ]  Add wishlists.
- [ ]  Preserve references to inactive variants.
- [ ]  Add optional cached alert/bookmark counts to variants if required for statistics.

## Phase 11 — Deployment and frontend serving

- [X]  Keep Rust static serving through `frontend.rs`.
- [X]  Serve the Vite production build from `website/dist`.
- [X]  Keep Vite development proxy support through `npm run dev`.
- [ ]  Rebuild `website/dist` before testing the Rust-served frontend.
- [ ]  Verify direct category routes and SPA refreshes in the production build.
- [ ]  Document production reverse-proxy compression and cache behavior.

## Phase 12 — Verification

- [X]  Rust compile check passes.
- [X]  TypeScript compile check passes.
- [X]  Run Rust unit tests.
- [X]  Run frontend unit tests.
- [ ]  Run the live catalog smoke test with populated data.
- [ ]  Add detail endpoint contract tests.
- [ ]  Add view batching/persistence tests.
- [ ]  Add variant storage migration and recovery tests.
- [ ]  Benchmark cold and warm detail requests.
