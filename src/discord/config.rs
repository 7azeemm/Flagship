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
    pub fn load() -> Result<Self, String> {
        Ok(Self {
            token: env::var("DISCORD_TOKEN").map_err(|_| "DISCORD_TOKEN is required".to_string())?,
            guild_id: GuildId::new(required_id("DISCORD_GUILD_ID")?),
            added_product_channel: ChannelId::new(required_id("DISCORD_ADDED_PRODUCT_CHANNEL_ID")?),
            edited_product_channel: ChannelId::new(required_id("DISCORD_EDITED_PRODUCT_CHANNEL_ID")?),
            removed_product_channel: ChannelId::new(required_id("DISCORD_REMOVED_PRODUCT_CHANNEL_ID")?),
            alert_channel: ChannelId::new(required_id("DISCORD_ALERT_CHANNEL_ID")?),
            scan_channel: ChannelId::new(required_id("DISCORD_SCAN_CHANNEL_ID")?),
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

fn required_id(name: &str) -> Result<u64, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .trim()
                .parse::<u64>()
                .map_err(|error| format!("Invalid {name}: {error}"))
                .and_then(|id| if id == 0 { Err(format!("{name} must be a nonzero Discord ID")) } else { Ok(id) })
        })
        .transpose()?
        .ok_or_else(|| format!("{name} is required."))
}
