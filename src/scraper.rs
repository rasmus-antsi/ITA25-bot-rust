use chrono::{DateTime, FixedOffset};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RawEvent {
	pub plan_id: String,
	pub title: String,
	pub start: DateTime<FixedOffset>,
	pub end: DateTime<FixedOffset>,
	#[serde(rename="classNames")]
	pub class_names: String
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
}
