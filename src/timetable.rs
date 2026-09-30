use chrono::{DateTime, FixedOffset, NaiveDate};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    Room(String),
    Online,
    Unspecified,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lesson {
    pub id: u64,
    pub subject: String,
    pub group: String,
    pub teacher: String,
    pub location: Location,
    pub start: DateTime<FixedOffset>,
    pub end: DateTime<FixedOffset>,
    pub lunch: Option<String>,
    pub notes: Vec<String>,
}

impl Lesson {
    pub fn minutes(&self) -> i64 {
        (self.end - self.start).num_minutes()
    }
}

/// All lessons of a group, kept sorted by start time (then subject).
#[derive(Debug, Clone, Default)]
pub struct Timetable {
    lessons: Vec<Lesson>,
}

/// Lessons that start at the same moment (parallel groups, electives).
#[derive(Debug)]
pub struct Slot<'a> {
    pub start: DateTime<FixedOffset>,
    pub lessons: Vec<&'a Lesson>,
}

impl Timetable {
    pub fn new(mut lessons: Vec<Lesson>) -> Self {
        lessons.sort_by(|a, b| {
            a.start
                .cmp(&b.start)
                .then_with(|| a.subject.cmp(&b.subject))
        });
        Timetable { lessons }
    }

    pub fn len(&self) -> usize {
        self.lessons.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lessons.is_empty()
    }

    /// Lessons that start on `date`, judged by each lesson's own local offset.
    pub fn lessons_on(&self, date: NaiveDate) -> Vec<&Lesson> {
        self.lessons
            .iter()
            .filter(|lesson| lesson.start.date_naive() == date)
            .collect()
    }

    /// The same lessons, grouped by start time.
    pub fn slots_on(&self, date: NaiveDate) -> Vec<Slot<'_>> {
        let mut slots: Vec<Slot<'_>> = Vec::new();
        for lesson in self.lessons_on(date) {
            match slots.last_mut() {
                Some(slot) if slot.start == lesson.start => slot.lessons.push(lesson),
                _ => slots.push(Slot {
                    start: lesson.start,
                    lessons: vec![lesson],
                }),
            }
        }
        slots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lesson(id: u64, subject: &str, start: &str, end: &str) -> Lesson {
        Lesson {
            id,
            subject: subject.to_string(),
            group: "ITA25".to_string(),
            teacher: "T".to_string(),
            location: Location::Unspecified,
            start: DateTime::parse_from_rfc3339(start).unwrap(),
            end: DateTime::parse_from_rfc3339(end).unwrap(),
            lunch: None,
            notes: vec![],
        }
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn new_sorts_by_start_then_subject() {
        let t = Timetable::new(vec![
            lesson(1, "B", "2026-10-02T10:15:00+03:00", "2026-10-02T11:45:00+03:00"),
            lesson(2, "A", "2026-10-02T10:15:00+03:00", "2026-10-02T11:45:00+03:00"),
            lesson(3, "Z", "2026-10-02T08:30:00+03:00", "2026-10-02T10:00:00+03:00"),
        ]);
        let ids: Vec<u64> = t.lessons_on(d(2026, 10, 2)).iter().map(|l| l.id).collect();
        assert_eq!(ids, vec![3, 2, 1]);
    }

    #[test]
    fn lessons_on_uses_the_lessons_own_local_date() {
        // 00:30 on the 3rd in +03:00 is still the 2nd in UTC; the local date must win.
        let t = Timetable::new(vec![
            lesson(1, "Late", "2026-10-02T23:30:00+03:00", "2026-10-03T00:15:00+03:00"),
            lesson(2, "Early", "2026-10-03T00:30:00+03:00", "2026-10-03T01:15:00+03:00"),
        ]);
        assert_eq!(t.lessons_on(d(2026, 10, 2)).len(), 1);
        assert_eq!(t.lessons_on(d(2026, 10, 3)).len(), 1);
        assert_eq!(t.lessons_on(d(2026, 10, 3))[0].id, 2);
    }

    #[test]
    fn empty_day_has_no_lessons_or_slots() {
        let t = Timetable::new(vec![lesson(
            1, "A", "2026-10-02T10:15:00+03:00", "2026-10-02T11:45:00+03:00",
        )]);
        assert!(t.lessons_on(d(2026, 10, 5)).is_empty());
        assert!(t.slots_on(d(2026, 10, 5)).is_empty());
        assert!(Timetable::default().is_empty());
    }

    #[test]
    fn slots_group_parallel_lessons() {
        let t = Timetable::new(vec![
            lesson(1, "French", "2026-10-02T10:15:00+03:00", "2026-10-02T11:45:00+03:00"),
            lesson(2, "German", "2026-10-02T10:15:00+03:00", "2026-10-02T11:45:00+03:00"),
            lesson(3, "SQL", "2026-10-02T11:55:00+03:00", "2026-10-02T14:00:00+03:00"),
            lesson(4, "Literature", "2026-10-02T08:30:00+03:00", "2026-10-02T10:00:00+03:00"),
        ]);
        let slots = t.slots_on(d(2026, 10, 2));
        let sizes: Vec<usize> = slots.iter().map(|s| s.lessons.len()).collect();
        assert_eq!(sizes, vec![1, 2, 1]);
        assert_eq!(slots[1].lessons[0].subject, "French");
        assert_eq!(slots[1].lessons[1].subject, "German");
    }
}
