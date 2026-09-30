const FIXTURE: &str = include_str!("ita25_week_copy.html");

fn main() {
	let mut s = FIXTURE;

	let start = s.find("events:").unwrap();
	s = &s[start..];

	let end = s.find("eventRender").unwrap();
	s = s[..end].trim_ascii_end().strip_suffix("],").unwrap();

	let start = s.find("[").unwrap();
	s = &s[start + 1..];

	println!("{}", s)
}
