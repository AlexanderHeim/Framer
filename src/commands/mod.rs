use serenity::{client::Context, model::channel::Message};
mod direct;
pub(crate) mod guild;

use crate::commands::direct::process_direct_command;
use crate::commands::guild::process_guild_command;

pub async fn process_command(ctx: Context, msg: Message) {
    if msg.guild_id.is_none() {
        process_direct_command(ctx, msg);
    } else if msg.content.starts_with("hs") {
        process_guild_command(ctx, msg).await;
    }
}

