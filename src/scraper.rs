pub fn extract_events(mut html: &str) -> Option<&str> {
	let start = html.find("events:")?;
	html = &html[start..];

	let end = html.find("eventRender")?;
	html = html[..end].trim_ascii_end().strip_suffix("],")?;

	let start = html.find("[")?;
	html = &html[start + 1..];

	Some(html)
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = include_str!("../tests/fixtures/ita25_week.html");

    #[test]
    fn finds_events() {
    	let events = extract_events(FIXTURE).expect("markers not found");
        assert!(events.starts_with("{plan_id"));
        assert!(events.ends_with("}"));
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
}
