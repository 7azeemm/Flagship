mod delivery;
mod evaluation;
pub mod handlers;
mod rules;
mod storage;

pub use delivery::{deliver, delivery_products};
pub use evaluation::{evaluate, prune_events};
pub use rules::{AlertEvent, AlertRule, AlertTarget, metadata, offer_snapshot, validate_current};
pub use storage::{ALERT_MONTHS, AlertRow, CreateAlertError, MAX_ACTIVE_ALERTS, insert_rule};
