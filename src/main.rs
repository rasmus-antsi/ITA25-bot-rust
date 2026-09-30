use poise::serenity_prelude as serenity;

mod scraper;
mod timetable;
mod dates;

struct Data {
    uptime: std::time::Instant,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(slash_command)]
async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    let gateway_latency = ctx.ping().await;
    let mut message = format!("Pong!, {}", gateway_latency.as_millis());

    let duration = std::time::Instant::now();
    let response = ctx.say(&message).await?;

    message = format!(
        "Pong! Gateway: {}ms | Round trip: {}ms",
        gateway_latency.as_millis(),
        duration.elapsed().as_millis()
    );
    response
        .edit(ctx, poise::CreateReply::default().content(message))
        .await?;

    Ok(())
}

#[poise::command(slash_command)]
async fn uptime(ctx: Context<'_>) -> Result<(), Error> {
    let uptime = ctx.data().uptime.elapsed().as_secs();

    let seconds = uptime % 60;
    let minutes = uptime / 60 % 60;
    let hours = uptime / 3600 % 24;
    let days = uptime / 86400;

    let message = format!(
        "I've been up for {}d {}h {}m {}s",
        days, hours, minutes, seconds
    );
    ctx.say(message).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");
    let intents = serenity::GatewayIntents::non_privileged();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![ping(), uptime()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    uptime: std::time::Instant::now(),
                })
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}
