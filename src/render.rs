use std::time::Duration;

use chrono::{DateTime, FixedOffset, NaiveDate};

use crate::timetable::{Lesson, Location, Slot};

/// Discord allows 1024 characters per embed field; keep some headroom.
const FIELD_LIMIT: usize = 1000;

/// What the embed should feel like: picks its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Normal,
    Empty,
    Stale,
    Error,
}

impl Tone {
    pub fn colour(self) -> u32 {
        match self {
            Tone::Normal => 0x3B82F6,
            Tone::Empty => 0x95A5A6,
            Tone::Stale => 0xF59E0B,
            Tone::Error => 0xEF4444,
        }
    }
}

/// A day, ready to be turned into an embed. Plain strings only, so it can be tested.
#[derive(Debug, PartialEq)]
pub struct DayView {
    pub title: String,
    pub description: Option<String>,
    /// (time range, lesson lines)
    pub fields: Vec<(String, String)>,
    pub footer: String,
    pub tone: Tone,
}

pub fn render_day(
    date: NaiveDate,
    slots: &[Slot<'_>],
    group: &str,
    age: Duration,
    stale: bool,
) -> DayView {
    let fields: Vec<(String, String)> = slots
        .iter()
        .filter_map(|slot| {
            let slot_end = slot.lessons.first()?.end;
            let text = slot
                .lessons
                .iter()
                .map(|lesson| lesson_text(lesson, slot_end))
                .collect::<Vec<_>>()
                .join("\n");
            Some((slot_heading(slot.start, slot_end), fit(&text)))
        })
        .collect();

    let tone = if stale {
        Tone::Stale
    } else if fields.is_empty() {
        Tone::Empty
    } else {
        Tone::Normal
    };
    let description = fields.is_empty().then(|| "No lessons 🎉".to_string());
    let footer = if stale {
        format!("⚠ Couldn't refresh · last updated {}", format_age(age))
    } else {
        format!("{group} · updated {}", format_age(age))
    };

    DayView {
        title: day_title(date),
        description,
        fields,
        footer,
        tone,
    }
}

pub fn error_view(date: NaiveDate, group: &str) -> DayView {
    DayView {
        title: day_title(date),
        description: Some(
            "Couldn't load the timetable right now. Try again in a minute.".to_string(),
        ),
        fields: Vec::new(),
        footer: group.to_string(),
        tone: Tone::Error,
    }
}

pub fn day_title(date: NaiveDate) -> String {
    date.format("%A, %-d %B %Y").to_string()
}

pub fn format_age(age: Duration) -> String {
    let minutes = age.as_secs() / 60;
    match minutes {
        0 => "just now".to_string(),
        1..=59 => format!("{minutes} min ago"),
        _ => format!("{} h ago", minutes / 60),
    }
}

fn slot_heading(start: DateTime<FixedOffset>, end: DateTime<FixedOffset>) -> String {
    format!("{} – {}", start.format("%H:%M"), end.format("%H:%M"))
}

fn lesson_text(lesson: &Lesson, slot_end: DateTime<FixedOffset>) -> String {
    let mut head = format!(
        "**{}** · {}",
        escape_markdown(&lesson.subject),
        escape_markdown(&lesson.teacher)
    );
    match &lesson.location {
        Location::Room(room) => {
            head.push_str(" · 📍 ");
            head.push_str(&escape_markdown(room));
        }
        Location::Online => head.push_str(" · 💻 Online"),
        Location::Unspecified => {}
    }
    if lesson.end != slot_end {
        head.push_str(&format!(" · until {}", lesson.end.format("%H:%M")));
    }

    let mut lines = vec![head];
    if let Some(range) = &lesson.lunch {
        lines.push(format!("🍴 Lunch {}", range.replace('-', "–")));
    }
    for note in &lesson.notes {
        lines.push(format!("ℹ️ {}", escape_markdown(note)));
    }
    lines.join("\n")
}

/// Stops Discord from turning `*`, `_` and friends in names into formatting.
fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '*' | '_' | '~' | '`' | '|') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Cuts text to the field limit (by characters, never in the middle of one).
fn fit(text: &str) -> String {
    if text.chars().count() <= FIELD_LIMIT {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(FIELD_LIMIT - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scraper::parse_lessons;
    use crate::timetable::Timetable;

    const FIXTURE: &str = include_str!("../tests/fixtures/ita25_week.html");

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn lesson(subject: &str, location: Location, start: &str, end: &str) -> Lesson {
        Lesson {
            id: 1,
            subject: subject.to_string(),
            group: "ITA25".to_string(),
            teacher: "Teacher".to_string(),
            location,
            start: DateTime::parse_from_rfc3339(start).unwrap(),
            end: DateTime::parse_from_rfc3339(end).unwrap(),
            lunch: None,
            notes: vec![],
        }
    }

    fn five_minutes() -> Duration {
        Duration::from_secs(5 * 60)
    }

    #[test]
    fn friday_of_the_fixture_renders_three_slots() {
        let timetable = Timetable::new(parse_lessons(FIXTURE).unwrap());
        let friday = d(2026, 10, 2);
        let view = render_day(
            friday,
            &timetable.slots_on(friday),
            "ITA25",
            five_minutes(),
            false,
        );

        assert_eq!(view.title, "Friday, 2 October 2026");
        assert_eq!(view.tone, Tone::Normal);
        assert_eq!(view.description, None);
        assert_eq!(view.footer, "ITA25 · updated 5 min ago");

        let headings: Vec<&str> = view.fields.iter().map(|(h, _)| h.as_str()).collect();
        assert_eq!(
            headings,
            vec!["08:30 – 10:00", "10:15 – 11:45", "11:55 – 14:00"]
        );

        // four parallel lessons, one line each
        assert_eq!(view.fields[1].1.lines().count(), 4);
        assert!(
            view.fields[1]
                .1
                .starts_with("**Prantsuse keel** · Ksenia Mets · 📍 KPL - B206")
        );

        // lunch and room on the long lesson
        assert_eq!(
            view.fields[2].1,
            "**SQL keel** · Evely Vutt · 📍 KPL - A406\n🍴 Lunch 12:30–13:00"
        );
    }

    #[test]
    fn empty_day_says_so() {
        let view = render_day(d(2026, 10, 3), &[], "ITA25", Duration::ZERO, false);
        assert_eq!(view.tone, Tone::Empty);
        assert_eq!(view.description.as_deref(), Some("No lessons 🎉"));
        assert!(view.fields.is_empty());
        assert_eq!(view.footer, "ITA25 · updated just now");
    }

    #[test]
    fn stale_data_is_flagged_even_on_an_empty_day() {
        let old = Duration::from_secs(45 * 60);
        let view = render_day(d(2026, 10, 3), &[], "ITA25", old, true);
        assert_eq!(view.tone, Tone::Stale);
        assert_eq!(view.footer, "⚠ Couldn't refresh · last updated 45 min ago");
    }

    #[test]
    fn online_and_unspecified_locations() {
        let online = lesson(
            "Mustrid",
            Location::Online,
            "2026-09-28T14:10:00+03:00",
            "2026-09-28T15:40:00+03:00",
        );
        let nowhere = lesson(
            "Mentortund",
            Location::Unspecified,
            "2026-09-28T14:10:00+03:00",
            "2026-09-28T15:40:00+03:00",
        );
        let end = online.end;
        assert_eq!(
            lesson_text(&online, end),
            "**Mustrid** · Teacher · 💻 Online"
        );
        assert_eq!(lesson_text(&nowhere, end), "**Mentortund** · Teacher");
    }

    #[test]
    fn lessons_ending_at_a_different_time_say_so() {
        let short = lesson(
            "Short",
            Location::Unspecified,
            "2026-10-02T10:15:00+03:00",
            "2026-10-02T11:30:00+03:00",
        );
        let slot_end = DateTime::parse_from_rfc3339("2026-10-02T11:45:00+03:00").unwrap();
        assert_eq!(
            lesson_text(&short, slot_end),
            "**Short** · Teacher · until 11:30"
        );
    }

    #[test]
    fn notes_get_their_own_line() {
        let mut l = lesson(
            "X",
            Location::Unspecified,
            "2026-10-02T10:15:00+03:00",
            "2026-10-02T11:45:00+03:00",
        );
        l.notes = vec!["Room changed".to_string()];
        let end = l.end;
        assert_eq!(lesson_text(&l, end), "**X** · Teacher\nℹ️ Room changed");
    }

    #[test]
    fn markdown_characters_are_escaped() {
        assert_eq!(escape_markdown("a*b_c"), "a\\*b\\_c");
        assert_eq!(escape_markdown("plain õ"), "plain õ");
        let l = lesson(
            "Git_hub *intro*",
            Location::Unspecified,
            "2026-10-02T10:15:00+03:00",
            "2026-10-02T11:45:00+03:00",
        );
        let end = l.end;
        assert!(lesson_text(&l, end).starts_with("**Git\\_hub \\*intro\\***"));
    }

    #[test]
    fn long_fields_are_cut_by_characters() {
        assert_eq!(fit("short"), "short");
        let long = "õ".repeat(2000);
        let cut = fit(&long);
        assert_eq!(cut.chars().count(), FIELD_LIMIT);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn ages_read_naturally() {
        assert_eq!(format_age(Duration::from_secs(0)), "just now");
        assert_eq!(format_age(Duration::from_secs(59)), "just now");
        assert_eq!(format_age(Duration::from_secs(60)), "1 min ago");
        assert_eq!(format_age(Duration::from_secs(59 * 60)), "59 min ago");
        assert_eq!(format_age(Duration::from_secs(3600)), "1 h ago");
        assert_eq!(format_age(Duration::from_secs(7300)), "2 h ago");
    }

    #[test]
    fn day_title_has_no_zero_padding() {
        assert_eq!(day_title(d(2026, 10, 2)), "Friday, 2 October 2026");
        assert_eq!(day_title(d(2026, 12, 24)), "Thursday, 24 December 2026");
    }

    #[test]
    fn error_view_is_red_and_friendly() {
        let view = error_view(d(2026, 10, 2), "ITA25");
        assert_eq!(view.tone, Tone::Error);
        assert!(view.description.unwrap().contains("Try again"));
    }
}
