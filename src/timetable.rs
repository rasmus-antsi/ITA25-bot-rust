use chrono::{DateTime, FixedOffset};

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
