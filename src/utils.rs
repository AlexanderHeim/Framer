use std::{io::Error, string::FromUtf8Error};

use serde_json::Value;
use serenity::{http::Http, model::id::ChannelId};
use tokio::process::Command;

pub async fn send_message(channel_id: ChannelId, http: &Http, message: &str) {
    match channel_id.say(http, message).await {
        Ok(_) => (),
        Err(error) => println!("Error sending message: {}, Error: {:?}", message, error),
    }
}

pub async fn get_links_from_playlist(url: &str) -> Result<Vec<String>, LinksFromPlaylistError> {
    let mut cmd = Command::new("youtube-dl");
    cmd.arg("-j")
    .arg("--flat-playlist")
    .arg(url);

    let downloaded = match cmd.output().await {
        Ok(downloaded) => downloaded,
        Err(error) => return Err(LinksFromPlaylistError::IOError(error)),
    };

    let downloaded = match String::from_utf8(downloaded.stdout) {
        Ok(downloaded) => downloaded,
        Err(error) => return Err(LinksFromPlaylistError::Utf8Error(error)),
    };
    let downloaded = downloaded.trim_matches('\"');
    let entries: Vec<&str> = downloaded.split_inclusive('}').collect();
    let mut result: Vec<String> = Vec::new();

    for entry in entries {
        let json: Value = match serde_json::from_str(entry) {
            Ok(json) => json,
            Err(_) => continue,
        };
        let url = match &json["url"].as_str() {
            Some(url) => *url,
            None => continue,
        };

        result.push(format!("https://www.youtube.com/watch?v={}", url));
    }

    Ok(result)
}

#[derive(Debug)]
pub enum LinksFromPlaylistError {
    IOError(Error),
    Utf8Error(FromUtf8Error),
}