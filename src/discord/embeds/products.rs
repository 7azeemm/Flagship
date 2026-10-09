use super::{BLUE, DARK, GREEN, RED, nonempty, split_text, truncate};
use crate::core::product::{Product, ProductChange, ProductStatus};
use crate::discord::events::ProductChangeKind;
use serde_json::Value;
use serenity::builder::{CreateActionRow, CreateButton, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter};
use serenity::model::application::ButtonStyle;
use std::collections::BTreeSet;

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
            format!("**{} TND**\n~~{} TND~~ · **{}% off**\nSave {} TND", product.price, original, percent, saved)
        })
        .unwrap_or_else(|| format!("**{} TND**", product.price));

    let event_time = if matches!(kind, ProductChangeKind::Removed) {
        product.removed_at.or(product.updated_at).unwrap_or(product.added_at)
    } else {
        product.updated_at.unwrap_or(product.added_at)
    };

    let mut embed = CreateEmbed::new()
        .author(CreateEmbedAuthor::new(truncate(&format!("{label} · {}", product.retailer), 256)))
        .title(truncate(&nonempty(product.title.trim().into(), "Untitled product"), 256))
        .color(color)
        .field("Price", discount, true)
        .field(
            "Availability",
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
        .timestamp(serenity::model::Timestamp::from_unix_timestamp(event_time.timestamp()).unwrap_or_default());

    embed = embed.url(&product.url);
    embed = embed.thumbnail(&product.image);

    if !changes.is_empty() {
        embed = embed.field(
            format!("↻ What changed · {} fields", changes.len()),
            truncate(&changes.iter().take(20).map(format_change).collect::<Vec<_>>().join("\n"), 900),
            false,
        );
    }

    if !product.description.is_empty() {
        for part in split_text(&truncate(&product.description, 1800), 900, 2).into_iter() {
            embed = embed.field("Description:", part, false);
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
                &product.notes.iter().rev().take(5).rev().map(|note| format!("- {note}")).collect::<Vec<_>>().join("\n"),
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
        CreateButton::new(format!("product:edit_json:{product_id}")).label("Edit JSON").style(ButtonStyle::Primary),
        CreateButton::new(format!("product:reparse:{product_id}")).label("Reparse").style(ButtonStyle::Secondary),
        CreateButton::new(format!("product:note:{product_id}")).label("Add note").style(ButtonStyle::Secondary),
        CreateButton::new(format!("product:remove_confirm:{product_id}")).label("Remove").style(ButtonStyle::Danger),
    ];
    if !product.approved {
        buttons.insert(0, CreateButton::new(format!("review:approve:{product_id}")).label("Approve").style(ButtonStyle::Success));
    }
    vec![CreateActionRow::Buttons(buttons)]
}

pub fn removed_product_actions(url: &str) -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![CreateButton::new_link(url).label("Open retailer page")])]
}

fn product_components(product: &Product) -> String {
    let mut lines = Vec::new();
    let mut details = product.components.iter().collect::<Vec<_>>();
    details.sort_by(|left, right| left.0.cmp(right.0));
    for (key, value) in details.into_iter().take(16) {
        lines.push(format!("- **{}**:  `{}`", humanize(key), value.label()));
    }
    truncate(&lines.join("\n"), 900)
}

fn humanize(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "cpu" => return "CPU".to_string(),
        "gpu" => return "GPU".to_string(),
        "id" => return "ID".to_string(),
        _ => {}
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

    if matches!(field_name, "filters" | "components")
        && let Some(formatted) = format_object_change(&field, Some(old_value), Some(new_value))
    {
        return formatted;
    }

    let old = compact(Some(old_value));
    let new = compact(Some(new_value));

    if old.is_empty() {
        format!("- **{field}:** `{}`", truncate(&new.replace('\n', " ").replace('`', "'"), 150))
    } else {
        format!(
            "- **{field}:** `{}` -> `{}`",
            truncate(&old.replace('\n', " ").replace('`', "'"), 150),
            truncate(&new.replace('\n', " ").replace('`', "'"), 150)
        )
    }
}

fn format_object_change(field: &str, old: Option<&Value>, new: Option<&Value>) -> Option<String> {
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
