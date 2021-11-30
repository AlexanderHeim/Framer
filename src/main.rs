use std::sync::Arc;

use commands::process_command;
use serenity::{Client, client::{Context, EventHandler}, model::{channel::Message, prelude::Ready}, prelude::Mutex};
use songbird::{SerenityInit, SongbirdKey};

pub mod commands;
pub mod utils;

use crate::commands::guild::music::MusicPlayer;

#[tokio::main]
async fn main() {
    let token = std::fs::read_to_string("token.txt").expect("Couldn't get token from token.txt");
    let mut client = Client::builder(token)
        .event_handler(Handler)
        .register_songbird()
        .await.unwrap();

    {
        let mut data = client.data.write().await;
        let songbird = data.get::<SongbirdKey>().cloned().unwrap();
        data.insert::<MusicPlayer>(Arc::new(Mutex::new(MusicPlayer::new(songbird))));
    }

    if let Err(why) = client.start().await {
        println!("Error with client: {:#?}", why);
    }
}

//Event Handler
struct Handler;

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, _data: Ready) {
        println!("Bot ready!");
    }

    async fn message(&self, ctx: Context, msg: Message) {
        process_command(ctx, msg).await;
    }
}