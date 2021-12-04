use std::{collections::{HashMap, VecDeque}, sync::Arc};

use serenity::{async_trait, client::Context, http::{CacheHttp, Http}, model::{channel::Message, id::ChannelId}, prelude::{Mutex, TypeMapKey}};
use songbird::{Call, Songbird, id::GuildId, tracks::Track, input::Input, create_player};

use crate::utils::{send_message, get_links_from_playlist};

pub struct MusicPlayer {
    connections: HashMap<GuildId, Arc<Mutex<CallConnection>>>,
    songbird: Arc<Songbird>,
}

impl MusicPlayer {
    pub fn new(songbird: Arc<Songbird>) -> Self {
        MusicPlayer {
            connections: HashMap::new(),
            songbird,
        }
    }

    pub async fn play(&mut self, ctx: &Context, msg: Message, cmd: Vec<String>) {
        let http = ctx.http.clone();
        if cmd.len() < 2 {
            send_message(msg.channel_id, &ctx.http, "No resource url given.").await;
            return;
        }

        let guild_id = msg.guild_id.unwrap();
        let guild = match msg.guild(&ctx.cache).await {
            Some(guild) => guild,
            None => {
                println!("Error retrieving guild from cache!");
                return;
            },
        };

        let channel_id = match guild
            .voice_states.get(&msg.author.id)
            .and_then(|voice_state| voice_state.channel_id) {
                Some(id) => id,
                None => {
                    send_message(msg.channel_id, &ctx.http(), "Please join a voice channel I can follow.").await;
                    return;
                },
            };

        match self.connect(msg.guild_id.unwrap().into(), channel_id, http, msg.channel_id).await {
            Ok(_) => {},
            Err(_) => {
                send_message(msg.channel_id, &ctx.http(), "I can't join the channel.").await;
                return;}
        }

        let url = cmd[1].clone();
        let mut urls: Vec<String> = Vec::new();

        if url.contains("&list=") || url.contains("playlist") {
            let _urls = match get_links_from_playlist(&url).await {
                Ok(result) => result,
                Err(error) => {
                    send_message(msg.channel_id, &ctx.http(), "I was unable to load in the playlist.").await;
                    return;
                }
            };
            urls = _urls;
        } else {
            match songbird::ytdl(&url).await {
                Ok(_) => (),
                Err(_) => {
                    send_message(msg.channel_id, &ctx.http(), "I was unable to load the audio from the given url.").await;
                    return;
                }
            }
            urls.push(url);
        }

        let connection_lock = self.connections.get_mut(&(guild_id.into())).unwrap();
        let event_lock = connection_lock.clone();
        {
            let mut connection = connection_lock.lock().await;
            for u in urls {
                connection.queue_links.push_back(u);
            }
            if connection.current_tracks.is_empty() {
                connection.load_next_track(event_lock.clone()).await;
            }
            send_message(connection.callback_channel, &connection.http, "Added to queue.").await;
            if !connection.is_playing {
                connection.play_next_song(event_lock.clone()).await;
            }
        }

    }

    pub async fn skip(&mut self, ctx: &Context, msg: Message, cmd: Vec<String>) {
        let connection_lock = match self.connections.get(&msg.guild_id.unwrap().into()) {
            Some(connection) => connection,
            None => {
                send_message(msg.channel_id, &ctx.http(), "I don't seem to be connected to any voice channel.").await;
                return;
            },
        };
        
        {
            let event_lock = connection_lock.clone();
            let mut connection = connection_lock.lock().await;
            if !connection.is_playing {
                send_message(msg.channel_id, &ctx.http(), "I am currently not playing anything.").await;
                return;
            }
            connection.is_playing = false;
            connection.play_next_song(event_lock.clone()).await;
        }

        send_message(msg.channel_id, &ctx.http(), "Skipped current track.").await;
    }

    pub async fn clear(&mut self, ctx: &Context, msg: Message, cmd: Vec<String>) {
        let connection_lock = match self.connections.get(&msg.guild_id.unwrap().into()) {
            Some(connection) => connection,
            None => {
                send_message(msg.channel_id, &ctx.http(), "I don't seem to be connected to any voice channel.").await;
                return;
            },
        };
        {
            let mut connection = connection_lock.lock().await;
            connection.current_tracks.clear();
            connection.queue_links.clear();
            connection.is_playing = false;
            let mut call = connection.call.lock().await;
            call.stop();
        }
        send_message(msg.channel_id, &ctx.http(), "I cleared the queue and stopped playing.").await;
    }

    async fn connect(&mut self, guild_id: GuildId, channel_id: ChannelId, http: Arc<Http>, callback_channel: ChannelId) -> Result<(), ()>{
        let (call, join_error) = self.songbird.join(guild_id, channel_id).await;
        if join_error.is_err() {
            println!("Error joining channel: {:?} in guild: {:?}, Error: {:?}", channel_id, guild_id, join_error.unwrap());
            return Err(());
        }

        match self.connections.get_mut(&guild_id) {
            Some(connection) => {
                {
                    let mut connection = connection.lock().await;
                    connection.call = call;
                }
            },
            None => {
                self.connections.insert(guild_id, Arc::new(Mutex::new(CallConnection::new(call, callback_channel, guild_id, http))));
            },
        }

        return Ok(())
    }
}

impl TypeMapKey for MusicPlayer {
    type Value = Arc<Mutex<MusicPlayer>>;
}

pub struct CallConnection {
    call: Arc<Mutex<Call>>,
    callback_channel: ChannelId,
    callback_guild: GuildId,
    is_playing: bool,
    queue_links: VecDeque<String>,
    current_tracks: VecDeque<Track>,
    http: Arc<Http>,
}

impl CallConnection {
    pub fn new(call: Arc<Mutex<Call>>, callback_channel: ChannelId, callback_guild: GuildId, http: Arc<Http>) -> Self {
        CallConnection {
            call,
            callback_channel,
            callback_guild,
            is_playing: false,
            queue_links: VecDeque::new(),
            current_tracks: VecDeque::new(),
            http,
        }
    }

    pub async fn load_next_track(&mut self, call_connection: Arc<Mutex<CallConnection>>) {
        loop {
            let url = match self.queue_links.pop_front() {
                Some(url) => url,
                None => return,
            };
            let source = match songbird::ytdl(url).await {
                Ok(source) => source,
                Err(_) => continue,
            };
            self.add_source(source, call_connection);
            return
        }
    }

    pub async fn play_next_song(&mut self, call_connection: Arc<Mutex<CallConnection>>) {
        let track: Track = match self.current_tracks.pop_front() {
            Some(track) => {
                self.load_next_track(call_connection).await;
                track
            },
            None => {
                self.call.lock().await.stop();
                self.is_playing = false;
                return;
            }
        };

        /*
        let track = match self.queue.pop_front() {
            Some(track) => track,
            None => {
                let url = match self.queue_links.pop_front() {
                    Some(url) => url,
                    None => { 
                        self.call.lock().await.stop();
                        return
                    }
                };
                let source = match songbird::ytdl(&url).await {
                    Ok(source) => source,
                    Err(error) => {
                        self.play_next_song().await;
                        return;
                    }
                };
                create_player(source).0
            }
        };*/
        let title = track.handle.metadata().title.clone();
        let mut call = self.call.lock().await;
        call.play_only(track);
        self.is_playing = true;
        send_message(self.callback_channel, &self.http, &format!("Now playing: {}", title.unwrap())).await;
    }

    pub fn add_source(&mut self, source: Input, call_connection: Arc<Mutex<CallConnection>>) {
        let mut track = create_player(source).0;
        let track_pos = track.position();
        track
            .events.as_mut().unwrap()
            .add_event(
            songbird::events::EventData::new(
                songbird::Event::Track(songbird::TrackEvent::End),
                SongEndNotifier(call_connection)),
            track_pos,
        );


        self.current_tracks.push_back(track);
    }
}

pub struct SongEndNotifier(Arc<Mutex<CallConnection>>);

#[async_trait]
impl songbird::EventHandler for SongEndNotifier {
    async fn act(&self, _ctx: &songbird::EventContext<'_>) -> Option<songbird::Event> {
        let remote_lock = self.0.clone();
        let mut connection = self.0.lock().await;
        connection.is_playing = false;
        connection.play_next_song(remote_lock).await;
        None
    }
}