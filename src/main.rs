use std::env;

use datetime::DateTime;
use serenity::{
    all::{Context, EventHandler, GatewayIntents, Message, Ready},
    async_trait, Client,
};
use sqlite::{ConnectionThreadSafe, State};
extern crate dtt;
use dtt::*;

struct Handler;

#[derive(Clone)]
struct BreadPost {
    date: String,
    message_url: String,
}

fn get_all_posts(conn: ConnectionThreadSafe) -> Vec<BreadPost> {
    let query = "select * from bread_posts order by date desc";
    let mut statement = conn.prepare(query).unwrap();
    let mut results = vec![];
    while let Ok(State::Row) = statement.next() {
        let date = statement.read::<String, _>("date").unwrap();
        let message_url = statement.read::<String, _>("message_url").unwrap();
        results.push(BreadPost { date, message_url });
    }
    results
}

// BPPD = bread posts per day
fn calculate_bppd(posts: &Vec<BreadPost>) -> f32 {
    let last_post_date =
        DateTime::parse(posts[0].clone().date.as_str()).expect("Valid date string");
    let first_post_date =
        DateTime::parse(posts[posts.len() - 1].clone().date.as_str()).expect("Valid date string");
    let diff = first_post_date.duration_since(&last_post_date);
    let num_posts = f32::from(u16::try_from(posts.len()).unwrap());
    (num_posts / (diff.as_seconds_f32() / 60.0 / 60.0 / 24.0)).abs()
}

#[async_trait]
impl EventHandler for Handler {
    // Set a handler for the `message` event. This is called whenever a new message is received.
    //
    // Event handlers are dispatched through a threadpool, and so multiple events can be
    // dispatched simultaneously.
    async fn message(&self, ctx: Context, msg: Message) {

        let target_user_id = env::var("TARGET_USER").expect("Expected TARGET_USER to be in the environment");
        let target_channel_id = env::var("TARGET_CHANNEL").expect("Expected TARGET_CHANNEL to be in the environment");

        if msg.author.id.to_string() == target_user_id
            && msg.channel_id.to_string() == target_channel_id
            && !msg.attachments.is_empty()
            && msg.content.to_lowercase().contains("bread")
        {
            // bread post detected, save it in the db
            let conn = sqlite::Connection::open_thread_safe("./bread_prod.db").unwrap();
            let query = format!(
                "insert into bread_posts values ({}, '{}', '{}')",
                msg.id.to_string(),
                msg.link(),
                msg.timestamp.to_string()
            );
            conn.execute(query).unwrap();

            let all_posts = get_all_posts(conn);

            if let Err(why) = msg
                .channel_id
                .say(&ctx.http, format!("New bread post!\nThis is bread post number {}.\nCurrent BPPD is {}\nLink to previous post: {}", all_posts.len(), calculate_bppd(&all_posts), all_posts[1].message_url))
                .await
            {
                println!("Error sending message: {why:?}");
            }
        }
    }

    // Set a handler to be called on the `ready` event. This is called when a shard is booted, and
    // a READY payload is sent by Discord. This payload contains data like the current user's guild
    // Ids, current user data, private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN to be in the environment");
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Create a new instance of the Client, logging in as a bot. This will automatically prepend
    // your bot token with "Bot ", which is a requirement by Discord for bot users.
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform exponential backoff until
    // it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
