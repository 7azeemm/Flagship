mod errors;
mod products;
mod scans;
use crate::discord::events::AlertEvent;
pub use errors::{
    error_cycle_group_keys, error_cycle_individual, error_cycle_individual_actions, error_cycle_individual_key, error_cycle_individual_page_count, error_cycle_open_action,
    error_cycle_page, error_cycle_page_actions, error_cycle_page_count, error_registry, error_registry_actions, error_registry_group_keys, error_registry_individual,
    error_registry_individual_actions, error_registry_individual_key, error_registry_individual_page_count, error_registry_page_count, scan_error_report,
};
pub use products::{product, product_actions, removed_product_actions};
pub use scans::{scan_actions, scan_overview, scan_part, scan_started};
use serenity::builder::{CreateEmbed, CreateEmbedAuthor};

const BLUE: u32 = 0x3B82F6;
const GREEN: u32 = 0x10B981;
const YELLOW: u32 = 0xF59E0B;
const RED: u32 = 0xEF4444;
const PURPLE: u32 = 0x8B5CF6;
const DARK: u32 = 0x334155;

pub fn alert(alert: &AlertEvent) -> CreateEmbed {
    let is_error = alert.level.eq_ignore_ascii_case("error");
    let mut embed = CreateEmbed::new()
        .author(CreateEmbedAuthor::new(if is_error { "SYSTEM ERROR" } else { "SYSTEM WARNING" }))
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

pub(super) fn split_text(value: &str, max_chars: usize, max_parts: usize) -> Vec<String> {
    if value.is_empty() || max_chars == 0 || max_parts == 0 {
        return Vec::new();
    }
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut current_length = 0;
    for character in value.chars() {
        if current_length == max_chars {
            parts.push(current);
            current = String::new();
            current_length = 0;
            if parts.len() == max_parts {
                return parts;
            }
        }
        current.push(character);
        current_length += 1;
    }
    if !current.is_empty() && parts.len() < max_parts {
        parts.push(current);
    }
    parts
}

fn nonempty(value: String, fallback: &str) -> String {
    if value.is_empty() { fallback.to_string() } else { value }
}

pub fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().nth(max_chars).is_none() {
        return value.to_string();
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }
    value.chars().take(max_chars.saturating_sub(3)).collect::<String>() + "..."
}
