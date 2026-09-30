use std::fmt;

use chrono::{DateTime, FixedOffset};
use serde::Deserialize;

use crate::timetable::{Lesson, Location, Timetable};

#[derive(Debug, Deserialize)]
pub struct RawEvent {
    pub plan_id: String,
    pub title: String,
    pub start: DateTime<FixedOffset>,
    pub end: DateTime<FixedOffset>,
    #[serde(rename = "classNames")]
    pub class_names: String,
}

pub fn extract_events(mut html: &str) -> Option<&str> {
    let start = html.find("events:")?;
    html = &html[start..];

    let end = html.find("eventRender")?;
    html = html[..end].trim_ascii_end().strip_suffix(",")?;

    let start = html.find("[")?;
    html = &html[start..];

    Some(html)
}

pub fn parse_events(events: &str) -> Result<Vec<RawEvent>, json5::Error> {
    json5::from_str(events)
}

#[derive(Debug)]
pub enum ScrapeError {
    EventsNotFound,
    Parse(json5::Error),
    BadTitle(String),
    BadPlanId(String),
}

impl fmt::Display for ScrapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScrapeError::EventsNotFound => write!(f, "events array not found in page"),
            ScrapeError::Parse(e) => write!(f, "parse error: {e}"),
            ScrapeError::BadTitle(e) => write!(f, "bad title: {e}"),
            ScrapeError::BadPlanId(id) => write!(f, "bad plan id: {id}"),
        }
    }
}

impl std::error::Error for ScrapeError {}

impl From<json5::Error> for ScrapeError {
    fn from(value: json5::Error) -> Self {
        ScrapeError::Parse(value)
    }
}

pub fn parse_page(html: &str) -> Result<Vec<RawEvent>, ScrapeError> {
    let events = extract_events(html).ok_or(ScrapeError::EventsNotFound)?;
    Ok(parse_events(events)?)
}

#[derive(Debug, PartialEq)]
struct ParsedTitle {
    subject: String,
    group: String,
    teacher: String,
    location: Location,
    lunch: Option<String>,
    notes: Vec<String>,
}

fn parse_title(title: &str) -> Result<ParsedTitle, ScrapeError> {
    let bad = || ScrapeError::BadTitle(title.to_string());

    let (subject, rest) = split_span(title).ok_or_else(bad)?;

    let mut parts = rest.strip_prefix("; ").ok_or_else(bad)?.split("; ");

    let group = parts.next().ok_or_else(bad)?;
    let teacher = parts.next().ok_or_else(bad)?;

    let mut location = Location::Unspecified;
    let mut lunch = None;
    let mut notes = Vec::new();

    for part in parts {
        if part.starts_with("<span") {
            let (text, _) = split_span(part).ok_or_else(bad)?;
            match text.strip_prefix("Söömine:") {
                Some(range) => lunch = Some(range.trim().to_string()),
                None => notes.push(text.to_string()),
            }
        } else if location == Location::Unspecified {
            location = if part == "Veebiõpe" {
                Location::Online
            } else {
                Location::Room(part.to_string())
            };
        } else {
            notes.push(part.to_string());
        }
    }

    Ok(ParsedTitle {
        subject: subject.to_string(),
        group: group.to_string(),
        teacher: teacher.to_string(),
        location,
        lunch,
        notes,
    })
}

fn split_span(s: &str) -> Option<(&str, &str)> {
    let (_, after_open) = s.split_once('>')?;
    let (text, rest) = after_open.split_once("</span>")?;
    Some((text.trim_ascii(), rest))
}

impl TryFrom<RawEvent> for Lesson {
    type Error = ScrapeError;

    fn try_from(raw: RawEvent) -> Result<Self, Self::Error> {
        let parsed = parse_title(&raw.title)?;
        let id = raw
            .plan_id
            .parse::<u64>()
            .map_err(|_| ScrapeError::BadPlanId(raw.plan_id))?;

        Ok(Lesson {
            id,
            subject: parsed.subject,
            group: parsed.group,
            teacher: parsed.teacher,
            location: parsed.location,
            start: raw.start,
            end: raw.end,
            lunch: parsed.lunch,
            notes: parsed.notes,
        })
    }
}

pub fn parse_lessons(html: &str) -> Result<Vec<Lesson>, ScrapeError> {
    parse_page(html)?.into_iter().map(Lesson::try_from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = include_str!("../tests/fixtures/ita25_week.html");

    #[test]
    fn finds_events() {
        let events = extract_events(FIXTURE).expect("markers not found");
        assert!(events.starts_with("[{plan_id"));
        assert!(events.ends_with("}]"));
        assert_eq!(events.matches("plan_id:").count(), 20);
    }

    #[test]
    fn missing_event_is_none() {
        assert_eq!(extract_events("<html></html>"), None);
    }

    #[test]
    fn wrong_order_is_none() {
        assert_eq!(extract_events("eventRender events: [{}],"), None);
    }

    use chrono::Timelike;

    fn fixture_events() -> Vec<RawEvent> {
        let raw = extract_events(FIXTURE).expect("markers not found");
        parse_events(raw).expect("fixture should parse")
    }

    #[test]
    fn parses_all_events_in_fixture() {
        assert_eq!(fixture_events().len(), 20);
    }

    #[test]
    fn first_event_has_expected_fields() {
        let events = fixture_events();
        let first = &events[0];
        assert_eq!(first.plan_id, "26218283");
        assert_eq!(first.start.hour(), 10);
        assert_eq!(first.start.minute(), 15);
        assert_eq!(first.start.offset().local_minus_utc(), 3 * 3600);
        assert_eq!(first.end.hour(), 11);
        assert_eq!(first.end.minute(), 45);
        assert_eq!(first.class_names, "");
    }

    #[test]
    fn unescapes_quotes_in_title() {
        let events = fixture_events();
        assert!(events[0].title.starts_with("<span class=\"entry_subjects"));
        assert!(!events[0].title.contains('\\'));
    }

    #[test]
    fn keeps_estonian_letters() {
        let events = fixture_events();
        let ev = events
            .iter()
            .find(|e| e.plan_id == "26219195")
            .expect("event 26219195 missing");
        assert!(ev.title.contains("Kirjanduse põhiliigid I"));
        assert!(ev.title.contains("Kätlin"));
    }

    #[test]
    fn escaped_apostrophe_and_winter_offset() {
        let src = r#"[{plan_id:'1',title:'It\'s here',start:'2026-10-25T10:00:00+02:00',end:'2026-10-25T11:00:00+02:00',classNames:''}]"#;
        let events = parse_events(src).unwrap();
        assert_eq!(events[0].title, "It's here");
        assert_eq!(events[0].start.offset().local_minus_utc(), 2 * 3600);
    }

    #[test]
    fn empty_array_is_ok_and_empty() {
        let events = parse_events("[]").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn broken_input_is_err() {
        assert!(parse_events("[{plan_id:'1',").is_err());
    }

    #[test]
    fn missing_field_is_err() {
        assert!(parse_events("[{plan_id:'1'}]").is_err());
    }

    #[test]
    fn bad_date_is_err() {
        let src = "[{plan_id:'1',title:'x',start:'not a date',end:'2026-10-25T11:00:00+02:00',classNames:''}]";
        assert!(parse_events(src).is_err());
    }

    #[test]
    fn parse_page_reads_fixture() {
        assert_eq!(parse_page(FIXTURE).unwrap().len(), 20);
    }

    #[test]
    fn parse_page_without_markers() {
        assert!(matches!(
            parse_page("<html></html>"),
            Err(ScrapeError::EventsNotFound)
        ));
    }

    #[test]
    fn parse_page_with_broken_array() {
        let html = "events: [{plan_id:'1', eventRender";
        assert!(matches!(parse_page(html), Err(ScrapeError::Parse(_))));
    }

    #[test]
    fn error_messages() {
        assert_eq!(
            ScrapeError::EventsNotFound.to_string(),
            "events array not found in page"
        );
        let e = parse_page("events: [{plan_id:'1', eventRender").unwrap_err();
        assert!(e.to_string().starts_with("parse error:"));
    }

    #[test]
    fn scrape_error_fits_in_boxed_error() {
        let b: Box<dyn std::error::Error + Send + Sync> = Box::new(ScrapeError::EventsNotFound);
        assert_eq!(b.to_string(), "events array not found in page");
    }

    #[test]
    fn split_span_with_trailing_text() {
        assert_eq!(
            split_span(r#"<span class="a">Hi </span>; rest"#),
            Some(("Hi", "; rest"))
        );
    }

    #[test]
    fn split_span_at_end_of_string() {
        assert_eq!(split_span(r#"<span class="a">Hi</span>"#), Some(("Hi", "")));
    }

    #[test]
    fn split_span_without_tags_is_none() {
        assert_eq!(split_span("no tags"), None);
    }

    #[test]
    fn split_span_without_attributes() {
        assert_eq!(split_span("<span>x</span>"), Some(("x", "")));
    }

    #[test]
    fn split_span_lunch_label() {
        assert_eq!(
            split_span(r#"<span class="label label-warning">Söömine: 12:30-13:00</span>"#),
            Some(("Söömine: 12:30-13:00", ""))
        );
    }

    fn subj(name: &str) -> String {
        format!(r#"<span class="entry_subjects bold font-bold">{name} </span>"#)
    }

    fn raw(plan_id: &str, title: &str) -> RawEvent {
        RawEvent {
            plan_id: plan_id.to_string(),
            title: title.to_string(),
            start: DateTime::parse_from_rfc3339("2026-10-02T10:15:00+03:00").unwrap(),
            end: DateTime::parse_from_rfc3339("2026-10-02T11:45:00+03:00").unwrap(),
            class_names: String::new(),
        }
    }

    #[test]
    fn converts_fixture_to_lessons() {
        assert_eq!(parse_lessons(FIXTURE).unwrap().len(), 20);
    }

    #[test]
    fn lesson_fields_come_through() {
        let lessons = parse_lessons(FIXTURE).unwrap();
        let l = lessons.iter().find(|l| l.id == 26219185).expect("lesson missing");
        assert_eq!(l.subject, "SQL keel");
        assert_eq!(l.group, "ITA25");
        assert_eq!(l.teacher, "Evely Vutt");
        assert_eq!(l.location, Location::Room("KPL - A406".to_string()));
        assert_eq!(l.lunch.as_deref(), Some("12:30-13:00"));
        assert_eq!(l.start.hour(), 11);
        assert_eq!(l.start.minute(), 55);
        assert_eq!(l.minutes(), 125);
    }

    #[test]
    fn lesson_minutes() {
        let l = Lesson::try_from(raw("1", &format!("{}; ITA25; T", subj("X")))).unwrap();
        assert_eq!(l.minutes(), 90);
    }

    #[test]
    fn non_numeric_plan_id_is_an_error() {
        let e = Lesson::try_from(raw("abc", &format!("{}; ITA25; T", subj("X")))).unwrap_err();
        assert!(matches!(e, ScrapeError::BadPlanId(ref id) if id == "abc"));
    }

    #[test]
    fn bad_title_is_an_error() {
        let e = Lesson::try_from(raw("1", "junk")).unwrap_err();
        assert!(matches!(e, ScrapeError::BadTitle(_)));
    }

    #[test]
    fn one_bad_event_fails_the_whole_page() {
        let html = r#"events: [{plan_id:'1',title:'<span class="a">A </span>; G; T',start:'2026-10-02T10:15:00+03:00',end:'2026-10-02T11:45:00+03:00',classNames:''},{plan_id:'2',title:'junk',start:'2026-10-02T10:15:00+03:00',end:'2026-10-02T11:45:00+03:00',classNames:''}],
            eventRender"#;
        assert_eq!(parse_page(html).unwrap().len(), 2);
        assert!(matches!(parse_lessons(html), Err(ScrapeError::BadTitle(_))));
    }

    #[test]
    fn fixture_friday_has_parallel_slots() {
        let timetable = Timetable::new(parse_lessons(FIXTURE).unwrap());
        let friday = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        let slots = timetable.slots_on(friday);

        let sizes: Vec<usize> = slots.iter().map(|s| s.lessons.len()).collect();
        assert_eq!(sizes, vec![1, 4, 1]);

        let parallel: Vec<&str> = slots[1].lessons.iter().map(|l| l.subject.as_str()).collect();
        assert_eq!(parallel, vec!["Prantsuse keel", "Saksa keel", "Soome keel", "Stereomeetria I"]);
    }

    #[test]
    fn fixture_has_lessons_every_weekday() {
        let timetable = Timetable::new(parse_lessons(FIXTURE).unwrap());
        assert_eq!(timetable.len(), 20);
        for day in 28..=30 {
            let date = chrono::NaiveDate::from_ymd_opt(2026, 9, day).unwrap();
            assert!(!timetable.lessons_on(date).is_empty(), "no lessons on {date}");
        }
    }
}
