const FIXTURE: &str = include_str!("../tests/fixtures/ita25_week.html");

pub fn extract_events(mut html: &str) -> Option<&str> {
	let start = html.find("events:")?;
	html = &html[start..];

	let end = html.find("eventRender")?;
	html = html[..end].trim_ascii_end().strip_suffix("],").unwrap();

	let start = html.find("[")?;
	html = &html[start + 1..];

	Some(html)
}
