use crate::core::product::{Product, ProductChange, ProductStatus};
use crate::core::tracking::error_tracker::{
    ErrorCycleRecord, ErrorCycleStatus, ErrorCycleSummary, ErrorRecord,
};
use crate::core::tracking::scan_cache::{ScanRecord, ScanTrigger};
use crate::discord::events::{AlertEvent, ProductChangeKind, ScanSummary};
use crate::utils::regex_cache::RegexCache;
use serde_json::Value;
use serenity::builder::{
    CreateActionRow, CreateButton, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter,
};
use serenity::model::application::ButtonStyle;
use std::collections::{BTreeMap, BTreeSet, HashMap};

const BLUE: u32 = 0x3B82F6;
const GREEN: u32 = 0x10B981;
const YELLOW: u32 = 0xF59E0B;
const RED: u32 = 0xEF4444;
const PURPLE: u32 = 0x8B5CF6;
const DARK: u32 = 0x334155;

pub fn product(product: &Product, kind: ProductChangeKind, changes: &[ProductChange]) -> CreateEmbed {
    let (label, color) = match kind {
        ProductChangeKind::New => ("✦ New product", GREEN),
        ProductChangeKind::Edited => ("↻ Product updated", BLUE),
        ProductChangeKind::Removed => ("− Product removed", RED),
        ProductChangeKind::Viewed => ("Product", DARK),
    };
    let discount = product
        .original_price
        .filter(|original| *original > 0 && product.price >= 0 && *original > product.price)
        .map(|original| {
            let saved = i64::from(original) - i64::from(product.price);
            let percent = (saved as f64 / original as f64 * 100.0).round() as i32;
            format!(
                "**{} TND**\n~~{} TND~~ · **{}% off**\nSave {} TND",
                product.price, original, percent, saved
            )
        })
        .unwrap_or_else(|| format!("**{} TND**", product.price));

    let event_time = if matches!(kind, ProductChangeKind::Removed) {
        product.removed_at.or(product.updated_at).unwrap_or(product.added_at)
    } else {
        product.updated_at.unwrap_or(product.added_at)
    };

    let mut embed = CreateEmbed::new()
        .author(CreateEmbedAuthor::new(truncate(
            &format!("{label} · {}", product.retailer),
            256,
        )))
        .title(truncate(
            &nonempty(product.title.trim().into(), "Untitled product"),
            256,
        ))
        .color(color)
        .field("Price", discount, true)
        .field("Availability",
            if matches!(kind, ProductChangeKind::Removed) {
                "⚪ Removed".into()
            } else {
                format!(
                    "{} {}",
                    match product.status {
                        ProductStatus::InStock => "🟢",
                        ProductStatus::OutOfStock => "🔴",
                        _ => "🟡",
                    },
                    product.status.readable_name()
                )
            },
            true,
        )
        .field("Category", product.section.to_string(), true)
        .footer(CreateEmbedFooter::new(format!(
            "ID: {} · Added {}",
            truncate(&product.id, 80),
            product.added_at.format("%d %b %Y"),
        )))
        .timestamp(
            serenity::model::Timestamp::from_unix_timestamp(event_time.timestamp())
                .unwrap_or_default(),
        );

    embed = embed.url(&product.url);
    embed = embed.thumbnail(&product.image);

    if !changes.is_empty() {
        embed = embed.field(
            format!("↻ What changed · {} fields", changes.len()),
            truncate(
                &changes
                    .iter()
                    .take(20)
                    .map(format_change)
                    .collect::<Vec<_>>()
                    .join("\n"),
                900,
            ),
            false,
        );
    }

    if !product.description.is_empty() {
        for part in split_text(&truncate(&product.description, 1800), 900, 2).into_iter() {
            embed = embed.field(
                "Description:",
                part,
                false,
            );
        }
    }
    let components = product_components(product);
    if !components.is_empty() {
        embed = embed.field("Components:", components, false);
    }
    if !product.notes.is_empty() {
        embed = embed.field(
            format!("Team notes · {}", product.notes.len()),
            truncate(
                &product.notes
                    .iter()
                    .rev()
                    .take(5)
                    .rev()
                    .map(|note| format!("- {note}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                800,
            ),
            false,
        );
    }
    embed
}

pub fn product_actions(product: &Product) -> Vec<CreateActionRow> {
    let product_id = &product.id;
    let mut buttons = vec![
        CreateButton::new(format!("product:edit_json:{product_id}"))
            .label("Edit JSON")
            .style(ButtonStyle::Primary),
        CreateButton::new(format!("product:reparse:{product_id}"))
            .label("Reparse")
            .style(ButtonStyle::Secondary),
        CreateButton::new(format!("product:note:{product_id}"))
            .label("Add note")
            .style(ButtonStyle::Secondary),
        CreateButton::new(format!("product:remove_confirm:{product_id}"))
            .label("Remove")
            .style(ButtonStyle::Danger),
    ];
    if !product.approved {
        buttons.insert(
            0,
            CreateButton::new(format!("review:approve:{product_id}"))
                .label("Approve")
                .style(ButtonStyle::Success),
        );
    }
    vec![CreateActionRow::Buttons(buttons)]
}

pub fn removed_product_actions(url: &str) -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        CreateButton::new_link(url).label("Open retailer page"),
    ])]
}

pub fn alert(alert: &AlertEvent) -> CreateEmbed {
    let is_error = alert.level.eq_ignore_ascii_case("error");
    let mut embed = CreateEmbed::new()
        .author(CreateEmbedAuthor::new(if is_error {
            "SYSTEM ERROR"
        } else {
            "SYSTEM WARNING"
        }))
        .title(truncate(&alert.message, 220))
        .description(format!("Source: `{}`", alert.target))
        .color(if is_error { RED } else { YELLOW })
        .field("SEVERITY", alert.level.to_uppercase(), true)
        .timestamp(serenity::model::Timestamp::now());
    for (name, value) in alert.fields.iter().take(8) {
        embed = embed.field(name.to_uppercase(), truncate(value, 500), true);
    }
    embed
}

pub fn scan_started(
    started_at: chrono::DateTime<chrono::Utc>,
    trigger: &ScanTrigger,
    sections: &[String],
    retailers: &[String],
) -> CreateEmbed {
    CreateEmbed::new()
        .author(CreateEmbedAuthor::new("PRICE TRACKER · SCAN REPORT"))
        .title("↻ Scan started")
        .color(BLUE)
        .description(format!(
            "Started <t:{}:R>",
            started_at.timestamp()
        ))
        .field("Triggered by:", truncate(&trigger.label(), 200), false)
        .field(
            "Retailers:",
            truncate(&nonempty(retailers.join(", "), "All retailers"), 800),
            false,
        )
        .field(
            "Sections:",
            truncate(&nonempty(sections.join(", "), "All sections"), 800),
            false,
        )
        .timestamp(
            serenity::model::Timestamp::from_unix_timestamp(started_at.timestamp())
                .unwrap_or_default(),
        )
}

pub fn scan(summary: &ScanSummary) -> Vec<CreateEmbed> {
    let duration_seconds = summary.duration_ms as f64 / 1000.0;
    let changed = summary.added + summary.edited + summary.removed;
    let successful_pages = summary.pages.saturating_sub(summary.failed_pages);
    let failure_rate = percent(summary.failed_pages, summary.pages);
    let change_rate = percent(changed, summary.total_products.max(changed));
    let scanned_products: usize = summary.site_metrics.iter().map(|site| site.products).sum();
    let products_per_page = scanned_products as f64 / summary.pages.max(1) as f64;
    let products_per_second = scanned_products as f64 / duration_seconds.max(0.001);
    let pages_per_second = summary.pages as f64 / duration_seconds.max(0.001);
    let metrics = &summary.metrics;

    let mut overview = CreateEmbed::new()
        .author(CreateEmbedAuthor::new("SCAN OVERVIEW"))
        .title("✓ Scan completed")
        .description(format!(
            "**{} products** · **{duration_seconds:.2}s** elapsed\n{} retailers · {} sections · completed <t:{}:R>",
            summary.total_products, summary.sites_scanned, summary.sections_scanned,
            summary.completed_at.timestamp(),
        ))
        .color(BLUE)
        .field("Changes", format!("**+{}** added\n**~{}** updated\n**−{}** removed", summary.added, summary.edited, summary.removed), true)
        .field("Page health", format!("**{successful_pages}** without errors\n**{}** with errors · {failure_rate:.1}%\n**{}** request attempts", summary.failed_pages, summary.attempts), true)
        .field("Scan processing", format!("**{}** duplicates removed\n**{}** moved sections\n**{:.0} MiB** decoded HTML", metrics.duplicates_removed, metrics.moved_sections, metrics.pages.html_bytes as f64 / 1_048_576.0, ), true)
        .field("Product Info Cache", format!("**{}** cached\n**{}** fetch requests", metrics.pages.cache_hits, metrics.pages.product_page_requests), true)
        .field("Errors", format!("**{}** errors in this scan\n**{}** unresolved errors tracked overall\n**{}** new · **{}** came back · **{}** cleared\n{}", summary.scrape_errors, summary.error_health.active, summary.error_health.newly_active, summary.error_health.reactivated, summary.error_health.resolved, review_counts(&summary.error_health)), true)
        .field(format!("Failed scopes · {}", summary.failed_scopes.len()), failed_scope_list(summary), false)
        .field("Changes by retailer", list_changes(&summary.change_sites), true)
        .field("Changes by section", list_changes(&summary.change_sections), true)
        .timestamp(serenity::model::Timestamp::from_unix_timestamp(summary.completed_at.timestamp()).unwrap_or_default());

    overview = overview.footer(CreateEmbedFooter::new(
        summary.metrics.next_scheduled_at
            .map_or_else(
                || "Next scheduled scan: schedule pending".into(),
                |next| {
                    format!(
                        "Next scheduled scan: {} UTC",
                        next.format("%d %b %Y, %H:%M")
                    )
                },
            ),
    ));

    let catalog = CreateEmbed::new()
        .author(CreateEmbedAuthor::new("CATALOG"))
        .title(format!("{} products in the catalog", summary.total_products))
        .color(GREEN)
        .field("Top sections:", truncate(&nonempty(summary.top_sections.iter().take(10).map(|(section, count)| format!("**{section}**  `{count}`")).collect::<Vec<_>>().join("\n"), "No section data."), 1000), true)
        .field("Top retailers:", truncate(&nonempty(summary.top_retailers.iter().take(10).map(|(site, count)| format!("**{site}**  `{count}`")).collect::<Vec<_>>().join("\n"), "No retailer data."), 1000), true)
        .field("Availability:", format!("🟢 **{}** in stock\n🔴 **{}** out of stock\n🟡 **{}** arriving\n⚪ **{}** on request", summary.catalog.in_stock, summary.catalog.out_of_stock, summary.catalog.on_arrive, summary.catalog.on_request), true);

    let mut ranked_sites = summary.site_metrics.iter().collect::<Vec<_>>();
    ranked_sites.sort_by(|left, right| {
        site_pages_per_second(left)
            .total_cmp(&site_pages_per_second(right))
            .then_with(|| left.site.cmp(&right.site))
    });
    let mut slowest_sections = summary.section_metrics.iter().collect::<Vec<_>>();
    slowest_sections.sort_by(|left, right| {
        right.duration_ms
            .cmp(&left.duration_ms)
            .then_with(|| left.section.cmp(&right.section))
    });

    let performance = CreateEmbed::new()
        .author(CreateEmbedAuthor::new("PERFORMANCE"))
        .color(PURPLE)
        .field("Scan throughput", format!("**{pages_per_second:.2}** pages/s · **{products_per_second:.1}** products/s\n**{products_per_page:.1}** products/page · **{change_rate:.1}%** of catalog changed"), false)
        .field(
            "Slowest retailers · top 5",
            truncate(&nonempty(
                ranked_sites
                    .iter()
                    .take(5)
                    .map(|site| {
                        format!(
                            "**{}** · **{} pages/s**\n{} pages\n{} products\n{} errors",
                            truncate(&site.site, 45),
                            if site.duration_ms == 0 { "—".into() } else {
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
            ), 1000),
            true,
        )
        .field(
            "Slowest sections · top 5",
            truncate(&nonempty(
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
            ), 1000),
            true,
        );

    vec![overview, catalog, performance]
}

pub fn scan_overview(record: &ScanRecord) -> CreateEmbed {
    scan(&record.summary).remove(0)
}

pub fn scan_part(record: &ScanRecord, part: &str) -> Option<CreateEmbed> {
    match part {
        "overview" => Some(scan_overview(record)),
        "catalog" => scan(&record.summary).into_iter().nth(1),
        "performance" => scan(&record.summary).into_iter().nth(2),
        "errors" => Some(scan_error_report(&record.summary.error_health)),
        _ => None,
    }
}

pub fn scan_actions(scan_id: &str) -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        CreateButton::new(format!("scan:part:{scan_id}:overview"))
            .label("Overview")
            .style(ButtonStyle::Primary),
        CreateButton::new(format!("scan:part:{scan_id}:catalog"))
            .label("Catalog")
            .style(ButtonStyle::Secondary),
        CreateButton::new(format!("scan:part:{scan_id}:performance"))
            .label("Performance")
            .style(ButtonStyle::Secondary),
        CreateButton::new(format!("scan:part:{scan_id}:errors"))
            .label("Errors")
            .style(ButtonStyle::Secondary),
    ])]
}

pub fn scan_error_report(summary: &ErrorCycleSummary) -> CreateEmbed {
    let reviewed = summary
        .cycle_records
        .iter()
        .filter(|item| item.record.reviewed)
        .count();
    let unreviewed = summary.cycle_records.len().saturating_sub(reviewed);
    CreateEmbed::new()
        .author(CreateEmbedAuthor::new("ERRORS"))
        .title(format!("{} errors in this scan", summary.observed))
        .color(if summary.cycle_records.is_empty() || unreviewed == 0 {
            GREEN
        } else if summary.newly_active > 0 {
            RED
        } else {
            YELLOW
        })
        .field(
            "THIS SCAN",
            format!(
                "{} events recorded\n{} tracked issues seen or cleared",
                summary.observed,
                summary.cycle_records.len()
            ),
            false,
        )
        .field(
            "CHANGES",
            format!(
                "`+{}` new\n`↻{}` came back\n`{}` cleared",
                summary.newly_active, summary.reactivated, summary.resolved
            ),
            true,
        )
        .field(
            "REVIEW",
            review_counts(summary),
            true,
        )
        .field(
            "ALL SCANS",
            format!(
                "{} unresolved issues\n{} cleared issues retained",
                summary.active, summary.inactive
            ),
            true,
        )
}

pub fn error_cycle_open_action(scan_id: &str) -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        CreateButton::new(format!("errors:cycle-open:{scan_id}"))
            .label("View errors")
            .style(ButtonStyle::Primary),
    ])]
}

pub fn error_cycle_page(summary: &ErrorCycleSummary, requested_page: usize) -> CreateEmbed {
    let groups = group_cycle_errors(&summary.cycle_records);
    let page_count = groups.len().max(1);
    let page = requested_page.min(page_count - 1);
    let mut embed = CreateEmbed::new()
        .author(CreateEmbedAuthor::new("ERRORS FROM THIS SCAN"))
        .title(format!("Error group {}/{}", page + 1, page_count))
        .color(YELLOW);
    if groups.is_empty() {
        embed = embed.field("RESULT", "No matching errors.", false);
    } else if let Some(group) = groups.get(page) {
        embed = populate_cycle_group_embed(embed, group);
    }
    embed
}

pub fn error_cycle_page_count(summary: &ErrorCycleSummary) -> usize {
    group_cycle_errors(&summary.cycle_records).len()
}

pub fn error_cycle_page_actions(
    summary: &ErrorCycleSummary,
    scan_id: &str,
    page: usize,
    page_count: usize,
) -> Vec<CreateActionRow> {
    let all_reviewed = cycle_group_all_reviewed(summary, page);
    grouped_error_actions(
        format!("errors:cycle-page:{scan_id}:{}", page.saturating_sub(1)),
        format!("errors:cycle-page:{scan_id}:{}", page + 1),
        format!("errors:cycle-item:{scan_id}:{page}:0"),
        format!(
            "errors:cycle-review-group:{scan_id}:{page}:{}",
            !all_reviewed
        ),
        all_reviewed,
        page,
        page_count,
    )
}

pub fn error_cycle_individual(
    summary: &ErrorCycleSummary,
    requested_group: usize,
    requested_item: usize,
) -> CreateEmbed {
    let groups = group_cycle_errors(&summary.cycle_records);
    let group_page = requested_group.min(groups.len().saturating_sub(1));
    let Some(group) = groups.get(group_page) else {
        return empty_individual_error();
    };
    let members = sorted_cycle_members(group);
    let item_page = requested_item.min(members.len().saturating_sub(1));
    let Some(item) = members.get(item_page) else {
        return empty_individual_error();
    };
    individual_error_embed(
        &item.record,
        cycle_status_label(item.status),
        item_page,
        members.len(),
    )
}

pub fn error_cycle_individual_page_count(
    summary: &ErrorCycleSummary,
    requested_group: usize,
) -> usize {
    let groups = group_cycle_errors(&summary.cycle_records);
    let group_page = requested_group.min(groups.len().saturating_sub(1));
    groups
        .get(group_page)
        .map(|group| group.members.len())
        .unwrap_or(0)
}

pub fn error_cycle_individual_actions(
    summary: &ErrorCycleSummary,
    scan_id: &str,
    group_page: usize,
    item_page: usize,
    item_count: usize,
) -> Vec<CreateActionRow> {
    let reviewed = cycle_individual_reviewed(summary, group_page, item_page);
    let groups = group_cycle_errors(&summary.cycle_records);
    let members = groups
        .get(group_page)
        .map(sorted_cycle_members)
        .unwrap_or_default();
    let next_site = next_site_index(
        &members
            .iter()
            .map(|item| item.record.retailer.as_str())
            .collect::<Vec<_>>(),
        item_page,
    );
    individual_error_actions(
        format!(
            "errors:cycle-item:{scan_id}:{group_page}:{}",
            item_page.saturating_sub(1)
        ),
        format!("errors:cycle-item:{scan_id}:{group_page}:{}", item_page + 1),
        format!(
            "errors:cycle-site:{scan_id}:{group_page}:{}",
            next_site.unwrap_or(item_page)
        ),
        format!("errors:cycle-page:{scan_id}:{group_page}"),
        format!(
            "errors:cycle-review-item:{scan_id}:{group_page}:{item_page}:{}",
            !reviewed
        ),
        reviewed,
        item_page,
        item_count,
        next_site.is_some(),
    )
}

pub fn error_registry(
    records: &[ErrorRecord],
    status: &str,
    site: Option<&str>,
    requested_page: usize,
) -> CreateEmbed {
    let groups = group_registry_errors(records);
    let page_count = groups.len().max(1);
    let page = requested_page.min(page_count - 1);
    let reviewed = records.iter().filter(|record| record.reviewed).count();
    let unreviewed = records.len().saturating_sub(reviewed);
    let new = records
        .iter()
        .filter(|record| record.new_in_latest_scan)
        .count();
    let mut embed = CreateEmbed::new()
        .author(CreateEmbedAuthor::new("ERROR TRACKER"))
        .title(format!(
            "{} {status} errors · group {}/{}",
            records.len(),
            page + 1,
            page_count
        ))
        .description(format!(
            "Retailer: **{}** · new errors are shown first · grouped by message.",
            site.unwrap_or("all retailers"),
        ))
        .color(if records.is_empty() || unreviewed == 0 {
            GREEN
        } else if new > 0 {
            RED
        } else if status == "active" {
            YELLOW
        } else {
            DARK
        })
        .field(
            "OVERVIEW",
            format!("{new} new · {unreviewed} unreviewed · {reviewed} reviewed"),
            false,
        );
    if groups.is_empty() {
        embed = embed.field("RESULT", "No matching errors.", false);
    } else if let Some(group) = groups.get(page) {
        embed = populate_registry_group_embed(embed, group);
    }
    embed
}

pub fn error_registry_page_count(records: &[ErrorRecord]) -> usize {
    group_registry_errors(records).len()
}

pub fn error_registry_actions(
    records: &[ErrorRecord],
    status: &str,
    site: Option<&str>,
    page: usize,
    page_count: usize,
) -> Vec<CreateActionRow> {
    let site = site.unwrap_or("*");
    let all_reviewed = registry_group_all_reviewed(records, page);
    grouped_error_actions(
        format!(
            "errors:registry-page:{status}:{site}:{}",
            page.saturating_sub(1)
        ),
        format!("errors:registry-page:{status}:{site}:{}", page + 1),
        format!("errors:registry-item:{status}:{site}:{page}:0"),
        format!(
            "errors:registry-review-group:{status}:{site}:{page}:{}",
            !all_reviewed
        ),
        all_reviewed,
        page,
        page_count,
    )
}

pub fn error_registry_individual(
    records: &[ErrorRecord],
    requested_group: usize,
    requested_item: usize,
) -> CreateEmbed {
    let groups = group_registry_errors(records);
    let group_page = requested_group.min(groups.len().saturating_sub(1));
    let Some(group) = groups.get(group_page) else {
        return empty_individual_error();
    };
    let members = sorted_registry_members(group);
    let item_page = requested_item.min(members.len().saturating_sub(1));
    let Some(record) = members.get(item_page) else {
        return empty_individual_error();
    };
    individual_error_embed(
        record,
        if record.new_in_latest_scan {
            "NEW"
        } else if record.active {
            "ACTIVE"
        } else {
            "NO LONGER ACTIVE"
        },
        item_page,
        members.len(),
    )
}

pub fn error_registry_individual_page_count(
    records: &[ErrorRecord],
    requested_group: usize,
) -> usize {
    let groups = group_registry_errors(records);
    let group_page = requested_group.min(groups.len().saturating_sub(1));
    groups
        .get(group_page)
        .map(|group| group.members.len())
        .unwrap_or(0)
}

pub fn error_registry_individual_actions(
    records: &[ErrorRecord],
    status: &str,
    site: Option<&str>,
    group_page: usize,
    item_page: usize,
    item_count: usize,
) -> Vec<CreateActionRow> {
    let site = site.unwrap_or("*");
    let reviewed = registry_individual_reviewed(records, group_page, item_page);
    let groups = group_registry_errors(records);
    let members = groups
        .get(group_page)
        .map(sorted_registry_members)
        .unwrap_or_default();
    let next_site = next_site_index(
        &members
            .iter()
            .map(|record| record.retailer.as_str())
            .collect::<Vec<_>>(),
        item_page,
    );
    individual_error_actions(
        format!(
            "errors:registry-item:{status}:{site}:{group_page}:{}",
            item_page.saturating_sub(1)
        ),
        format!(
            "errors:registry-item:{status}:{site}:{group_page}:{}",
            item_page + 1
        ),
        format!(
            "errors:registry-site:{status}:{site}:{group_page}:{}",
            next_site.unwrap_or(item_page)
        ),
        format!("errors:registry-page:{status}:{site}:{group_page}"),
        format!(
            "errors:registry-review-item:{status}:{site}:{group_page}:{item_page}:{}",
            !reviewed
        ),
        reviewed,
        item_page,
        item_count,
        next_site.is_some(),
    )
}

pub fn error_cycle_group_keys(summary: &ErrorCycleSummary, page: usize) -> Vec<String> {
    group_cycle_errors(&summary.cycle_records)
        .get(page)
        .map(|group| {
            group
                .members
                .iter()
                .map(|item| item.record.key.clone())
                .collect()
        })
        .unwrap_or_default()
}

pub fn error_cycle_individual_key(
    summary: &ErrorCycleSummary,
    group_page: usize,
    item_page: usize,
) -> Option<String> {
    let groups = group_cycle_errors(&summary.cycle_records);
    let group = groups.get(group_page)?;
    sorted_cycle_members(group)
        .get(item_page)
        .map(|item| item.record.key.clone())
}

pub fn error_registry_group_keys(records: &[ErrorRecord], page: usize) -> Vec<String> {
    group_registry_errors(records)
        .get(page)
        .map(|group| {
            group
                .members
                .iter()
                .map(|record| record.key.clone())
                .collect()
        })
        .unwrap_or_default()
}

pub fn error_registry_individual_key(
    records: &[ErrorRecord],
    group_page: usize,
    item_page: usize,
) -> Option<String> {
    let groups = group_registry_errors(records);
    let group = groups.get(group_page)?;
    sorted_registry_members(group)
        .get(item_page)
        .map(|record| record.key.clone())
}

struct CycleErrorGroup<'a> {
    message: String,
    members: Vec<&'a ErrorCycleRecord>,
}

struct RegistryErrorGroup<'a> {
    message: String,
    members: Vec<&'a ErrorRecord>,
}

fn group_cycle_errors(records: &[ErrorCycleRecord]) -> Vec<CycleErrorGroup<'_>> {
    let mut grouped = HashMap::<String, Vec<&ErrorCycleRecord>>::new();
    for item in records {
        grouped
            .entry(normalized_error_message(&item.record.message))
            .or_default()
            .push(item);
    }
    let mut groups = grouped
        .into_iter()
        .map(|(message, members)| CycleErrorGroup { message, members })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        cycle_group_rank(left)
            .cmp(&cycle_group_rank(right))
            .then_with(|| right.members.len().cmp(&left.members.len()))
            .then_with(|| {
                right
                    .members
                    .iter()
                    .map(|item| item.record.last_seen)
                    .max()
                    .cmp(&left.members.iter().map(|item| item.record.last_seen).max())
            })
            .then_with(|| left.message.cmp(&right.message))
    });
    groups
}

fn cycle_group_rank(group: &CycleErrorGroup<'_>) -> u8 {
    group
        .members
        .iter()
        .map(|item| match item.status {
            ErrorCycleStatus::New => 0,
            ErrorCycleStatus::Active => 1,
            ErrorCycleStatus::Resolved => 2,
        })
        .min()
        .unwrap_or(3)
}

fn group_registry_errors(records: &[ErrorRecord]) -> Vec<RegistryErrorGroup<'_>> {
    let mut grouped = HashMap::<String, Vec<&ErrorRecord>>::new();
    for record in records {
        grouped
            .entry(normalized_error_message(&record.message))
            .or_default()
            .push(record);
    }
    let mut groups = grouped
        .into_iter()
        .map(|(message, members)| RegistryErrorGroup { message, members })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        registry_group_has_new(right)
            .cmp(&registry_group_has_new(left))
            .then_with(|| right.members.len().cmp(&left.members.len()))
            .then_with(|| {
                right
                    .members
                    .iter()
                    .map(|record| record.last_seen)
                    .max()
                    .cmp(&left.members.iter().map(|record| record.last_seen).max())
            })
            .then_with(|| left.message.cmp(&right.message))
    });
    groups
}

fn registry_group_has_new(group: &RegistryErrorGroup<'_>) -> bool {
    group.members.iter().any(|record| record.new_in_latest_scan)
}

fn registry_group_all_reviewed_value(group: &RegistryErrorGroup<'_>) -> bool {
    !group.members.is_empty() && group.members.iter().all(|record| record.reviewed)
}

fn sorted_cycle_members<'a>(group: &CycleErrorGroup<'a>) -> Vec<&'a ErrorCycleRecord> {
    let mut members = group.members.clone();
    members.sort_by(|left, right| {
        cycle_status_sort_rank(left.status)
            .cmp(&cycle_status_sort_rank(right.status))
            .then_with(|| left.record.retailer.cmp(&right.record.retailer))
            .then_with(|| left.record.section.cmp(&right.record.section))
            .then_with(|| left.record.page_url.cmp(&right.record.page_url))
            .then_with(|| left.record.target_url.cmp(&right.record.target_url))
            .then_with(|| left.record.key.cmp(&right.record.key))
    });
    members
}

fn sorted_registry_members<'a>(group: &RegistryErrorGroup<'a>) -> Vec<&'a ErrorRecord> {
    let mut members = group.members.clone();
    members.sort_by(|left, right| {
        right
            .new_in_latest_scan
            .cmp(&left.new_in_latest_scan)
            .then_with(|| left.reviewed.cmp(&right.reviewed))
            .then_with(|| left.retailer.cmp(&right.retailer))
            .then_with(|| left.section.cmp(&right.section))
            .then_with(|| left.page_url.cmp(&right.page_url))
            .then_with(|| left.target_url.cmp(&right.target_url))
            .then_with(|| left.key.cmp(&right.key))
    });
    members
}

fn cycle_status_sort_rank(status: ErrorCycleStatus) -> u8 {
    match status {
        ErrorCycleStatus::New => 0,
        ErrorCycleStatus::Active => 1,
        ErrorCycleStatus::Resolved => 2,
    }
}

fn cycle_group_all_reviewed(summary: &ErrorCycleSummary, page: usize) -> bool {
    group_cycle_errors(&summary.cycle_records)
        .get(page)
        .is_some_and(|group| {
            !group.members.is_empty() && group.members.iter().all(|item| item.record.reviewed)
        })
}

fn cycle_individual_reviewed(
    summary: &ErrorCycleSummary,
    group_page: usize,
    item_page: usize,
) -> bool {
    let groups = group_cycle_errors(&summary.cycle_records);
    groups
        .get(group_page)
        .and_then(|group| sorted_cycle_members(group).get(item_page).copied())
        .is_some_and(|item| item.record.reviewed)
}

fn registry_group_all_reviewed(records: &[ErrorRecord], page: usize) -> bool {
    group_registry_errors(records)
        .get(page)
        .is_some_and(registry_group_all_reviewed_value)
}

fn registry_individual_reviewed(
    records: &[ErrorRecord],
    group_page: usize,
    item_page: usize,
) -> bool {
    let groups = group_registry_errors(records);
    groups
        .get(group_page)
        .and_then(|group| sorted_registry_members(group).get(item_page).copied())
        .is_some_and(|record| record.reviewed)
}

fn cycle_group_lifecycle(group: &CycleErrorGroup<'_>) -> String {
    let new = group
        .members
        .iter()
        .filter(|item| item.status == ErrorCycleStatus::New)
        .count();
    let active = group
        .members
        .iter()
        .filter(|item| item.status == ErrorCycleStatus::Active)
        .count();
    let resolved = group
        .members
        .iter()
        .filter(|item| item.status == ErrorCycleStatus::Resolved)
        .count();
    [
        (new > 0).then(|| format!("NEW {new}")),
        (active > 0).then(|| format!("ACTIVE {active}")),
        (resolved > 0).then(|| format!("NO LONGER ACTIVE {resolved}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}

fn populate_cycle_group_embed(mut embed: CreateEmbed, group: &CycleErrorGroup<'_>) -> CreateEmbed {
    let records = group
        .members
        .iter()
        .map(|item| &item.record)
        .collect::<Vec<_>>();
    embed = embed
        .description(truncate(&group.message, 1000))
        .color(cycle_group_color(group))
        .field("STATUS", cycle_group_lifecycle(group), false)
        .field("REVIEW", cycle_group_review(group), false)
        .field("HISTORY", group_history(&records), true)
        .field("AFFECTED", affected_records(&records), true);
    embed
}

fn populate_registry_group_embed(
    mut embed: CreateEmbed,
    group: &RegistryErrorGroup<'_>,
) -> CreateEmbed {
    let active = group.members.iter().filter(|record| record.active).count();
    let inactive = group.members.len().saturating_sub(active);
    let new = group
        .members
        .iter()
        .filter(|record| record.new_in_latest_scan)
        .count();
    let reviewed = group
        .members
        .iter()
        .filter(|record| record.reviewed)
        .count();
    let unreviewed = group.members.len().saturating_sub(reviewed);
    embed = embed
        .description(truncate(&group.message, 1000))
        .color(registry_group_color(group))
        .field(
            "STATUS",
            [
                (new > 0).then(|| format!("NEW {new}")),
                (active > 0).then(|| format!("ACTIVE {active}")),
                (inactive > 0).then(|| format!("NO LONGER ACTIVE {inactive}")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · "),
            false,
        )
        .field(
            "REVIEW",
            format!("{unreviewed} unreviewed · {reviewed} reviewed"),
            false,
        )
        .field("HISTORY", group_history(&group.members), true)
        .field("AFFECTED", affected_records(&group.members), true);
    embed
}

fn cycle_group_review(group: &CycleErrorGroup<'_>) -> String {
    let reviewed = group
        .members
        .iter()
        .filter(|item| item.record.reviewed)
        .count();
    let unreviewed = group.members.len().saturating_sub(reviewed);
    format!("{unreviewed} unreviewed · {reviewed} reviewed")
}

fn cycle_group_color(group: &CycleErrorGroup<'_>) -> u32 {
    if cycle_group_all_reviewed_value(group) {
        GREEN
    } else if group.members.iter().any(|item| item.status == ErrorCycleStatus::New) {
        RED
    } else if group.members.iter().any(|item| item.status == ErrorCycleStatus::Active) {
        YELLOW
    } else {
        GREEN
    }
}

fn cycle_group_all_reviewed_value(group: &CycleErrorGroup<'_>) -> bool {
    !group.members.is_empty() && group.members.iter().all(|item| item.record.reviewed)
}

fn registry_group_color(group: &RegistryErrorGroup<'_>) -> u32 {
    if registry_group_all_reviewed_value(group) {
        GREEN
    } else if registry_group_has_new(group) {
        RED
    } else if group.members.iter().any(|record| record.active) {
        YELLOW
    } else {
        GREEN
    }
}

fn group_history(records: &[&ErrorRecord]) -> String {
    let first = records.iter().map(|record| record.first_seen).min();
    let last = records.iter().map(|record| record.last_seen).max();
    let occurrences = records
        .iter()
        .map(|record| record.occurrences)
        .sum::<usize>();
    format!(
        "First: {}\nLast: {}\nRecorded: **{} times**\nAffected pages: **{}**",
        first
            .map(|time| format!("<t:{}:R>", time.timestamp()))
            .unwrap_or_else(|| "unknown".into()),
        last.map(|time| format!("<t:{}:R>", time.timestamp()))
            .unwrap_or_else(|| "unknown".into()),
        occurrences,
        records.len(),
    )
}

fn affected_records(records: &[&ErrorRecord]) -> String {
    let mut sites = HashMap::<&str, usize>::new();
    let mut sections = HashMap::<&str, usize>::new();
    for record in records {
        *sites.entry(&record.retailer).or_default() += 1;
        *sections.entry(&record.section).or_default() += 1;
    }
    let mut sites = sites.into_iter().collect::<Vec<_>>();
    sites.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    let mut sections = sections.into_iter().collect::<Vec<_>>();
    sections.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    let sites = sites
        .iter()
        .take(6)
        .map(|(site, count)| format!("{site} ({count})"))
        .collect::<Vec<_>>()
        .join(", ");
    let sections = sections
        .iter()
        .take(6)
        .map(|(section, count)| format!("{section} ({count})"))
        .collect::<Vec<_>>()
        .join(", ");
    truncate(
        &format!(
            "Retailers: {}\nSections: {}",
            nonempty(sites, "None"),
            nonempty(sections, "None")
        ),
        1000,
    )
}

fn individual_error_embed(
    record: &ErrorRecord,
    status: &str,
    item_page: usize,
    item_count: usize,
) -> CreateEmbed {
    let mut urls = vec![format!("Page: {}", &record.page_url)];
    if let Some(target) = record.target_url.as_deref().filter(|target| *target != record.page_url) {
        urls.push(format!("Product: {target}"));
    }
    let resolved = record
        .resolved_at
        .map(|time| format!("\nResolved: <t:{}:R>", time.timestamp()))
        .unwrap_or_default();
    let review_state = match record.reviewed_at.filter(|_| record.reviewed) {
        Some(time) => format!("REVIEWED · <t:{}:R>", time.timestamp()),
        None => "UNREVIEWED".into(),
    };
    CreateEmbed::new()
        .author(CreateEmbedAuthor::new("INDIVIDUAL SCRAPER ERROR"))
        .title(truncate(
            &format!("{} · {} · {}", record.retailer, record.section, record.kind),
            256,
        ))
        .description(truncate(&record.message, 1800))
        .color(if record.reviewed || status == "NO LONGER ACTIVE" {
            GREEN
        } else if status == "NEW" {
            RED
        } else {
            YELLOW
        })
        .field("STATUS", status, true)
        .field("REVIEW", review_state, true)
        .field(
            "HISTORY",
            format!(
                "First: <t:{}:R>\nLast: <t:{}:R>\nRecorded: **{} times**\nTimes it came back: **{}**{}",
                record.first_seen.timestamp(),
                record.last_seen.timestamp(),
                record.occurrences,
                record.activations,
                resolved,
            ),
            true,
        )
        .field("LOCATION", urls.join("\n"), false)
        .footer(CreateEmbedFooter::new(format!(
            "Individual error {}/{}",
            item_page + 1,
            item_count.max(1)
        )))
}

fn empty_individual_error() -> CreateEmbed {
    CreateEmbed::new()
        .author(CreateEmbedAuthor::new("INDIVIDUAL SCRAPER ERROR"))
        .title("Error record unavailable")
        .description("This error group no longer contains a matching record.")
        .color(DARK)
}

fn cycle_status_label(status: ErrorCycleStatus) -> &'static str {
    match status {
        ErrorCycleStatus::New => "NEW",
        ErrorCycleStatus::Active => "ACTIVE",
        ErrorCycleStatus::Resolved => "NO LONGER ACTIVE",
    }
}

fn grouped_error_actions(
    previous_id: String,
    next_id: String,
    individual_id: String,
    toggle_id: String,
    all_reviewed: bool,
    page: usize,
    page_count: usize,
) -> Vec<CreateActionRow> {
    if page_count == 0 {
        return Vec::new();
    }
    let mut buttons = Vec::new();
    if page_count > 1 {
        buttons.push(
            CreateButton::new(previous_id)
                .label("Previous")
                .style(ButtonStyle::Secondary)
                .disabled(page == 0),
        );
        buttons.push(
            CreateButton::new(next_id)
                .label("Next")
                .style(ButtonStyle::Secondary)
                .disabled(page + 1 >= page_count),
        );
    }
    buttons.push(
        CreateButton::new(individual_id)
            .label("View individual errors")
            .style(ButtonStyle::Primary),
    );
    buttons.push(
        CreateButton::new(toggle_id)
            .label(if all_reviewed {
                "Mark group unreviewed"
            } else {
                "Mark group reviewed"
            })
            .style(if all_reviewed {
                ButtonStyle::Secondary
            } else {
                ButtonStyle::Success
            }),
    );
    vec![CreateActionRow::Buttons(buttons)]
}

fn individual_error_actions(
    previous_id: String,
    next_id: String,
    next_site_id: String,
    back_id: String,
    toggle_id: String,
    reviewed: bool,
    item_page: usize,
    item_count: usize,
    has_next_site: bool,
) -> Vec<CreateActionRow> {
    if item_count == 0 {
        return Vec::new();
    }
    let mut buttons = Vec::new();
    buttons.push(
        CreateButton::new(previous_id)
            .label("Previous error")
            .style(ButtonStyle::Secondary)
            .disabled(item_page == 0),
    );
    buttons.push(
        CreateButton::new(next_id)
            .label("Next error")
            .style(ButtonStyle::Secondary)
            .disabled(item_page + 1 >= item_count),
    );
    buttons.push(
        CreateButton::new(next_site_id)
            .label("Next site")
            .style(ButtonStyle::Secondary)
            .disabled(!has_next_site),
    );
    buttons.push(
        CreateButton::new(back_id)
            .label("Back to group")
            .style(ButtonStyle::Primary),
    );
    buttons.push(
        CreateButton::new(toggle_id)
            .label(if reviewed {
                "Mark unreviewed"
            } else {
                "Mark reviewed"
            })
            .style(if reviewed {
                ButtonStyle::Secondary
            } else {
                ButtonStyle::Success
            }),
    );
    vec![CreateActionRow::Buttons(buttons)]
}

fn next_site_index(sites: &[&str], current: usize) -> Option<usize> {
    let current_site = sites.get(current)?;
    sites
        .iter()
        .enumerate()
        .skip(current + 1)
        .find_map(|(index, site)| (!site.eq_ignore_ascii_case(current_site)).then_some(index))
}

fn normalized_error_message(message: &str) -> String {
    RegexCache::replace_all(r"https?://[^\s\)\]`]+", message, "<url>").to_string()
}

fn percent(value: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        value as f64 / total as f64 * 100.0
    }
}

fn site_pages_per_second(site: &crate::discord::events::ScanSiteMetric) -> f64 {
    if site.duration_ms == 0 {
        f64::INFINITY
    } else {
        site.pages as f64 * 1000.0 / site.duration_ms as f64
    }
}

fn product_components(product: &Product) -> String {
    let mut lines = Vec::new();
    let mut details = product.components.iter().collect::<Vec<_>>();
    details.sort_by(|left, right| left.0.cmp(right.0));
    for (key, value) in details.into_iter().take(16) {
        lines.push(format!("- **{}**:  `{}`", humanize(key), value));
    }
    truncate(&lines.join("\n"), 900)
}

fn split_text(value: &str, max_chars: usize, max_parts: usize) -> Vec<String> {
    if value.is_empty() || max_chars == 0 || max_parts == 0 {
        return Vec::new();
    }
    let mut parts = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if current.chars().count() == max_chars {
            parts.push(current);
            current = String::new();
            if parts.len() == max_parts {
                return parts;
            }
        }
        current.push(character);
    }
    if !current.is_empty() && parts.len() < max_parts {
        parts.push(current);
    }
    parts
}

fn list_changes(changes: &[(String, usize, usize, usize)]) -> String {
    if changes.is_empty() {
        return "No catalog changes detected.".into();
    }
    truncate(
        &changes
            .iter()
            .take(12)
            .map(|(site, added, edited, removed)| {
                format!("**{site}**  `+{added}` `~{edited}` `-{removed}`")
            })
            .collect::<Vec<_>>()
            .join("\n"),
        800,
    )
}

fn review_counts(summary: &ErrorCycleSummary) -> String {
    let reviewed = summary
        .cycle_records
        .iter()
        .filter(|item| item.record.reviewed)
        .count();
    let unreviewed = summary.cycle_records.len().saturating_sub(reviewed);
    format!("**{unreviewed}** unreviewed · **{reviewed}** reviewed")
}

fn failed_scope_list(summary: &ScanSummary) -> String {
    let scopes = &summary.failed_scopes;
    if scopes.is_empty() {
        return "✓ No fetch or pagination failures.".into();
    }
    let mut retailers = BTreeMap::<&str, BTreeSet<&str>>::new();
    for scope in scopes {
        retailers
            .entry(&scope.site)
            .or_default()
            .insert(&scope.section);
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
        lines.push(format!(
            "… {} more retailers · open Errors for details.",
            retailers.len() - lines.len()
        ));
    }
    lines.join("\n")
}

fn nonempty(value: String, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn humanize(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "cpu" => return "CPU".to_string(),
        "gpu" => return "GPU".to_string(),
        "id" => return "ID".to_string(),
        _ => {},
    }

    let mut chars = value.replace('_', " ").chars().collect::<Vec<_>>();
    if let Some(first) = chars.first_mut() {
        first.make_ascii_uppercase();
    }
    chars.into_iter().collect()
}

fn format_change(change: &ProductChange) -> String {
    let field_name = change.field.as_ref();
    let field = humanize(field_name);
    let (old_value, new_value) = (&change.old_value, &change.new_value);
    
    if matches!(field_name, "filters" | "components") {
        if let Some(formatted) = format_object_change(
            &field,
            Some(old_value),
            Some(new_value),
        ) {
            return formatted;
        }
    }

    let old = compact(Some(old_value));
    let new = compact(Some(new_value));

    if old.is_empty() {
        format!(
            "- **{field}:** `{}`",
            truncate(&new.replace('\n', " ").replace('`', "'"), 150)
        )
    } else {
        format!(
            "- **{field}:** `{}` -> `{}`",
            truncate(&old.replace('\n', " ").replace('`', "'"), 150),
            truncate(&new.replace('\n', " ").replace('`', "'"), 150)
        )
    }
}

fn format_object_change(
    field: &str,
    old: Option<&Value>,
    new: Option<&Value>,
) -> Option<String> {
    let old = old.and_then(Value::as_object)?;
    let new = new.and_then(Value::as_object)?;
    let keys = old.keys().chain(new.keys()).collect::<BTreeSet<_>>();
    let lines = keys
        .into_iter()
        .filter(|key| old.get(*key) != new.get(*key))
        .map(|key| {
            format!(
                "  - **{}:** `{}` → `{}`",
                humanize(key),
                truncate(&compact(old.get(key)).replace('\n', " ").replace('`', "'"), 180),
                truncate(&compact(new.get(key)).replace('\n', " ").replace('`', "'"), 180)
            )
        })
        .collect::<Vec<_>>();

    (!lines.is_empty()).then(|| format!("- **{field}:**\n{}", lines.join("\n")))
}

fn compact(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Null) | None => "None".into(),
        Some(value) => value.to_string(),
    }
}

pub fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars.saturating_sub(3)).collect::<String>() + "..."
}
