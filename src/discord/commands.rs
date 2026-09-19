use crate::core::product::ProductStatus;
use crate::core::retailers::RETAILERS;
use crate::core::scanner::CatalogScanner;
use crate::core::sections::{Section, SectionConfig};
use crate::core::storage::{ProductQuery, ProductStorage};
use crate::core::tracking::error_tracker::{ErrorStatusFilter, ErrorTracker};
use crate::core::tracking::scan_cache::ScanTrigger;
use crate::discord::embeds;
use crate::discord::events::{self, DiscordEvent, ProductChangeKind};
use crate::discord::interactions::{respond_product, respond_queue};
use serenity::all::{
    CommandDataOption, CommandDataOptionValue, CommandInteraction, CommandOptionType, Context,
    CreateAutocompleteResponse, CreateCommand, CreateCommandOption, CreateInteractionResponse,
    CreateInteractionResponseMessage, EditInteractionResponse, Permissions,
};
use std::str::FromStr;
use strum::IntoEnumIterator;

pub fn definitions() -> Vec<CreateCommand> {
    let permissions = Permissions::MANAGE_GUILD;
    vec![
        CreateCommand::new("product")
            .description("Find a product by ID, text, section, retailer, stock, or price")
            .default_member_permissions(permissions)
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "id", "Exact product ID")
                    .required(false)
                    .set_autocomplete(true),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "query",
                    "Product title or description",
                )
                .required(false)
                .set_autocomplete(true),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "section",
                    "Only products in this section",
                )
                .required(false)
                .set_autocomplete(true),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "site",
                    "Only products from this retailer",
                )
                .required(false)
                .set_autocomplete(true),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "stock",
                    "Only products with this stock status",
                )
                .required(false)
                .add_string_choice("In stock", "InStock")
                .add_string_choice("Out of stock", "OutOfStock")
                .add_string_choice("On arrival", "OnArrive")
                .add_string_choice("On request", "OnRequest"),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "min_price",
                    "Minimum price in TND",
                )
                .min_int_value(0)
                .required(false),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::Integer,
                    "max_price",
                    "Maximum price in TND",
                )
                .min_int_value(0)
                .required(false),
            ),
        CreateCommand::new("review-queue")
            .description("Open the pending new-product review queue")
            .default_member_permissions(permissions)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "section",
                    "Only products in this section",
                )
                .required(false)
                .set_autocomplete(true),
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::Integer, "page", "Queue page to open")
                    .min_int_value(1)
                    .required(false),
            ),
        CreateCommand::new("errors")
            .description("Track scraper errors and mark them reviewed or unreviewed")
            .default_member_permissions(permissions)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "status",
                    "Current error status",
                )
                .add_string_choice("Active", "active")
                .add_string_choice("Inactive", "inactive")
                .add_string_choice("All", "all")
                .required(false),
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "site", "Only one retailer")
                    .set_autocomplete(true)
                    .required(false),
            ),
        CreateCommand::new("reparse")
            .description("Reparse every stored product in a section")
            .default_member_permissions(permissions)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "section",
                    "Section to reparse",
                )
                .required(true)
                .set_autocomplete(true),
            ),
        CreateCommand::new("reload")
            .description("Reload section configs and datasets")
            .default_member_permissions(permissions),
        CreateCommand::new("scan")
            .description("Run a catalog scan")
            .default_member_permissions(permissions)
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "run", "Start a catalog scan")
                    .add_sub_option(
                        CreateCommandOption::new(
                            CommandOptionType::String,
                            "sections",
                            "Sections to scan, separated by commas",
                        )
                            .set_autocomplete(true)
                            .required(false),
                    )
                    .add_sub_option(
                        CreateCommandOption::new(
                            CommandOptionType::String,
                            "retailers",
                            "Retailers to scan, separated by commas",
                        )
                            .set_autocomplete(true)
                            .required(false),
                    ),
            )
    ]
}

pub async fn handle(ctx: &Context, command: &CommandInteraction) -> Result<(), String> {
    match command.data.name.as_str() {
        "product" => {
            let product = match option(command, "id") {
                Some(id) => ProductStorage::find_by_id(id).await.ok_or_else(|| "Product ID not found".to_string())?,
                None => {
                    let query = product_query(command)?;
                    if query_is_empty(&query) {
                        return Err("Provide an `id`, `query`, or at least one filter".into());
                    }
                    ProductStorage::query(&query, 1)
                        .await
                        .into_iter()
                        .next()
                        .ok_or_else(|| "Product not found".to_string())?
                }
            };
            respond_product(ctx, command, &product).await
        }
        "review-queue" => {
            let page = integer_option(&command.data.options, "page").unwrap_or(1).max(1) as usize - 1;
            let section = option(command, "section")
                .map(Section::from_str)
                .transpose()?;
            respond_queue(ctx, command, page, section).await
        }
        "scan" => handle_scan(ctx, command).await,
        "errors" => handle_errors(ctx, command).await,
        "reparse" => handle_reparse(ctx, command).await,
        "reload" => handle_reload(ctx, command).await,
        _ => Ok(()),
    }
}

async fn handle_reparse(ctx: &Context, command: &CommandInteraction) -> Result<(), String> {
    let section_name = option(command, "section").ok_or_else(|| "Choose a section".to_string())?;
    let section = Section::from_str(section_name)?;

    command
        .defer_ephemeral(&ctx.http)
        .await
        .map_err(|error| error.to_string())?;

    let summary = match ProductStorage::reparse_section(section).await {
        Ok(summary) => summary,
        Err(error) => {
            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content(format!("Reparse failed: {error}")),
                )
                .await
                .map_err(|response_error| response_error.to_string())?;
            return Ok(());
        }
    };
    for update in &summary.updates {
        events::emit(DiscordEvent::Product {
            kind: ProductChangeKind::Edited,
            product: update.product.clone(),
            changes: update.changes.clone(),
        });
    }
    let unchanged = summary.total.saturating_sub(summary.changed);
    command.edit_response(
            &ctx.http,
            EditInteractionResponse::new().content(format!(
                "Reparsed **{}** products starting in **{section}**:\n**{}** changed\n**{}** moved to another section\n**{unchanged}** unchanged.",
                summary.total, summary.changed, summary.moved,
            )),
        )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn handle_reload(ctx: &Context, command: &CommandInteraction) -> Result<(), String> {
    command
        .defer_ephemeral(&ctx.http)
        .await
        .map_err(|error| error.to_string())?;

    let content = match SectionConfig::reload().await {
        Ok(count) => format!("Reloaded configs and datasets for **{count}** sections."),
        Err(error) => format!("Reload failed; the existing configs are still active: {error}"),
    };

    command
        .edit_response(&ctx.http, EditInteractionResponse::new().content(content))
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn handle_errors(ctx: &Context, command: &CommandInteraction) -> Result<(), String> {
    let status_name = option(command, "status").unwrap_or("active");
    let status = match status_name {
        "inactive" => ErrorStatusFilter::Inactive,
        "all" => ErrorStatusFilter::All,
        _ => ErrorStatusFilter::Active,
    };
    let site = option(command, "site");
    let records = ErrorTracker::records(status, site).await;
    let page_count = embeds::error_registry_page_count(&records);
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(embeds::error_registry(&records, status_name, site, 0))
                    .components(embeds::error_registry_actions(
                        &records,
                        status_name,
                        site,
                        0,
                        page_count,
                    ))
                    .ephemeral(true),
            ),
        )
        .await
        .map_err(|error| error.to_string())
}

pub async fn autocomplete(ctx: &Context, command: &CommandInteraction) -> Result<(), String> {
    let mut response = CreateAutocompleteResponse::new();
    let (name, query) = focused_option(&command.data.options).unwrap_or(("", ""));
    match name {
        "query" | "id" => {
            let mut filters = product_query(command).unwrap_or_default();
            filters.text = Some(query.to_string());
            for product in ProductStorage::query(&filters, 25).await {
                response = response.add_string_choice(
                    embeds::truncate(
                        &format!(
                            "{} | {} | {} TND",
                            product.title, product.retailer, product.price
                        ),
                        100,
                    ),
                    product.id,
                );
            }
        }
        "sections" if command.data.name == "scan" => {
            add_list_autocomplete(
                &mut response,
                query,
                Section::iter().map(|item| item.to_string()),
            );
        }
        "retailers" if command.data.name == "scan" => {
            add_list_autocomplete(
                &mut response,
                query,
                RETAILERS.iter().map(|site| site.name().to_string()),
            );
        }
        "section" => {
            for section in Section::iter()
                .filter(|section| {
                    section
                        .to_string()
                        .to_lowercase()
                        .contains(&query.to_lowercase())
                })
                .take(25)
            {
                response = response.add_string_choice(section.to_string(), section.to_string());
            }
        }
        "site" => {
            for site in RETAILERS
                .iter()
                .map(|site| site.name())
                .filter(|site| site.to_lowercase().contains(&query.to_lowercase()))
                .take(25)
            {
                response = response.add_string_choice(site, site);
            }
        }
        _ => {}
    }
    command
        .create_response(&ctx.http, CreateInteractionResponse::Autocomplete(response))
        .await
        .map_err(|error| error.to_string())
}

async fn handle_scan(ctx: &Context, command: &CommandInteraction) -> Result<(), String> {
    let (subcommand, options) = subcommand(&command.data.options).ok_or_else(|| "Choose run".to_string())?;
    match subcommand {
        "run" => run_scan(ctx, command, options).await,
        _ => Err("Unknown scan action".into()),
    }
}

async fn run_scan(
    ctx: &Context,
    command: &CommandInteraction,
    options: &[CommandDataOption],
) -> Result<(), String> {
    let scanner = CatalogScanner::try_get().ok_or_else(|| "The scanner is still starting".to_string())?;
    let section_names = csv_values(string_option(options, "sections"));
    let sections = match section_names.is_empty() {
        false => section_names
            .iter()
            .map(|section| Section::from_str(section))
            .collect::<Result<Vec<_>, _>>()?,
        true => Section::iter().collect(),
    };
    let requested_retailers = csv_values(string_option(options, "retailers"));
    let mut sites = Vec::new();
    for requested in requested_retailers {
        let site = RETAILERS
            .iter()
            .map(|site| site.name())
            .find(|site| site.eq_ignore_ascii_case(&requested))
            .ok_or_else(|| format!("Unknown retailer: {requested}"))?;
        if !sites.contains(&site.to_string()) {
            sites.push(site.to_string());
        }
    }
    let scope = format!(
        "{} / {}",
        if section_names.is_empty() {
            "all sections".into()
        } else {
            section_names.join(", ")
        },
        if sites.is_empty() {
            "all retailers".into()
        } else {
            sites.join(", ")
        },
    );
    let trigger = ScanTrigger::Discord {
        user_name: command.user.name.clone(),
    };
    scanner.spawn(sections, sites, trigger)?;
    command.create_response(
        &ctx.http,
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(format!("Catalog scan started for **{scope}**. The detailed report will be posted automatically when it finishes."))
                .ephemeral(true),
        ),
    ).await.map_err(|error| error.to_string())?;
    Ok(())
}

fn product_query(command: &CommandInteraction) -> Result<ProductQuery, String> {
    let min_price = integer_option(&command.data.options, "min_price")
        .map(|value| i32::try_from(value).map_err(|_| "Minimum price is too large"))
        .transpose()?;
    let max_price = integer_option(&command.data.options, "max_price")
        .map(|value| i32::try_from(value).map_err(|_| "Maximum price is too large"))
        .transpose()?;
    if min_price.zip(max_price).is_some_and(|(min, max)| min > max) {
        return Err("Minimum price cannot exceed maximum price".into());
    }
    Ok(ProductQuery {
        text: option(command, "query").map(str::to_string),
        section: option(command, "section").map(Section::from_str).transpose()?,
        site: option(command, "site").map(str::to_string),
        status: option(command, "stock").map(ProductStatus::from_str).transpose().map_err(|err| err.to_string())?,
        min_price,
        max_price,
        exclude_others: true,
    })
}

fn csv_values(value: Option<&str>) -> Vec<String> {
    let mut values = Vec::new();
    for item in value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        if !values.iter().any(|existing: &String| existing.eq_ignore_ascii_case(item)) {
            values.push(item.to_string());
        }
    }
    values
}

fn add_list_autocomplete(
    response: &mut CreateAutocompleteResponse,
    query: &str,
    values: impl Iterator<Item = String>,
) {
    let (prefix, current) = query
        .rsplit_once(',')
        .map_or(("", query), |(prefix, current)| (prefix, current));
    let current = current.trim().to_lowercase();
    let selected = csv_values(Some(prefix));
    for value in values
        .filter(|value| value.to_lowercase().contains(&current))
        .filter(|value| {
            !selected
                .iter()
                .any(|selected| selected.eq_ignore_ascii_case(value))
        })
        .take(25)
    {
        let completed = if prefix.is_empty() {
            value.clone()
        } else {
            format!("{}, {}", prefix.trim(), value)
        };
        *response = std::mem::take(response).add_string_choice(completed.clone(), completed);
    }
}

fn query_is_empty(query: &ProductQuery) -> bool {
    query.text.is_none()
        && query.section.is_none()
        && query.site.is_none()
        && query.status.is_none()
        && query.min_price.is_none()
        && query.max_price.is_none()
}

fn option<'a>(command: &'a CommandInteraction, name: &str) -> Option<&'a str> {
    string_option(&command.data.options, name)
}

fn subcommand(options: &[CommandDataOption]) -> Option<(&str, &[CommandDataOption])> {
    options.first().and_then(|option| match &option.value {
        CommandDataOptionValue::SubCommand(options) => {
            Some((option.name.as_str(), options.as_slice()))
        }
        _ => None,
    })
}

fn string_option<'a>(options: &'a [CommandDataOption], name: &str) -> Option<&'a str> {
    options
        .iter()
        .find(|option| option.name == name)
        .and_then(|option| option.value.as_str())
}

fn integer_option(options: &[CommandDataOption], name: &str) -> Option<i64> {
    options.iter().find(|option| option.name == name).and_then(|option| option.value.as_i64())
}

fn focused_option(options: &[CommandDataOption]) -> Option<(&str, &str)> {
    for option in options {
        match &option.value {
            CommandDataOptionValue::Autocomplete { value, .. } => return Some((&option.name, value)),
            CommandDataOptionValue::SubCommand(children) | CommandDataOptionValue::SubCommandGroup(children) => {
                if let Some(found) = focused_option(children) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}
