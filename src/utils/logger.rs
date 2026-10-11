use crate::discord::events::alert;
use std::collections::BTreeMap;
use std::time::Instant;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::{format::Writer, time::FormatTime};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{Layer, util::SubscriberInitExt};

tokio::task_local! {
    pub(crate) static SCAN_PROGRESS: Arc<ScanLogProgress>;
}

pub(crate) struct ScanLogProgress {
    mode: String,
    pub pages: AtomicUsize,
    pub total_pages: AtomicUsize,
    pub products: AtomicUsize,
}

impl ScanLogProgress {
    pub fn new(mode: &str, total_pages: usize) -> Self {
        Self { mode: mode.into(), pages: AtomicUsize::new(0), total_pages: AtomicUsize::new(total_pages), products: AtomicUsize::new(0) }
    }

    pub fn prefix(&self) -> String {
        format!("[{} pages:{}/{} products:{}]", self.mode, self.pages.load(Ordering::Relaxed), self.total_pages.load(Ordering::Relaxed), self.products.load(Ordering::Relaxed))
    }
}

fn scan_prefix() -> String {
    SCAN_PROGRESS.try_with(|progress| format!("{} ", progress.prefix())).unwrap_or_default()
}

pub fn setup_logging() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let location = panic_info
            .location()
            .map(|location| location.to_string())
            .unwrap_or_else(|| "unknown".into());
        let message = panic_info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic_info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("panic without a string payload");

        alert("error", "panic", message, vec![("Location".into(), location)]);

        default_hook(panic_info);
    }));

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(DiscordLogLayer)
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_target(false)
                .with_timer(LogTime)
                .with_ansi(true),
        )
        .init();
}

pub struct LogTime;

impl FormatTime for LogTime {
    fn format_time(&self, writer: &mut Writer<'_>) -> std::fmt::Result {
        write!(writer, "{}", chrono::Utc::now().format("%H:%M:%S"))
    }
}

/// Keep routine request logs readable; debug events retain the complete URL and error.
pub struct FetchLog<'a> {
    site: &'a str,
    url: &'a str,
    started: Instant,
}

impl<'a> FetchLog<'a> {
    pub fn new(site: &'a str, url: &'a str) -> Self {
        tracing::debug!(site, url, "Product fetch started");
        Self {
            site,
            url,
            started: Instant::now(),
        }
    }

    pub fn completed(&self, attempt: i32, bytes: usize, cached: bool) {
        tracing::debug!(
            site = self.site,
            url = self.url,
            attempt,
            bytes,
            elapsed_ms = self.started.elapsed().as_millis(),
            "Product fetch completed"
        );
        let page = page_label(self.url);
        let seconds = self.started.elapsed().as_secs_f64();
        let progress = scan_prefix();
        if cached {
            tracing::info!(target: "retailer_fetch", "{progress}{} fetched {} ({seconds:.1}s)", self.site, page);
        } else {
            tracing::warn!(target: "retailer_fetch", "{progress}{} empty {} ({seconds:.1}s)", self.site, page);
        }
    }

    pub fn failed(&self, attempt: i32, error: &str, retry: bool) {
        tracing::debug!(
            site = self.site,
            url = self.url,
            attempt,
            error,
            retry,
            elapsed_ms = self.started.elapsed().as_millis(),
            "Product fetch failed"
        );
        let page = page_label(self.url);
        let error = short_error(error, self.url);
        let seconds = self.started.elapsed().as_secs_f64();
        let progress = scan_prefix();
        if retry {
            tracing::warn!(target: "retailer_fetch", "{progress}{} retry {attempt} {page} ({seconds:.1}s): {error}", self.site);
        } else {
            tracing::error!(target: "retailer_fetch", "{progress}{} failed {page} ({seconds:.1}s): {error}", self.site);
        }
    }
}

fn page_label(url: &str) -> String {
    let parsed = reqwest::Url::parse(url).ok();
    let path = parsed.as_ref().map_or(url, |url| url.path());
    let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or(path);
    let decoded = urlencoding::decode(name).unwrap_or_else(|_| name.into());
    shorten(&decoded, 42)
}

fn short_error(error: &str, url: &str) -> String {
    let error = error.replace(url, "");
    let line = error.lines().next().unwrap_or_default();
    shorten(line.trim().trim_end_matches(':'), 72)
}

fn shorten(value: &str, limit: usize) -> String {
    let chars: Vec<_> = value.chars().collect();
    if chars.len() <= limit {
        return value.to_owned();
    }
    let prefix: String = chars[..limit - 13].iter().collect();
    let suffix: String = chars[chars.len() - 10..].iter().collect();
    format!("{prefix}...{suffix}")
}

struct DiscordLogLayer;

impl<S> Layer<S> for DiscordLogLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        // Expected retailer retries/failures are already included in scan error reports.
        if metadata.target() == "retailer_fetch" || !matches!(*metadata.level(), Level::ERROR | Level::WARN) {
            return;
        }

        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        let message = visitor
            .fields
            .remove("message")
            .unwrap_or_else(|| metadata.name().to_string());

        alert(
            metadata.level().as_str().to_lowercase(),
            metadata.target(),
            message,
            visitor.fields.into_iter().collect(),
        );
    }
}

#[derive(Default)]
struct FieldVisitor {
    fields: BTreeMap<String, String>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(
            field.name().to_string(),
            format!("{value:?}").trim_matches('"').to_string(),
        );
    }
}
