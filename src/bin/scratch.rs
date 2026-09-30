const FIXTURE: &str = include_str!("ita25_week_copy.html");

fn main() {
    let s = "<span class=\"entry_subjects bold font-bold\">Prantsuse keel </span>; ITA25; Ksenia Mets; KPL - B206";
    let i = s.find("</span>").unwrap();
    let outside = &s[i + 7..].trim_ascii();

    let mut lesson = &s[..i];
    let i = lesson.find("\">").unwrap();
    lesson = &lesson[i + 2..].trim_ascii();

    println!("{}", outside)
}
