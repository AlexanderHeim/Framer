use serenity::{client::Context, model::channel::Message};

use self::{music::play, ping::ping};

mod ping;
pub(crate) mod music;

pub(crate) async fn process_guild_command(ctx: Context, msg: Message) {
    let mut cmd: Vec<String> = msg.content
                                    .strip_prefix("hs").unwrap()
                                    .trim()
                                    .split(' ')
                                    .map(|x| x.to_string())
                                    .collect();

    if !cmd.is_empty() {
        cmd[0] = cmd[0].to_lowercase();

        match cmd[0].as_str() {
            "ping" => ping(&ctx, msg).await,
            "play" => play(ctx, msg, cmd).await,
            _ => (),
        }
    }
}