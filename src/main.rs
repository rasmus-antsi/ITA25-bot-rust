use poise::serenity_prelude as serenity;

mod commands;
mod dates;
mod render;
mod schedule;
mod scraper;
mod timetable;

use schedule::{Schedule, Siseveeb};
use std::time::{Duration, Instant};

const ITA25_GROUP_ID: u32 = 2078;
const GROUP_NAME: &str = "ITA25";
const CACHE_TTL: Duration = Duration::from_secs(30 * 60);

struct Data {
    uptime: Instant,
    schedule: Schedule,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");
    let intents = serenity::GatewayIntents::non_privileged();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::ping(),
                commands::uptime(),
                commands::today(),
                commands::tomorrow(),
                commands::date(),
            ],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    uptime: Instant::now(),
                    schedule: Schedule::new(Siseveeb::new(ITA25_GROUP_ID)?, CACHE_TTL),
                })
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}
