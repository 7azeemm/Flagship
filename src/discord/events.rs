use crate::core::product::Product;
use crate::core::tracking::error_tracker::ErrorCycleSummary;
use crate::core::tracking::scan_cache::{ScanRecord, ScanTrigger};
use crate::core::tracking::scan_report::FailedScope;
use crate::discord::config::DiscordConfig;
use crate::discord::embeds;
use chrono::{DateTime, Utc};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::builder::{CreateMessage, EditMessage, CreateAttachment, CreateAllowedMentions};
use serenity::http::Http;
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::core::retailers::validate_url;
use crate::core::tracking::scan_metrics::ScanMetrics;
use crate::utils::web_client::WebClient;

static EVENT_SENDER: OnceCell<mpsc::UnboundedSender<DiscordEvent>> = OnceCell::new();
static PRODUCT_EVENT_SENDER: OnceCell<mpsc::UnboundedSender<DiscordEvent>> = OnceCell::new();

#[derive(Clone, Copy, Debug)]
pub enum ProductChangeKind {
    New,
    Edited,
    Removed,
    Viewed,
}

#[derive(Clone, Debug)]
pub struct AlertEvent {
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: Vec<(String, String)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanSummary {
    pub completed_at: DateTime<Utc>,
    pub duration_ms: u128,
    pub total_products: usize,
    pub added: usize,
    pub edited: usize,
    pub removed: usize,
    pub scrape_errors: usize,
    pub error_health: ErrorCycleSummary,
    pub pages: usize,
    pub failed_pages: usize,
    pub failed_scopes: Vec<FailedScope>,
    pub attempts: usize,
    pub sections_scanned: usize,
    pub sites_scanned: usize,
    pub change_sites: Vec<(String, usize, usize, usize)>,
    pub change_sections: Vec<(String, usize, usize, usize)>,
    pub top_retailers: Vec<(String, usize)>,
    pub top_sections: Vec<(String, usize)>,
    pub catalog: ScanCatalogMetrics,
    pub site_metrics: Vec<ScanSiteMetric>,
    pub section_metrics: Vec<ScanSectionMetric>,
    pub metrics: ScanMetrics,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScanCatalogMetrics {
    pub in_stock: usize,
    pub out_of_stock: usize,
    pub on_arrive: usize,
    pub on_request: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanSiteMetric {
    pub site: String,
    pub products: usize,
    pub pages: usize,
    pub errors: usize,
    pub duration_ms: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanSectionMetric {
    pub section: String,
    pub products: usize,
    pub sites: usize,
    pub errors: usize,
    pub duration_ms: u128,
}

#[derive(Clone, Debug)]
pub enum DiscordEvent {
    Product {
        kind: ProductChangeKind,
        product: Product,
        changes: Vec<Value>,
    },
    Alert(AlertEvent),
    DebugUrl(String),
    Scan(ScanRecord),
    ScanStarted {
        started_at: DateTime<Utc>,
        trigger: ScanTrigger,
        sections: Vec<String>,
        retailers: Vec<String>,
    },
}

pub fn initialize(http: Arc<Http>, config: DiscordConfig) {
    let (sender, receiver) = mpsc::unbounded_channel();
    let (product_sender, product_receiver) = mpsc::unbounded_channel();
    
    let _ = EVENT_SENDER.set(sender);
    let _ = PRODUCT_EVENT_SENDER.set(product_sender);

    for mut receiver in [receiver, product_receiver] {
        let http = http.clone();
        let config = config.clone();
        tokio::spawn(async move {
            let mut scan_message = None;
            while let Some(event) = receiver.recv().await {
                let result = match event {
                    DiscordEvent::Product {
                        kind,
                        product,
                        changes,
                    } => {
                        let components = if matches!(kind, ProductChangeKind::Removed) {
                            embeds::removed_product_actions(&product.url)
                        } else {
                            embeds::product_actions(&product)
                        };
                        let mut embed = embeds::product(&product, kind, &changes);
                        let mut message = CreateMessage::new().components(components);
                        if let Ok(attachment) = download_product_image(&product).await {
                            embed = embed.thumbnail(format!("attachment://{}", attachment.filename));
                            message = message.add_file(attachment);
                        }
                        let message = message.embed(embed);
                        config
                            .product_channel(kind)
                            .send_message(&http, message)
                            .await
                            .map(|_| ())
                    }
                    DiscordEvent::Scan(record) => {
                        if let Some(message_id) = scan_message.take() {
                            config.scan_channel.edit_message(
                                &http, message_id,
                                EditMessage::new()
                                    .embed(embeds::scan_overview(&record))
                                    .components(embeds::scan_actions(&record.id)),
                            ).await.map(|_| ())
                        } else {
                            config.scan_channel.send_message(
                                &http, CreateMessage::new()
                                    .embed(embeds::scan_overview(&record))
                                    .components(embeds::scan_actions(&record.id)),
                            ).await.map(|_| ())
                        }
                    },
                    DiscordEvent::ScanStarted {
                        started_at,
                        trigger,
                        sections,
                        retailers,
                    } => {
                        scan_message = None;
                        config.scan_channel
                            .send_message(
                                &http,
                                CreateMessage::new().embed(embeds::scan_started(
                                    started_at,
                                    &trigger,
                                    &sections,
                                    &retailers,
                                )),
                            )
                            .await
                            .map(|message| { scan_message = Some(message.id); })
                    },
                    DiscordEvent::DebugUrl(message) => config.alert_channel
                        .send_message(&http, CreateMessage::new()
                            .content(embeds::truncate(&message, 1900))
                            .flags(serenity::model::channel::MessageFlags::SUPPRESS_EMBEDS)
                            .allowed_mentions(CreateAllowedMentions::new()))
                        .await.map(|_| ()),
                    DiscordEvent::Alert(alert) => config
                        .alert_channel
                        .send_message(&http, CreateMessage::new().embed(embeds::alert(&alert)))
                        .await
                        .map(|_| ()),
                };

                if let Err(error) = result {
                    eprintln!("Failed to send Discord event: {error}");
                }
            }
        });
    }
}

pub fn emit(event: DiscordEvent) {
    let sender = if matches!(event, DiscordEvent::Product { .. }) {
        PRODUCT_EVENT_SENDER.get()
    } else {
        EVENT_SENDER.get()
    };
    if let Some(sender) = sender {
        let _ = sender.send(event);
    }
}

pub fn alert(
    level: impl Into<String>,
    target: impl Into<String>,
    message: impl Into<String>,
    fields: Vec<(String, String)>,
) {
    emit(DiscordEvent::Alert(AlertEvent {
        level: level.into(),
        target: target.into(),
        message: message.into(),
        fields,
    }));
}

async fn download_product_image(product: &Product) -> Result<CreateAttachment, String> {
    validate_url(&product.image).map_err(|error| error.to_string())?;

    let client = &WebClient::get().http_client;
    let request = || client.get(&product.image)
        .header(reqwest::header::ACCEPT, "image/webp,image/png,image/jpeg,image/gif;q=0.9,*/*;q=0.1")
        .timeout(std::time::Duration::from_secs(10));

    let mut response = request().send().await.map_err(|error| error.to_string())?;
    if response.status() == reqwest::StatusCode::FORBIDDEN {
        response = request().header(reqwest::header::REFERER, &product.url)
            .send().await.map_err(|error| error.to_string())?;
    }

    let mut response = response.error_for_status().map_err(|error| error.to_string())?;
    const MAX_BYTES: usize = 8 * 1024 * 1024;
    if response.content_length().is_some_and(|size| size > MAX_BYTES as u64) {
        return Err("image exceeds 8 MiB attachment limit".into());
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if bytes.len() + chunk.len() > MAX_BYTES {
            return Err("image exceeds 8 MiB attachment limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }

    let extension = image_extension(&bytes).ok_or("response is not a supported image (possibly HTML or a placeholder)")?;
    Ok(CreateAttachment::bytes(bytes, format!("product.{extension}")))
}

fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { Some("png") }
    else if bytes.starts_with(b"\xff\xd8\xff") { Some("jpg") }
    else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") { Some("gif") }
    else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") { Some("webp") }
    else { None }
}