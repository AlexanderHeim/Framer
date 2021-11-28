use serenity::{client::Context, model::channel::Message};

pub(crate) fn process_direct_command(_ctx: Context, msg: Message) {
    let _cmd: Vec<String> = msg.content
                                .trim()
                                .split(' ')
                                .map(|x| x.to_string())
                                .collect();

    
}