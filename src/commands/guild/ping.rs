use serenity::{client::Context, model::channel::Message};

pub(super) async fn ping(ctx: &Context, msg: Message) {
    match msg.channel_id.say(&ctx.http, "pong").await {
        Ok(_) => (),
        Err(err) => println!("Error sending message: 'pong' to channel_id: {:?}, Error: {:?}", msg.channel_id, err),
    };
}