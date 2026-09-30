use chrono::NaiveDate;
use poise::serenity_prelude as serenity;

use crate::render::{self, DayView};
use crate::{Context, Error, GROUP_NAME, dates};

fn to_embed(view: DayView) -> serenity::CreateEmbed {
    let mut embed = serenity::CreateEmbed::new()
        .title(view.title)
        .colour(serenity::Colour::new(view.tone.colour()))
        .footer(serenity::CreateEmbedFooter::new(view.footer))
        .fields(
            view.fields
                .into_iter()
                .map(|(name, value)| (name, value, false)),
        );
    if let Some(description) = view.description {
        embed = embed.description(description);
    }
    embed
}

async fn show_day(ctx: Context<'_>, date: NaiveDate) -> Result<(), Error> {
    // Discord wants an answer within 3 seconds; a cold fetch can take longer.
    ctx.defer().await?;

    let view = match ctx.data().schedule.week_of(date).await {
        Ok(week) => {
            let slots = week.timetable.slots_on(date);
            render::render_day(date, &slots, GROUP_NAME, week.age, week.stale)
        }
        Err(err) => {
            eprintln!("timetable fetch failed: {err}");
            render::error_view(date, GROUP_NAME)
        }
    };

    ctx.send(poise::CreateReply::default().embed(to_embed(view)))
        .await?;
    Ok(())
}

/// Today's lessons
#[poise::command(slash_command)]
pub async fn today(ctx: Context<'_>) -> Result<(), Error> {
    show_day(ctx, dates::today_in_tallinn()).await
}

/// The next school day's lessons (on Friday: Monday)
#[poise::command(slash_command)]
pub async fn tomorrow(ctx: Context<'_>) -> Result<(), Error> {
    show_day(ctx, dates::next_school_day(dates::today_in_tallinn())).await
}

/// Lessons on a given date
#[poise::command(slash_command)]
pub async fn date(
    ctx: Context<'_>,
    #[description = "02.10.2026, 2026-10-02 or just 2.10"] day: String,
) -> Result<(), Error> {
    let Some(date) = dates::parse_date(&day, dates::today_in_tallinn()) else {
        ctx.send(
            poise::CreateReply::default()
                .content("I couldn't read that date. Try `02.10.2026`, `2026-10-02` or `2.10`.")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    };
    show_day(ctx, date).await
}

#[poise::command(slash_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
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
pub async fn uptime(ctx: Context<'_>) -> Result<(), Error> {
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
