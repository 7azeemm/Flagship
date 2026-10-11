use crate::discord::events::ProductChangeKind;
use serenity::model::id::{ChannelId, GuildId};
use std::env;

#[derive(Clone, Debug)]
pub struct DiscordConfig {
    pub token: String,
    pub guild_id: GuildId,
    pub added_product_channel: ChannelId,
    pub edited_product_channel: ChannelId,
    pub removed_product_channel: ChannelId,
    pub alert_channel: ChannelId,
    pub scan_channel: ChannelId,
}

impl DiscordConfig {
    pub fn development() -> bool {
        cfg!(debug_assertions) || env::var("FLAGSHIP_LOCAL_DEV").is_ok_and(|value| value.eq_ignore_ascii_case("true"))
    }

    pub fn enabled(scanning: bool) -> bool {
        let development = Self::development();
        let key = if development {
            "ENABLE_DISCORD_DEV"
        } else {
            "ENABLE_DISCORD"
        };
        env::var(key)
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or_else(|_| {
                if development {
                    env::var("DISCORD_DEV_TOKEN").is_ok_and(|token| !token.trim().is_empty())
                } else {
                    scanning
                }
            })
    }

    pub fn load() -> Result<Self, String> {
        Self::from_values(Self::development(), |name| env::var(name).ok())
    }

    fn from_values(development: bool, get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let prefix = if development { "DISCORD_DEV_" } else { "DISCORD_" };
        let required = |suffix: &str| {
            let name = format!("{prefix}{suffix}");
            get(&name)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| format!("{name} is required"))
        };
        let id = |suffix: &str| {
            let name = format!("{prefix}{suffix}");
            required(suffix)?
                .trim()
                .parse::<u64>()
                .ok()
                .filter(|id| *id != 0)
                .ok_or_else(|| format!("{name} must be a nonzero Discord ID"))
        };
        let token = required("TOKEN")?;
        if development && get("DISCORD_TOKEN").is_some_and(|production| production.trim() == token.trim()) {
            return Err(
                "DISCORD_DEV_TOKEN must belong to a separate development bot; it cannot reuse DISCORD_TOKEN".into(),
            );
        }
        Ok(Self {
            token,
            guild_id: GuildId::new(id("GUILD_ID")?),
            added_product_channel: ChannelId::new(id("ADDED_PRODUCT_CHANNEL_ID")?),
            edited_product_channel: ChannelId::new(id("EDITED_PRODUCT_CHANNEL_ID")?),
            removed_product_channel: ChannelId::new(id("REMOVED_PRODUCT_CHANNEL_ID")?),
            alert_channel: ChannelId::new(id("ALERT_CHANNEL_ID")?),
            scan_channel: ChannelId::new(id("SCAN_CHANNEL_ID")?),
        })
    }

    pub fn product_channel(&self, kind: ProductChangeKind) -> ChannelId {
        match kind {
            ProductChangeKind::New | ProductChangeKind::Viewed => self.added_product_channel,
            ProductChangeKind::Edited => self.edited_product_channel,
            ProductChangeKind::Removed => self.removed_product_channel,
        }
    }
}
