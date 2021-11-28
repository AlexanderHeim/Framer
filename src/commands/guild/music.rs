use std::{collections::{HashMap, VecDeque}, sync::Arc};

use serenity::{async_trait, client::Context, model::{channel::Message, id::GuildId}, prelude::{Mutex, TypeMapKey}};
use songbird::{Call, Songbird, TrackEvent, create_player, events::EventData, input::Input, tracks::Track};

pub(super) async fn play(ctx: Context, msg: Message, cmd: Vec<String>) {
    if cmd.len() < 2 {
        match msg.channel_id.say(&ctx.http, "No youtube link given.").await {
            Ok(_) => return,
            Err(err) => {
                println!("Error sending message: 'pong' to channel_id: {:?}, Error: {:?}", msg.channel_id, err);
                return;
            },
        };
    }

    let songbird = songbird::get(&ctx).await.unwrap();
    {
        let mut data = ctx.data.write().await;
        let mp = data.get_mut::<MusicPlayer>().expect("No MusicPlayer in context data??");
        mp.play(songbird, &ctx, msg, &cmd[1]).await;
    }
}

pub struct MusicPlayer {
    connections: Arc<Mutex<HashMap<GuildId, Arc<Mutex<CallConnection>>>>>,
}

impl TypeMapKey for MusicPlayer {
    type Value = MusicPlayer;
}

impl MusicPlayer {
    pub fn new() -> Self {
        MusicPlayer {
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn play(&mut self, songbird: Arc<Songbird>, ctx: &Context, msg: Message, link: &str) {
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
                    match msg.channel_id.say(&ctx.http, "Please join a voice channel that I can follow.").await {
                        Ok(_) => (),
                        Err(err) => println!("Error sending message to channel_id: {:?}, Error: {:?}", msg.channel_id, err),
                    };
                    return;
                },
            };

        let source = match songbird::ytdl(link).await {
            Ok(source) => source,
            Err(error) => {
                match msg.channel_id.say(&ctx.http, &format!("Unable to load audio source! Error: {}", error)).await {
                    Ok(_) => (),
                    Err(err) => println!("Error sending message to channel_id: {:?}, Error: {:?}", msg.channel_id, err),
                };
                return;
            },
        };

        let (call_lock, res) = songbird.join(guild_id, channel_id).await;
        match res {
            Ok(_) => (),
            Err(error) => {
                match msg.channel_id.say(&ctx.http, &format!("Unable to join the voice channel! Error: {}", error)).await {
                    Ok(_) => (),
                    Err(err) => println!("Error sending message to channel_id: {:?}, Error: {:?}", msg.channel_id, err),
                };
                return;
            },
        }
        let mut connections = self.connections.lock().await;
        if !connections.contains_key(&guild_id) {
            connections.insert(guild_id, Arc::new(Mutex::new(CallConnection::new(call_lock))));
        }
        let connection_lock = match connections.get_mut(&guild_id) {
            Some(ok) => ok,
            None => {
                println!("Connection doesnt exist somehow??");
                return;
            },
        };
        let lock1 = connection_lock.clone();
        let mut connection = lock1.lock().await;

        if connection.queue.is_empty() {
            if connection.is_playing {
                connection.add_source(connection_lock.clone(), source);
            } else {
                connection.add_source(connection_lock.clone(), source);
                connection.start_play().await;
            }
        } else {
            connection.add_source(connection_lock.clone(), source);
        }

        match msg.channel_id.say(&ctx.http, "Added song to queue.").await {
            Ok(_) => (),
            Err(err) => println!("Error sending message to channel_id: {:?}, Error: {:?}", msg.channel_id, err),
        };
    }
}



struct CallConnection {
    call: Arc<Mutex<Call>>,
    is_playing: bool,
    queue: VecDeque<Track>,
}

impl CallConnection {
    pub fn new(call: Arc<Mutex<Call>>) -> Self {
        CallConnection {
            call,
            is_playing: false,
            queue: VecDeque::new(),
        }
    }

    pub fn add_source(&mut self, call_connection: Arc<Mutex<CallConnection>>, source: Input) {
        let mut track = create_player(source).0;
        let track_pos = track.position();

        track
            .events.as_mut().unwrap()
            .add_event(
            EventData::new(songbird::Event::Track(TrackEvent::End), SongEndNotifier { call_connection }),
            track_pos,
        );


        self.queue.push_back(track);
    }

    pub async fn start_play(&mut self) {
        let track = match self.queue.pop_front() {
            Some(t) => t,
            None => {
                println!("Tried to call start_play with empty queue!");
                return;
            }
        };
        let mut call = self.call.lock().await;
        self.is_playing = true;
        call.play_only(track);
    }
}

struct SongEndNotifier {
    call_connection: Arc<Mutex<CallConnection>>,
}

#[async_trait]
impl songbird::EventHandler for SongEndNotifier {
    async fn act(&self, _ctx: &songbird::EventContext<'_>) -> Option<songbird::Event> {
        let mut call_connection = self.call_connection.lock().await;

        if call_connection.queue.is_empty() {
            call_connection.is_playing = false;
        } else {
            call_connection.start_play().await;
        }
        None
    }
}