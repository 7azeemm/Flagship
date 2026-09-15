## Todo list

- [ ]  Change histroy field
- [ ]  Remove ids from CPU, and GPU datasets
- [ ]  idk
- [ ]  New-category detector in retailers
- [ ]  User Accounts
- [ ]  Alerts System
- [ ]  Statistics Page
- [ ]  Collect Statistics from website (+ discord command)
- [ ]  Homepage
- [ ]  Wishlist
- [ ]  Legal Page
- [ ]  Payment
- [ ]  Review Products
- [ ]  Discord Server (+ finish bot)
- [ ]  Hosting Server
- [ ]  Tests for retailers

## Product design notes

### Alerts System

Alerts have two separate modes:

1. **Existing product alert** — the user follows one current product or offer. They can be notified about a price target/drop, restock, or major availability change.
2. **Variant alert** — the user describes a product that may not exist in the catalog yet. They choose a section first, then build criteria from that section's supported fields and values (for example CPU, GPU, RAM capacity, memory type, storage, or display). When a newly scanned or updated product matches all selected criteria, Mythos sends the alert.

Variant-alert fields and their allowed values must come from the existing section configuration and datasets, not from a separate hard-coded form. A rule can therefore be valid even when no matching variant exists yet. The system should show the user whether there are currently matching offers, but it must keep watching when the result is empty.

Rules need clear matching semantics before implementation: AND between selected fields; OR between multiple values chosen for one field; a stable definition of price and stock status; a record of the last matched product/offer and last delivery; and deduplication so the same scan does not repeatedly notify the user.

### Statistics Page

The homepage stays focused on discovery: a small market summary, notable deals, new products, and a limited number of hand-picked highlights. Detailed analysis belongs on the Statistics page.

The Statistics page has two layers:

1. **Curated market charts** — charts chosen by Mythos because they explain an important market change, such as RAM price indices overall and split by DDR4/DDR5. Each chart is backed by one or more existing variants and has its own default scope, title, explanation, time range, and data-quality/sample-size note.
2. **Variant explorer** — a user selects a section and then filters it using the same section-specific fields used by variant alerts. It returns the price/stock history for the matching existing variants, so users can explore any supported combination without Mythos manually creating a chart for every case.

Curated charts should be configuration-driven rather than hard-coded. A chart definition needs a section, default variant filters, metric (price index, median/lowest price, availability, etc.), time range, display type, and explanatory text. It may expose a small approved set of controls, such as `memory_type`, allowing one chart to switch between DDR4 and DDR5 without changing its purpose.

The explorer can provide global controls (section, filters, time range, retailer selection, and metric) that apply to every explorer chart. Curated charts should keep their own scope by default; global controls should not silently alter them unless the user explicitly switches to a comparison/exploration mode.

Every statistic needs to show its definition and coverage: number of matching variants/offers, selected retailers, time range, last scan time, and how missing/out-of-stock products are handled. This is especially important for price indices, where a small or changing sample can otherwise look like a market trend.
