use super::errors::review_counts;
use super::{BLUE, GREEN, PURPLE, nonempty, scan_error_report, truncate};
use crate::discord::events::ScanSummary;
use crate::scraping::tracking::scan_cache::{ScanRecord, ScanTrigger};
use serenity::builder::{CreateActionRow, CreateButton, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter};
use serenity::model::application::ButtonStyle;
use std::collections::{BTreeMap, BTreeSet};

pub fn scan_started(started_at: chrono::DateTime<chrono::Utc>, trigger: &ScanTrigger, sections: &[String], retailers: &[String]) -> CreateEmbed {
    CreateEmbed::new()
        .author(CreateEmbedAuthor::new("PRICE TRACKER · SCAN REPORT"))
        .title("↻ Scan started")
        .color(BLUE)
        .description(format!("Started <t:{}:R>", started_at.timestamp()))
        .field("Triggered by:", truncate(&trigger.label(), 200), false)
        .field("Retailers:", truncate(&nonempty(retailers.join(", "), "All retailers"), 800), false)
        .field("Sections:", truncate(&nonempty(sections.join(", "), "All sections"), 800), false)
        .timestamp(serenity::model::Timestamp::from_unix_timestamp(started_at.timestamp()).unwrap_or_default())
}

fn overview(summary: &ScanSummary) -> CreateEmbed {
    let duration_seconds = summary.duration_ms as f64 / 1000.0;
    let successful_pages = summary.pages.saturating_sub(summary.failed_pages);
    let failure_rate = percent(summary.failed_pages, summary.pages);
    let metrics = &summary.metrics;

    let mut overview = CreateEmbed::new()
        .author(CreateEmbedAuthor::new("SCAN OVERVIEW"))
        .title("✓ Scan completed")
        .description(format!(
            "**{} products** · **{duration_seconds:.2}s** elapsed\n{} retailers · {} sections · completed <t:{}:R>",
            summary.total_products,
            summary.sites_scanned,
            summary.sections_scanned,
            summary.completed_at.timestamp(),
        ))
        .color(BLUE)
        .field(
            "Changes",
            format!("**+{}** added\n**~{}** updated\n**−{}** removed", summary.added, summary.edited, summary.removed),
            true,
        )
        .field(
            "Page health",
            format!(
                "**{successful_pages}** without errors\n**{}** with errors · {failure_rate:.1}%\n**{}** request attempts",
                summary.failed_pages, summary.attempts
            ),
            true,
        )
        .field(
            "Scan processing",
            format!(
                "**{}** duplicates removed\n**{}** moved sections\n**{:.0} MiB** decoded HTML",
                metrics.duplicates_removed,
                metrics.moved_sections,
                metrics.pages.html_bytes as f64 / 1_048_576.0,
            ),
            true,
        )
        .field(
            "Product Info Cache",
            format!("**{}** cached\n**{}** fetch requests", metrics.pages.cache_hits, metrics.pages.product_page_requests),
            true,
        )
        .field(
            "Errors",
            format!(
                "**{}** errors in this scan\n**{}** unresolved errors tracked overall\n**{}** new · **{}** came back · **{}** cleared\n{}",
                summary.scrape_errors,
                summary.error_health.active,
                summary.error_health.newly_active,
                summary.error_health.reactivated,
                summary.error_health.resolved,
                review_counts(&summary.error_health)
            ),
            true,
        )
        .field(format!("Failed scopes · {}", summary.failed_scopes.len()), failed_scope_list(summary), false)
        .field("Changes by retailer", list_changes(&summary.change_sites), true)
        .field("Changes by section", list_changes(&summary.change_sections), true)
        .timestamp(serenity::model::Timestamp::from_unix_timestamp(summary.completed_at.timestamp()).unwrap_or_default());

    overview = overview.footer(CreateEmbedFooter::new(summary.metrics.next_scheduled_at.map_or_else(
        || "Next scheduled scan: schedule pending".into(),
        |next| format!("Next scheduled scan: {} UTC", next.format("%d %b %Y, %H:%M")),
    )));

    overview
}

fn catalog(summary: &ScanSummary) -> CreateEmbed {
    CreateEmbed::new()
        .author(CreateEmbedAuthor::new("CATALOG"))
        .title(format!("{} products in the catalog", summary.total_products))
        .color(GREEN)
        .field(
            "Top sections:",
            truncate(
                &nonempty(
                    summary
                        .top_sections
                        .iter()
                        .take(10)
                        .map(|(section, count)| format!("**{section}**  `{count}`"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    "No section data.",
                ),
                1000,
            ),
            true,
        )
        .field(
            "Top retailers:",
            truncate(
                &nonempty(
                    summary
                        .top_retailers
                        .iter()
                        .take(10)
                        .map(|(site, count)| format!("**{site}**  `{count}`"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    "No retailer data.",
                ),
                1000,
            ),
            true,
        )
        .field(
            "Availability:",
            format!(
                "🟢 **{}** in stock\n🔴 **{}** out of stock\n🟡 **{}** arriving\n⚪ **{}** on request",
                summary.catalog.in_stock, summary.catalog.out_of_stock, summary.catalog.on_arrive, summary.catalog.on_request
            ),
            true,
        )
}

fn performance(summary: &ScanSummary) -> CreateEmbed {
    let duration_seconds = summary.duration_ms as f64 / 1000.0;
    let changed = summary.added + summary.edited + summary.removed;
    let change_rate = percent(changed, summary.total_products.max(changed));
    let scanned_products: usize = summary.site_metrics.iter().map(|site| site.products).sum();
    let products_per_page = scanned_products as f64 / summary.pages.max(1) as f64;
    let products_per_second = scanned_products as f64 / duration_seconds.max(0.001);
    let pages_per_second = summary.pages as f64 / duration_seconds.max(0.001);

    let mut ranked_sites = summary.site_metrics.iter().collect::<Vec<_>>();
    ranked_sites.sort_by(|left, right| {
        site_pages_per_second(left)
            .total_cmp(&site_pages_per_second(right))
            .then_with(|| left.site.cmp(&right.site))
    });
    let mut slowest_sections = summary.section_metrics.iter().collect::<Vec<_>>();
    slowest_sections.sort_by(|left, right| right.duration_ms.cmp(&left.duration_ms).then_with(|| left.section.cmp(&right.section)));

    CreateEmbed::new()
        .author(CreateEmbedAuthor::new("PERFORMANCE"))
        .color(PURPLE)
        .field(
            "Scan throughput",
            format!(
                "**{pages_per_second:.2}** pages/s · **{products_per_second:.1}** products/s\n**{products_per_page:.1}** products/page · **{change_rate:.1}%** of catalog changed"
            ),
            false,
        )
        .field(
            "Slowest retailers · top 5",
            truncate(
                &nonempty(
                    ranked_sites
                        .iter()
                        .take(5)
                        .map(|site| {
                            format!(
                                "**{}** · **{} pages/s**\n{} pages\n{} products\n{} errors",
                                truncate(&site.site, 45),
                                if site.duration_ms == 0 {
                                    "—".into()
                                } else {
                                    format!("{:.2}", site_pages_per_second(site))
                                },
                                site.pages,
                                site.products,
                                site.errors
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                    "No retailer metrics.",
                ),
                1000,
            ),
            true,
        )
        .field(
            "Slowest sections · top 5",
            truncate(
                &nonempty(
                    slowest_sections
                        .iter()
                        .take(5)
                        .map(|section| {
                            format!(
                                "**{}** · **{:.2}s**\n{} products\n{} retailers\n{} errors",
                                truncate(&section.section, 45),
                                section.duration_ms as f64 / 1000.0,
                                section.products,
                                section.sites,
                                section.errors,
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                    "No section metrics.",
                ),
                1000,
            ),
            true,
        )
}

pub fn scan_overview(record: &ScanRecord) -> CreateEmbed {
    overview(&record.summary)
}

pub fn scan_part(record: &ScanRecord, part: &str) -> Option<CreateEmbed> {
    match part {
        "overview" => Some(scan_overview(record)),
        "catalog" => Some(catalog(&record.summary)),
        "performance" => Some(performance(&record.summary)),
        "errors" => Some(scan_error_report(&record.summary.error_health)),
        _ => None,
    }
}

pub fn scan_actions(scan_id: &str) -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        CreateButton::new(format!("scan:part:{scan_id}:overview")).label("Overview").style(ButtonStyle::Primary),
        CreateButton::new(format!("scan:part:{scan_id}:catalog")).label("Catalog").style(ButtonStyle::Secondary),
        CreateButton::new(format!("scan:part:{scan_id}:performance"))
            .label("Performance")
            .style(ButtonStyle::Secondary),
        CreateButton::new(format!("scan:part:{scan_id}:errors")).label("Errors").style(ButtonStyle::Secondary),
    ])]
}

fn percent(value: usize, total: usize) -> f64 {
    if total == 0 { 0.0 } else { value as f64 / total as f64 * 100.0 }
}

fn site_pages_per_second(site: &crate::discord::events::ScanSiteMetric) -> f64 {
    if site.duration_ms == 0 {
        f64::INFINITY
    } else {
        site.pages as f64 * 1000.0 / site.duration_ms as f64
    }
}

fn list_changes(changes: &[(String, usize, usize, usize)]) -> String {
    if changes.is_empty() {
        return "No catalog changes detected.".into();
    }
    truncate(
        &changes
            .iter()
            .take(12)
            .map(|(site, added, edited, removed)| format!("**{site}**  `+{added}` `~{edited}` `-{removed}`"))
            .collect::<Vec<_>>()
            .join("\n"),
        800,
    )
}

fn failed_scope_list(summary: &ScanSummary) -> String {
    let scopes = &summary.failed_scopes;
    if scopes.is_empty() {
        return "✓ No fetch or pagination failures.".into();
    }
    let mut retailers = BTreeMap::<&str, BTreeSet<&str>>::new();
    for scope in scopes {
        retailers.entry(&scope.site).or_default().insert(&scope.section);
    }
    let mut lines = Vec::new();
    let mut length = 0;
    for (site, sections) in &retailers {
        let mut labels = Vec::new();
        let mut section_length = 0;
        for section in sections {
            let label = truncate(section, 35);
            let added_length = label.chars().count() + usize::from(!labels.is_empty()) * 2;
            if section_length + added_length > 200 {
                break;
            }
            section_length += added_length;
            labels.push(label);
        }
        if sections.len() > labels.len() {
            labels.push(format!("+{} sections", sections.len() - labels.len()));
        }
        let line = format!("**{}**: {}", truncate(site, 45), labels.join(", "));
        let added_length = line.chars().count() + usize::from(!lines.is_empty());
        // Reserve space for the overflow indicator within Discord's field limit.
        if length + added_length > 900 {
            break;
        }
        length += added_length;
        lines.push(line);
    }
    if retailers.len() > lines.len() {
        lines.push(format!("… {} more retailers · open Errors for details.", retailers.len() - lines.len()));
    }
    lines.join("\n")
}
