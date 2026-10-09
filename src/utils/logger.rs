use crate::discord::events::alert;
use std::collections::BTreeMap;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{Layer, util::SubscriberInitExt};

pub fn setup_logging() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let location = panic_info.location().map(|location| location.to_string()).unwrap_or_else(|| "unknown".into());
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
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();
}

struct DiscordLogLayer;

impl<S> Layer<S> for DiscordLogLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        if !matches!(*metadata.level(), Level::ERROR | Level::WARN) {
            return;
        }

        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        let message = visitor.fields.remove("message").unwrap_or_else(|| metadata.name().to_string());

        alert(metadata.level().as_str().to_lowercase(), metadata.target(), message, visitor.fields.into_iter().collect());
    }
}

#[derive(Default)]
struct FieldVisitor {
    fields: BTreeMap<String, String>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(field.name().to_string(), format!("{value:?}").trim_matches('"').to_string());
    }
}
