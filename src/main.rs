mod skipped_words;
use dotenvy::dotenv;
use serenity::async_trait;
use serenity::model::prelude::{ChannelId, Ready, UserId};
use serenity::prelude::*;
use std::collections::HashMap;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

struct Handler;

#[derive(Debug)]
struct UserInfo {
    username: String,
    messages: u32,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _: Ready) {
        let channels = env::var("CHANNELS").expect("CHANNELS var not found");
        for channel in channels.split(",") {
            count(&ctx, channel.parse().expect("Invalid CHANNELS variable")).await;
        }
        std::process::exit(0);
    }
}

async fn count(ctx: &Context, channel: u64) {
    let channel_id = ChannelId(channel);
    let mut messages = channel_id
        .messages(&ctx.http, |retriever| retriever.limit(100))
        .await
        .unwrap();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time has gone backwards")
        .as_secs();

    while i64::abs(now as i64 - &messages.last().unwrap().timestamp.unix_timestamp())
        <= 60 * 60 * 24
    {
        messages.extend(
            channel_id
                .messages(&ctx.http, |retriever| {
                    retriever.before(messages.last().unwrap().id).limit(100)
                })
                .await
                .unwrap(),
        );
    }

    messages = messages
        .into_iter()
        .filter(|message| i64::abs(now as i64 - message.timestamp.unix_timestamp()) <= 60 * 60 * 24)
        .collect();

    let mut info: HashMap<UserId, UserInfo> = HashMap::new();
    let mut word_counts: HashMap<String, u32> = HashMap::new();
    for message in &messages {
        info.entry(message.author.id)
            .or_insert(UserInfo {
                username: message.author.name.clone(),
                messages: 0,
            })
            .messages += 1;

        for word in message.content.split_whitespace() {
            if skipped_words::SKIPPED_WORDS.contains(&word) {
                continue;
            }
            *word_counts.entry(word.to_lowercase()).or_insert(0) += 1;
        }
    }

    let mut word_counts: Vec<(String, u32)> = Vec::from_iter(word_counts.into_iter());
    word_counts.sort_by_key(|x| x.1);

    let mut info: Vec<UserInfo> = info.into_values().collect();
    info.sort_by_key(|x| x.messages);

    let _ = channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Active members");
                e.color(0xe190de);
                e.footer(|f| {
                    f.text(format!(
                        "Total messages: {} | Unique chatters: {}",
                        messages.len(),
                        info.len()
                    ))
                });

                for (i, user) in info.iter().rev().enumerate() {
                    if i >= 9 {
                        break;
                    }
                    e.field(
                        format!("#{} {}", i + 1, user.username),
                        format!("{} messages", user.messages),
                        true,
                    );
                }

                let words: String = word_counts
                    .iter()
                    .take(8)
                    .map(|x| x.0.clone())
                    .collect::<Vec<String>>()
                    .join(", ");

                e.field("Most used words", words, false);
                e
            })
        })
        .await;
}

#[tokio::main]
async fn main() {
    let _ = dotenv();

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(env::var("TOKEN").expect("TOKEN var not found"), intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
