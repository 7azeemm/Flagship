use crate::discord::config::DiscordConfig;
use crate::discord::{commands, events, interactions};
use async_trait::async_trait;
use serenity::Client;
use serenity::all::{Context, EventHandler, GatewayIntents, GuildId, Interaction, Ready};
use std::sync::Arc;

pub async fn start() {
    let config = match DiscordConfig::load() {
        Ok(config) => config,
        Err(error) => {
            panic!("Discord bot configuration is invalid: {error}")
        }
    };

    super::moderation::load_routes(config.guild_id).await.expect("Failed to load Discord channel routes");
    let http = Arc::new(serenity::http::Http::new(&config.token));
    events::initialize(http, config.clone());

    let builder = Client::builder(&config.token, GatewayIntents::GUILDS);
    let handler = Handler(config.guild_id);

    let mut client = match builder.event_handler(handler).await {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(%error, "Failed to create Discord client");
            return;
        }
    };

    tokio::spawn(async move {
        if let Err(error) = client.start().await {
            tracing::error!(%error, "Discord client stopped");
        }
    });
}

struct Handler(GuildId);

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        match self.0.set_commands(&ctx.http, commands::definitions()).await {
            Ok(_) => tracing::info!(user = %ready.user.name, "Discord bot is ready"),
            Err(error) => tracing::error!(%error, "Failed to register Discord slash commands"),
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if interaction.guild_id() != Some(self.0) { return; }
        match interaction {
            Interaction::Command(command) => {
                if let Err(error) = commands::handle(&ctx, &command).await {
                    interactions::command_error(&ctx, &command, &error).await;
                }
            }
            Interaction::Autocomplete(command) => {
                if let Err(error) = commands::autocomplete(&ctx, &command).await {
                    tracing::error!(%error, "Discord autocomplete failed");
                }
            }
            Interaction::Component(component) => {
                if !interactions::has_manage_guild(component.member.as_ref()) {
                    let _ = interactions::ephemeral_component(
                        &ctx,
                        &component,
                        "You need Manage Server permission to use this control.",
                    ).await;
                } else if let Err(error) = interactions::handle_component(&ctx, &component).await {
                    interactions::component_error(&ctx, &component, &error).await;
                }
            }
            Interaction::Modal(modal) => {
                if !interactions::has_manage_guild(modal.member.as_ref()) {
                    let _ = interactions::ephemeral_modal(
                        &ctx,
                        &modal,
                        "You need Manage Server permission to submit this form.",
                    ).await;
                } else if let Err(error) = interactions::handle_modal(&ctx, &modal).await {
                    interactions::modal_error(&ctx, &modal, &error).await;
                }
            }
            _ => {}
        }
    }
}
