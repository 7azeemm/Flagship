use crate::core::parsers::SectionParser;
use crate::core::product::{Product, ProductChange};
use crate::core::sections::Section;
use crate::core::storage::ProductStorage;
use crate::core::tracking::error_tracker::{ErrorCycleSummary, ErrorStatusFilter, ErrorTracker};
use crate::core::tracking::scan_cache::ScanCache;
use crate::discord::embeds;
use crate::discord::events::{self, DiscordEvent, ProductChangeKind};
use serde::Serialize;
use serenity::all::{
    ActionRowComponent, CommandInteraction, ComponentInteraction, Context, CreateActionRow,
    CreateButton, CreateInputText, CreateInteractionResponse, CreateInteractionResponseMessage,
    CreateModal, EditInteractionResponse, EditMessage, InputTextStyle, MessageId, ModalInteraction,
    Permissions,
};

pub async fn handle_component(
    ctx: &Context,
    component: &ComponentInteraction,
) -> Result<(), String> {
    let parts: Vec<_> = component.data.custom_id.split(':').collect();
    defer_scan_or_error_component(ctx, component, &parts).await?;
    match parts.as_slice() {
        ["queue", "page", page] => {
            show_queue_component(ctx, component, page.parse().unwrap_or(0), None).await
        }
        ["queue", "page", section, page] => {
            let section = (*section != "*")
                .then(|| Section::from_str(section))
                .transpose()?;
            show_queue_component(ctx, component, page.parse().unwrap_or(0), section).await
        }
        ["scan", "part", scan_id, part] => show_scan_part(ctx, component, scan_id, part).await,
        ["errors", "cycle-open", scan_id] => open_error_cycle(ctx, component, scan_id).await,
        ["errors", "cycle-page", scan_id, page] => {
            show_error_cycle_page(ctx, component, scan_id, page.parse().unwrap_or(0)).await
        }
        ["errors", "cycle-review-group", scan_id, page, reviewed] |
        ["errors", "cycle-group", scan_id, page, reviewed] => {
            set_error_cycle_group_review(
                ctx,
                component,
                scan_id,
                page.parse().unwrap_or(0),
                *reviewed == "true",
            )
            .await
        }
        ["errors", "cycle-item", scan_id, group, item] |
        ["errors", "cycle-site", scan_id, group, item] => {
            show_error_cycle_item(
                ctx,
                component,
                scan_id,
                group.parse().unwrap_or(0),
                item.parse().unwrap_or(0),
            )
            .await
        }
        ["errors", "cycle-review-item", scan_id, group, item, reviewed] |
        ["errors", "cycle-item-set", scan_id, group, item, reviewed] => {
            set_error_cycle_item_review(
                ctx,
                component,
                scan_id,
                group.parse().unwrap_or(0),
                item.parse().unwrap_or(0),
                *reviewed == "true",
            )
            .await
        }
        ["errors", "registry-page", status, site, page] => {
            show_error_registry_page(
                ctx,
                component,
                status,
                (*site != "*").then_some(*site),
                page.parse().unwrap_or(0),
            )
            .await
        }
        ["errors", "registry-review-group", status, site, page, reviewed] |
        ["errors", "registry-group", status, site, page, reviewed] => {
            set_error_registry_group_review(
                ctx,
                component,
                status,
                (*site != "*").then_some(*site),
                page.parse().unwrap_or(0),
                *reviewed == "true",
            )
            .await
        }
        ["errors", "registry-item", status, site, group, item] |
        ["errors", "registry-site", status, site, group, item] => {
            show_error_registry_item(
                ctx,
                component,
                status,
                (*site != "*").then_some(*site),
                group.parse().unwrap_or(0),
                item.parse().unwrap_or(0),
            )
            .await
        }
        ["errors", "registry-review-item", status, site, group, item, reviewed] |
        ["errors", "registry-item-set", status, site, group, item, reviewed] => {
            set_error_registry_item_review(
                ctx,
                component,
                status,
                (*site != "*").then_some(*site),
                group.parse().unwrap_or(0),
                item.parse().unwrap_or(0),
                *reviewed == "true",
            )
            .await
        }
        ["review", "approve", product_id] => {
            let product = ProductStorage::approve(product_id).await?;
            update_product_component(ctx, component, &product).await
        }
        ["product", "edit_json", product_id] => show_json_editor(ctx, component, product_id).await,
        ["product", "note", product_id] => show_note_modal(ctx, component, product_id).await,
        ["product", "reparse", product_id] => reparse(ctx, component, product_id).await,
        ["product", "remove_confirm", product_id] => show_remove_confirmation(ctx, component, product_id).await,
        ["product", "remove", product_id] => remove_product(ctx, component, product_id).await,
        ["product", "remove", product_id, channel_id, message_id] => {
            remove_product_and_card(ctx, component, product_id, channel_id, message_id).await
        }
        ["product", "remove_cancel", _] => {
            ephemeral_component(ctx, component, "Removal cancelled.").await
        }
        _ => Err("Unknown component action".into()),
    }
}

async fn defer_scan_or_error_component(
    ctx: &Context,
    component: &ComponentInteraction,
    parts: &[&str],
) -> Result<(), String> {
    match parts {
        ["scan", "part", ..] | ["errors", "cycle-open", ..] => component
            .defer_ephemeral(&ctx.http)
            .await
            .map_err(|error| error.to_string()),
        ["errors", ..] => component
            .defer(&ctx.http)
            .await
            .map_err(|error| error.to_string()),
        _ => Ok(()),
    }
}

async fn show_scan_part(
    ctx: &Context,
    component: &ComponentInteraction,
    scan_id: &str,
    part: &str,
) -> Result<(), String> {
    let mut record = ScanCache::find(scan_id).await.ok_or_else(|| "This scan report has expired".to_string())?;
    if matches!(part, "overview" | "errors") {
        ErrorTracker::refresh_review_state(&mut record.summary.error_health).await;
    }

    let embed = embeds::scan_part(&record, part).ok_or_else(|| "Unknown scan report part".to_string())?;
    let mut components = embeds::scan_actions(scan_id);
    if part == "errors" && !record.summary.error_health.cycle_records.is_empty() {
        components.extend(embeds::error_cycle_open_action(scan_id));
    }

    component.edit_response(
        &ctx.http,
        EditInteractionResponse::new()
            .embed(embed)
            .components(components),
    )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn open_error_cycle(
    ctx: &Context,
    component: &ComponentInteraction,
    scan_id: &str,
) -> Result<(), String> {
    let summary = scan_error_summary(scan_id).await?;
    let page_count = embeds::error_cycle_page_count(&summary);
    component
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .embed(embeds::error_cycle_page(&summary, 0))
                .components(embeds::error_cycle_page_actions(
                    &summary, scan_id, 0, page_count,
                )),
        )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn scan_error_summary(scan_id: &str) -> Result<ErrorCycleSummary, String> {
    let mut record = ScanCache::find(scan_id).await.ok_or_else(|| "This scan report has expired".to_string())?;
    ErrorTracker::refresh_review_state(&mut record.summary.error_health).await;
    Ok(record.summary.error_health)
}

async fn set_error_cycle_group_review(
    ctx: &Context,
    component: &ComponentInteraction,
    scan_id: &str,
    page: usize,
    reviewed: bool,
) -> Result<(), String> {
    let summary = scan_error_summary(scan_id).await?;
    let keys = embeds::error_cycle_group_keys(&summary, page);
    ErrorTracker::set_reviewed(&keys, reviewed).await?;
    show_error_cycle_page(ctx, component, scan_id, page).await
}

async fn set_error_cycle_item_review(
    ctx: &Context,
    component: &ComponentInteraction,
    scan_id: &str,
    group_page: usize,
    item_page: usize,
    reviewed: bool,
) -> Result<(), String> {
    let summary = scan_error_summary(scan_id).await?;
    let key = embeds::error_cycle_individual_key(&summary, group_page, item_page)
        .ok_or_else(|| "The selected error is no longer available".to_string())?;
    ErrorTracker::set_reviewed(&[key], reviewed).await?;
    show_error_cycle_item(ctx, component, scan_id, group_page, item_page).await
}

async fn show_error_cycle_page(
    ctx: &Context,
    component: &ComponentInteraction,
    scan_id: &str,
    requested_page: usize,
) -> Result<(), String> {
    let summary = scan_error_summary(scan_id).await?;
    let page_count = embeds::error_cycle_page_count(&summary);
    let page = requested_page.min(page_count.saturating_sub(1));
    update_component_message(
        ctx,
        component,
        embeds::error_cycle_page(&summary, page),
        embeds::error_cycle_page_actions(&summary, scan_id, page, page_count),
    )
    .await
}

async fn show_error_cycle_item(
    ctx: &Context,
    component: &ComponentInteraction,
    scan_id: &str,
    requested_group: usize,
    requested_item: usize,
) -> Result<(), String> {
    let summary = scan_error_summary(scan_id).await?;
    let group_count = embeds::error_cycle_page_count(&summary);
    let group_page = requested_group.min(group_count.saturating_sub(1));
    let item_count = embeds::error_cycle_individual_page_count(&summary, group_page);
    let item_page = requested_item.min(item_count.saturating_sub(1));
    update_component_message(
        ctx,
        component,
        embeds::error_cycle_individual(&summary, group_page, item_page),
        embeds::error_cycle_individual_actions(
            &summary, scan_id, group_page, item_page, item_count,
        ),
    )
    .await
}

async fn show_error_registry_page(
    ctx: &Context,
    component: &ComponentInteraction,
    status_name: &str,
    site: Option<&str>,
    requested_page: usize,
) -> Result<(), String> {
    let records = ErrorTracker::records(error_status_filter(status_name), site).await;
    let page_count = embeds::error_registry_page_count(&records);
    let page = requested_page.min(page_count.saturating_sub(1));
    update_component_message(
        ctx,
        component,
        embeds::error_registry(&records, status_name, site, page),
        embeds::error_registry_actions(&records, status_name, site, page, page_count),
    )
    .await
}

async fn show_error_registry_item(
    ctx: &Context,
    component: &ComponentInteraction,
    status_name: &str,
    site: Option<&str>,
    requested_group: usize,
    requested_item: usize,
) -> Result<(), String> {
    let records = ErrorTracker::records(error_status_filter(status_name), site).await;
    let group_count = embeds::error_registry_page_count(&records);
    let group_page = requested_group.min(group_count.saturating_sub(1));
    let item_count = embeds::error_registry_individual_page_count(&records, group_page);
    let item_page = requested_item.min(item_count.saturating_sub(1));
    update_component_message(
        ctx,
        component,
        embeds::error_registry_individual(&records, group_page, item_page),
        embeds::error_registry_individual_actions(
            &records,
            status_name,
            site,
            group_page,
            item_page,
            item_count,
        ),
    )
    .await
}

async fn set_error_registry_group_review(
    ctx: &Context,
    component: &ComponentInteraction,
    status_name: &str,
    site: Option<&str>,
    page: usize,
    reviewed: bool,
) -> Result<(), String> {
    let records = ErrorTracker::records(error_status_filter(status_name), site).await;
    let keys = embeds::error_registry_group_keys(&records, page);
    ErrorTracker::set_reviewed(&keys, reviewed).await?;
    show_error_registry_page(ctx, component, status_name, site, page).await
}

async fn set_error_registry_item_review(
    ctx: &Context,
    component: &ComponentInteraction,
    status_name: &str,
    site: Option<&str>,
    group_page: usize,
    item_page: usize,
    reviewed: bool,
) -> Result<(), String> {
    let records = ErrorTracker::records(error_status_filter(status_name), site).await;
    let key = embeds::error_registry_individual_key(&records, group_page, item_page)
        .ok_or_else(|| "The selected error is no longer available".to_string())?;
    ErrorTracker::set_reviewed(&[key], reviewed).await?;
    show_error_registry_item(ctx, component, status_name, site, group_page, item_page).await
}

fn error_status_filter(status_name: &str) -> ErrorStatusFilter {
    match status_name {
        "inactive" => ErrorStatusFilter::Inactive,
        "all" => ErrorStatusFilter::All,
        _ => ErrorStatusFilter::Active,
    }
}

async fn update_component_message(
    ctx: &Context,
    component: &ComponentInteraction,
    embed: serenity::builder::CreateEmbed,
    components: Vec<CreateActionRow>,
) -> Result<(), String> {
    component
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .embed(embed)
                .components(components),
        )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub async fn handle_modal(ctx: &Context, modal: &ModalInteraction) -> Result<(), String> {
    let parts: Vec<_> = modal.data.custom_id.split(':').collect();
    match parts.as_slice() {
        ["modal", "json", product_id] => apply_json_edit(ctx, modal, product_id).await,
        ["modal", "note", product_id] => apply_note(ctx, modal, product_id).await,
        _ => Err("Unknown modal action".into()),
    }
}

pub async fn respond_product(
    ctx: &Context,
    command: &CommandInteraction,
    product: &Product,
) -> Result<(), String> {
    let message = CreateInteractionResponseMessage::new()
        .embed(embeds::product(product, ProductChangeKind::Viewed, &[]))
        .components(embeds::product_actions(product));

    command.create_response(&ctx.http, CreateInteractionResponse::Message(message))
        .await.map_err(|error| error.to_string())
}

pub async fn respond_queue(
    ctx: &Context,
    command: &CommandInteraction,
    page: usize,
    section: Option<Section>,
) -> Result<(), String> {
    let message = queue_message(page, section).await?;
    command.create_response(&ctx.http, CreateInteractionResponse::Message(message))
        .await.map_err(|error| error.to_string())
}

async fn show_queue_component(
    ctx: &Context,
    component: &ComponentInteraction,
    page: usize,
    section: Option<Section>,
) -> Result<(), String> {
    let message = queue_message(page, section).await?;
    component.create_response(&ctx.http, CreateInteractionResponse::UpdateMessage(message))
        .await.map_err(|error| error.to_string())
}

async fn queue_message(
    page: usize,
    section: Option<Section>,
) -> Result<CreateInteractionResponseMessage, String> {
    let pending = ProductStorage::pending_review(section).await;
    if pending.is_empty() {
        let content = section.map_or_else(
            || "The review queue is empty.".to_string(),
            |section| format!("The review queue is empty for section **{section}**."),
        );
        return Ok(CreateInteractionResponseMessage::new().content(content));
    }
    let page = page.min(pending.len() - 1);
    let product = &pending[page];
    let section_id = section.map_or_else(|| "*".to_string(), |section| section.to_string());
    let mut rows = embeds::product_actions(product);
    rows.push(CreateActionRow::Buttons(vec![
        CreateButton::new(format!(
            "queue:page:{section_id}:{}",
            page.saturating_sub(1)
        ))
            .label("Previous")
            .disabled(page == 0),
        CreateButton::new(format!("queue:page:{section_id}:{}", page + 1))
            .label("Next")
            .disabled(page + 1 >= pending.len()),
    ]));
    let section_label = section.map_or_else(String::new, |section| format!(" in **{section}**"));
    let message = CreateInteractionResponseMessage::new()
        .content(format!(
            "Review item **{} / {}**{section_label}",
            page + 1,
            pending.len()
        ))
        .embed(embeds::product(&product, ProductChangeKind::Viewed, &[]))
        .components(rows);
    Ok(message)
}

async fn show_json_editor(
    ctx: &Context,
    component: &ComponentInteraction,
    product_id: &str,
) -> Result<(), String> {
    let product = ProductStorage::get(product_id).await.ok_or("Product not found")?;
    let pretty = serde_json::to_string_pretty(&product)
        .map_err(|error| format!("Failed to serialize product: {error}"))?;

    let json = if pretty.chars().count() <= 19_500 {
        pretty
    } else {
        let compact = serde_json::to_string(&product)
            .map_err(|error| format!("Failed to serialize product: {error}"))?;
        if compact.chars().count() > 19_500 {
            return Err("Product JSON exceeds Discord's total five-field modal capacity".into());
        }
        compact
    };

    let chunks = split_modal_json(&json, 3900);
    let chunk_count = chunks.len();
    let fields = chunks
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| {
            CreateActionRow::InputText(
                CreateInputText::new(
                    InputTextStyle::Paragraph,
                    if chunk_count == 1 {
                        "Product JSON".into()
                    } else {
                        format!("Product JSON {} / {chunk_count}", index + 1)
                    },
                    format!("json_{index}"),
                )
                .value(chunk)
                .max_length(4000),
            )
        })
        .collect();

    let modal = CreateModal::new(
        format!("modal:json:{product_id}"),
        "Edit complete product JSON",
    )
    .components(fields);
    component
        .create_response(&ctx.http, CreateInteractionResponse::Modal(modal))
        .await
        .map_err(|error| error.to_string())
}

async fn show_note_modal(
    ctx: &Context,
    component: &ComponentInteraction,
    product_id: &str,
) -> Result<(), String> {
    let modal = CreateModal::new(format!("modal:note:{product_id}"), "Add review note").components(vec![
        CreateActionRow::InputText(
            CreateInputText::new(InputTextStyle::Paragraph, "Note", "note").max_length(1000),
        ),
    ]);
    component
        .create_response(&ctx.http, CreateInteractionResponse::Modal(modal))
        .await
        .map_err(|error| error.to_string())
}

async fn apply_json_edit(
    ctx: &Context,
    modal: &ModalInteraction,
    product_id: &str,
) -> Result<(), String> {
    let old = ProductStorage::get(product_id).await.ok_or("Product not found")?;
    let raw = modal_value(modal, "json")
        .map(str::to_string)
        .unwrap_or_else(|| (0..5).filter_map(|index| modal_value(modal, &format!("json_{index}"))).collect::<String>());
    if raw.trim().is_empty() {
        return Err("Product JSON cannot be empty".into());
    }

    let mut product: Product = serde_json::from_str(&raw).map_err(|error| format!("Invalid product JSON: {error}"))?;

    if product.id != old.id {
        return Err("Product ID cannot be modified".into())
    }

    let changes = product.record_override_changes(&old, false).await;
    if changes.len() > 0 {
        ProductStorage::replace(product.clone()).await?;
        events::emit(DiscordEvent::Product {
            kind: ProductChangeKind::Edited,
            product: product.clone(),
            changes,
        });
    }

    update_product_modal(ctx, modal, &product).await
}

async fn apply_note(ctx: &Context, modal: &ModalInteraction, product_id: &str) -> Result<(), String> {
    let note = modal_value(modal, "note").ok_or("Missing note")?.trim().to_string();
    let update = ProductStorage::add_note(product_id, note).await?;
    events::emit(DiscordEvent::Product {
        kind: ProductChangeKind::Edited,
        product: update.product.clone(),
        changes: update.changes,
    });
    update_product_modal(ctx, modal, &update.product).await
}

async fn reparse(
    ctx: &Context,
    component: &ComponentInteraction,
    product_id: &str,
) -> Result<(), String> {
    let update = ProductStorage::reparse_product(product_id).await?;
    if !update.changes.is_empty() {
        events::emit(DiscordEvent::Product {
            kind: ProductChangeKind::Edited,
            product: update.product.clone(),
            changes: update.changes,
        });
    }

    update_product_component(ctx, component, &update.product).await
}

async fn show_remove_confirmation(
    ctx: &Context,
    component: &ComponentInteraction,
    product_id: &str,
) -> Result<(), String> {
    component
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .ephemeral(true)
                    .content("Remove this product from storage?")
                    .button(
                        CreateButton::new(format!(
                            "product:remove:{product_id}:{}:{}",
                            component.channel_id.get(),
                            component.message.id.get(),
                        ))
                        .label("Remove permanently")
                        .style(serenity::all::ButtonStyle::Danger),
                    )
                    .button(
                        CreateButton::new(format!("product:remove_cancel:{product_id}"))
                            .label("Cancel"),
                    ),
            ),
        )
        .await
        .map_err(|error| error.to_string())
}

async fn remove_product(
    ctx: &Context,
    component: &ComponentInteraction,
    product_id: &str,
) -> Result<(), String> {
    let product = ProductStorage::remove(product_id).await?;
    events::emit(DiscordEvent::Product {
        kind: ProductChangeKind::Removed,
        product,
        changes: Vec::new(),
    });
    ephemeral_component(ctx, component, "Product removed.").await
}

async fn remove_product_and_card(
    ctx: &Context,
    component: &ComponentInteraction,
    product_id: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<(), String> {
    let product = ProductStorage::remove(product_id).await?;
    events::emit(DiscordEvent::Product {
        kind: ProductChangeKind::Removed,
        product: product.clone(),
        changes: Vec::new(),
    });
    let channel_id = channel_id
        .parse::<u64>()
        .map_err(|_| "Invalid channel ID")?;
    let message_id = message_id
        .parse::<u64>()
        .map_err(|_| "Invalid message ID")?;
    serenity::all::ChannelId::new(channel_id)
        .edit_message(
            &ctx.http,
            MessageId::new(message_id),
            EditMessage::new()
                .embed(embeds::product(&product, ProductChangeKind::Removed, &[]))
                .components(embeds::removed_product_actions(&product.url)),
        )
        .await
        .map_err(|error| {
            format!("Product removed, but the original card could not be refreshed: {error}")
        })?;
    component
        .create_response(
            &ctx.http,
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .content("Product removed and archived.")
                    .components(Vec::new()),
            ),
        )
        .await
        .map_err(|error| error.to_string())
}

async fn update_product_component(
    ctx: &Context,
    interaction: &ComponentInteraction,
    product: &Product,
) -> Result<(), String> {
    interaction
        .create_response(
            &ctx.http,
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .embed(embeds::product(product, ProductChangeKind::Viewed, &[]))
                    .components(embeds::product_actions(product)),
            ),
        )
        .await
        .map_err(|error| error.to_string())
}

async fn update_product_modal(
    ctx: &Context,
    interaction: &ModalInteraction,
    product: &Product,
) -> Result<(), String> {
    interaction
        .create_response(
            &ctx.http,
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .embed(embeds::product(product, ProductChangeKind::Viewed, &[]))
                    .components(embeds::product_actions(product)),
            ),
        )
        .await
        .map_err(|error| error.to_string())
}

pub async fn ephemeral_component(
    ctx: &Context,
    interaction: &ComponentInteraction,
    content: &str,
) -> Result<(), String> {
    interaction
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(content)
                    .ephemeral(true),
            ),
        )
        .await
        .map_err(|error| error.to_string())
}

pub async fn ephemeral_modal(
    ctx: &Context,
    interaction: &ModalInteraction,
    content: &str,
) -> Result<(), String> {
    interaction
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(content)
                    .ephemeral(true),
            ),
        )
        .await
        .map_err(|error| error.to_string())
}

fn modal_value<'a>(modal: &'a ModalInteraction, id: &str) -> Option<&'a str> {
    modal
        .data
        .components
        .iter()
        .flat_map(|row| &row.components)
        .find_map(|component| match component {
            ActionRowComponent::InputText(input) if input.custom_id == id => input.value.as_deref(),
            _ => None,
        })
}

fn split_modal_json(value: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut chunk = String::new();
    let mut chunk_length = 0;
    for character in value.chars() {
        if chunk_length == max_chars {
            chunks.push(chunk);
            chunk = String::new();
            chunk_length = 0;
        }
        chunk.push(character);
        chunk_length += 1;
    }
    if !chunk.is_empty() {
        chunks.push(chunk);
    }
    chunks
}

pub fn has_manage_guild(member: Option<&serenity::all::Member>) -> bool {
    member.and_then(|member| member.permissions)
        .is_some_and(|permissions| permissions.contains(Permissions::MANAGE_GUILD))
}

pub async fn command_error(ctx: &Context, command: &CommandInteraction, error: &str) {
    tracing::warn!(%error, command = %command.data.name, "Discord command failed");
    let _ = command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(error)
                    .ephemeral(true),
            ),
        )
        .await;
}

pub async fn component_error(ctx: &Context, component: &ComponentInteraction, error: &str) {
    tracing::warn!(%error, custom_id = %component.data.custom_id, "Discord component failed");
    let _ = ephemeral_component(ctx, component, error).await;
}

pub async fn modal_error(ctx: &Context, modal: &ModalInteraction, error: &str) {
    tracing::warn!(%error, custom_id = %modal.data.custom_id, "Discord modal failed");
    let _ = ephemeral_modal(ctx, modal, error).await;
}
