#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    Room(String),
    Online,
    Unspecified,
}
