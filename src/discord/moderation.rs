use crate::database::Database;
use chrono::{Duration, Utc};
use serenity::all::{
    ChannelId, ChannelType, CommandDataOptionValue, CommandInteraction, Context, CreateChannel, CreateMessage,
    EditInteractionResponse, GetMessages, GuildId, Permissions,
};
use std::collections::HashMap;
use std::sync::LazyLock;
use tokio::sync::RwLock;

pub(super) static ROUTES: LazyLock<RwLock<HashMap<ChannelId, ChannelId>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub(super) fn route(routes: &HashMap<ChannelId, ChannelId>, channel: ChannelId) -> ChannelId {
    routes.get(&channel).copied().unwrap_or(channel)
}

pub(super) async fn load_routes(guild: GuildId) -> Result<(), String> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT original_channel_id,current_channel_id FROM discord_channel_routes WHERE guild_id=$1")
            .bind(guild.to_string())
            .fetch_all(Database::pool())
            .await
            .map_err(|e| e.to_string())?;
    let mut routes = ROUTES.write().await;
    for (original, current) in rows {
        routes.insert(
            ChannelId::new(original.parse::<u64>().map_err(|e| e.to_string())?),
            ChannelId::new(current.parse::<u64>().map_err(|e| e.to_string())?),
        );
    }
    Ok(())
}

fn require_permission(command: &CommandInteraction, permission: Permissions) -> Result<(), String> {
    if command
        .member
        .as_ref()
        .and_then(|member| member.permissions)
        .is_some_and(|permissions| permissions.contains(permission))
    {
        Ok(())
    } else {
        Err(format!("You need {permission} permission in this channel."))
    }
}

pub(super) async fn clear(ctx: &Context, command: &CommandInteraction) -> Result<(), String> {
    let count = recent_count(&command.data.options)?;
    let Some(count) = count else {
        return reset(ctx, command).await;
    };
    require_permission(command, Permissions::MANAGE_MESSAGES)?;
    if !command.app_permissions.is_some_and(|permissions| {
        permissions
            .contains(Permissions::MANAGE_MESSAGES | Permissions::READ_MESSAGE_HISTORY | Permissions::VIEW_CHANNEL)
    }) {
        return Err("The bot needs View Channel, Read Message History and Manage Messages permissions.".into());
    }
    command.defer_ephemeral(&ctx.http).await.map_err(|e| e.to_string())?;
    let _routes = ROUTES.read().await;
    let messages = command
        .channel_id
        .messages(&ctx.http, GetMessages::new().limit(count as u8))
        .await
        .map_err(|e| e.to_string())?;
    // Leave a margin at Discord's two-week boundary so messages do not age out
    // between retrieval and the bulk-delete request.
    let cutoff = Utc::now() - Duration::days(14) + Duration::minutes(5);
    let ids = messages
        .iter()
        .filter(|message| !message.pinned && message.timestamp.unix_timestamp() > cutoff.timestamp())
        .map(|message| message.id)
        .collect::<Vec<_>>();
    if !ids.is_empty() {
        command
            .channel_id
            .delete_messages(&ctx.http, &ids)
            .await
            .map_err(|e| e.to_string())?;
    }
    command.edit_response(&ctx.http, EditInteractionResponse::new().content(format!("Deleted {} messages. Skipped {} pinned or old messages. Use `/clear-chat` without a count to permanently reset the entire channel.", ids.len(), messages.len() - ids.len())))
        .await.map(|_| ()).map_err(|e| e.to_string())
}

fn recent_count(options: &[serenity::all::CommandDataOption]) -> Result<Option<u8>, String> {
    match options
        .iter()
        .find(|option| option.name == "count")
        .map(|option| &option.value)
    {
        None => Ok(None),
        Some(CommandDataOptionValue::Integer(count)) if (1..=100).contains(count) => Ok(Some(*count as u8)),
        _ => Err("Choose a count from 1 to 100".into()),
    }
}

fn replacement_positions(mut channels: Vec<(ChannelId, u16)>, old: ChannelId, new: ChannelId) -> Vec<(ChannelId, u64)> {
    // Preserve relative order even if Discord reported equal positions: replacing
    // only the position would use the new snowflake to break ties differently.
    channels.sort_by_key(|(id, position)| (*position, *id));
    channels
        .into_iter()
        .enumerate()
        .map(|(position, (id, _))| (if id == old { new } else { id }, position as u64))
        .collect()
}

fn replace_routes(
    routes: &HashMap<ChannelId, ChannelId>,
    old: ChannelId,
    new: ChannelId,
) -> HashMap<ChannelId, ChannelId> {
    let mut next = routes.clone();
    for current in next.values_mut() {
        if *current == old {
            *current = new;
        }
    }
    next.insert(old, new);
    next
}

async fn reset(ctx: &Context, command: &CommandInteraction) -> Result<(), String> {
    require_permission(command, Permissions::MANAGE_CHANNELS)?;
    if !command.app_permissions.is_some_and(|permissions| {
        permissions.contains(Permissions::MANAGE_CHANNELS | Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES)
    }) {
        return Err("The bot needs View Channel, Send Messages and Manage Channels permissions.".into());
    }
    command.defer_ephemeral(&ctx.http).await.map_err(|e| e.to_string())?;
    let guild = command.guild_id.ok_or("This command requires a server text channel")?;
    let mut routes = ROUTES.write().await;
    let channel = command
        .channel_id
        .to_channel(&ctx.http)
        .await
        .map_err(|e| e.to_string())?
        .guild()
        .ok_or("This command requires a server text channel")?;
    if channel.guild_id != guild || channel.kind != ChannelType::Text {
        return Err("Only ordinary server text channels can be reset.".into());
    }
    let positions = guild
        .channels(&ctx.http)
        .await
        .map_err(|e| e.to_string())?
        .values()
        .filter(|channel| {
            matches!(
                channel.kind,
                ChannelType::Text | ChannelType::News | ChannelType::Forum
            )
        })
        .map(|channel| (channel.id, channel.position))
        .collect::<Vec<_>>();
    let mut builder = CreateChannel::new(&channel.name)
        .kind(ChannelType::Text)
        .permissions(channel.permission_overwrites.clone())
        .position(channel.position)
        .nsfw(channel.nsfw)
        .rate_limit_per_user(channel.rate_limit_per_user.unwrap_or(0))
        .audit_log_reason("Flagship clear-chat without count requested by a server manager");
    if let Some(category) = channel.parent_id {
        builder = builder.category(category);
    }
    if let Some(topic) = &channel.topic {
        builder = builder.topic(topic);
    }
    if let Some(duration) = channel.default_auto_archive_duration {
        builder = builder.default_auto_archive_duration(duration);
    }
    let replacement = guild
        .create_channel(&ctx.http, builder)
        .await
        .map_err(|e| e.to_string())?;
    let next = replace_routes(&routes, channel.id, replacement.id);
    let persist = async {
        let mut tx = Database::pool().begin().await.map_err(|e| e.to_string())?;
        for (original, current) in &next {
            sqlx::query("INSERT INTO discord_channel_routes (guild_id,original_channel_id,current_channel_id) VALUES ($1,$2,$3) ON CONFLICT (guild_id,original_channel_id) DO UPDATE SET current_channel_id=EXCLUDED.current_channel_id")
                .bind(guild.to_string()).bind(original.to_string()).bind(current.to_string()).execute(&mut *tx).await.map_err(|e| e.to_string())?;
        }
        tx.commit().await.map_err(|e| e.to_string())
    }.await;
    if let Err(error) = persist {
        // No history is removed when saving the replacement fails.
        let cleanup = replacement.delete(&ctx.http).await;
        return Err(format!(
            "Reset stopped before deleting history: {error}. Replacement cleanup: {}",
            if cleanup.is_ok() {
                "completed"
            } else {
                "failed; remove the duplicate channel manually"
            }
        ));
    }
    *routes = next;
    // Persist and publish the new destination before deleting the old channel.
    // If Discord rejects deletion, notifications still have a valid destination.
    let _ = command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content(format!(
                "Replacement ready: <#{}>. Permanently deleting this channel's history now.",
                replacement.id
            )),
        )
        .await;

    if let Err(error) = channel.delete(&ctx.http).await {
        return Err(format!(
            "Replacement <#{}> is active and saved, but the old channel was not deleted: {error}. Remove the old channel manually.",
            replacement.id
        ));
    }

    let order = replacement_positions(positions, channel.id, replacement.id);
    if let Err(error) = guild.reorder_channels(&ctx.http, order).await {
        tracing::warn!(%error, channel_id = %replacement.id, "Chat cleared but channel order could not be restored");
        let content = "Chat cleared and Flagship notifications updated, but Discord rejected restoring the channel order. Move this channel back to its previous position manually.".to_string();
        let _ = replacement.id.send_message(&ctx.http, CreateMessage::new().content(content)).await;
    }

    Ok(())
}
