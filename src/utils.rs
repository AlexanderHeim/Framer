use serenity::{http::Http, model::id::ChannelId};

pub async fn send_message(channel_id: ChannelId, http: &Http, message: &str) {
    match channel_id.say(http, message).await {
        Ok(_) => (),
        Err(error) => println!("Error sending message: {}, Error: {:?}", message, error),
    }
}