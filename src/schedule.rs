use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use tokio::sync::Mutex;

use crate::dates::monday_of;
use crate::scraper::{ScrapeError, parse_lessons};
use crate::timetable::Timetable;

pub const TIMETABLE_URL: &str = "https://siseveeb.voco.ee/veebivormid/tunniplaan/tunniplaan";

#[derive(Debug)]
pub enum FetchError {
    Http(reqwest::Error),
    Scrape(ScrapeError),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::Http(e) => write!(f, "could not fetch the timetable: {e}"),
            FetchError::Scrape(e) => write!(f, "could not read the timetable page: {e}"),
        }
    }
}

impl std::error::Error for FetchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FetchError::Http(e) => Some(e),
            FetchError::Scrape(e) => Some(e),
        }
    }
}

impl From<reqwest::Error> for FetchError {
    fn from(value: reqwest::Error) -> Self {
        FetchError::Http(value)
    }
}

impl From<ScrapeError> for FetchError {
    fn from(value: ScrapeError) -> Self {
        FetchError::Scrape(value)
    }
}

/// Talks to the school's timetable page for one group.
pub struct Siseveeb {
    client: reqwest::Client,
    base_url: String,
    group_id: u32,
}

impl Siseveeb {
    pub fn new(group_id: u32) -> Result<Self, reqwest::Error> {
        Self::with_base_url(TIMETABLE_URL, group_id)
    }

    pub fn with_base_url(base_url: &str, group_id: u32) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("ita25-timetable-bot")
            .build()?;
        Ok(Siseveeb {
            client,
            base_url: base_url.to_string(),
            group_id,
        })
    }

    /// Fetches the week that contains `date`.
    pub async fn fetch_week(&self, date: NaiveDate) -> Result<Timetable, FetchError> {
        let html = self
            .client
            .get(&self.base_url)
            .query(&[
                ("oppegrupp", self.group_id.to_string()),
                ("nadal", nadal_param(date)),
            ])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(Timetable::new(parse_lessons(&html)?))
    }
}

/// The site wants `dd.mm.yyyy`.
fn nadal_param(date: NaiveDate) -> String {
    date.format("%d.%m.%Y").to_string()
}

/// One week of lessons, and whether it is older than we would like.
pub struct Week {
    pub timetable: Arc<Timetable>,
    pub stale: bool,
    pub age: Duration,
}

struct Entry {
    fetched_at: Instant,
    timetable: Arc<Timetable>,
}

/// Fetches weeks on demand and remembers them for `ttl`.
pub struct Schedule {
    source: Siseveeb,
    ttl: Duration,
    cache: Mutex<HashMap<NaiveDate, Entry>>,
}

impl Schedule {
    pub fn new(source: Siseveeb, ttl: Duration) -> Self {
        Schedule {
            source,
            ttl,
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub async fn week_of(&self, date: NaiveDate) -> Result<Week, FetchError> {
        let monday = monday_of(date);
        let mut cache = self.cache.lock().await;

        if let Some(entry) = cache.get(&monday) {
            if entry.fetched_at.elapsed() < self.ttl {
                return Ok(Week {
                    timetable: Arc::clone(&entry.timetable),
                    stale: false,
                    age: entry.fetched_at.elapsed(),
                });
            }
        }

        match self.source.fetch_week(monday).await {
            Ok(timetable) => {
                let timetable = Arc::new(timetable);
                cache.insert(
                    monday,
                    Entry {
                        fetched_at: Instant::now(),
                        timetable: Arc::clone(&timetable),
                    },
                );
                Ok(Week {
                    timetable,
                    stale: false,
                    age: Duration::ZERO,
                })
            }
            Err(err) => match cache.get(&monday) {
                Some(entry) => {
                    eprintln!("refresh of week {monday} failed, serving old data: {err}");
                    Ok(Week {
                        timetable: Arc::clone(&entry.timetable),
                        stale: true,
                        age: entry.fetched_at.elapsed(),
                    })
                }
                None => Err(err),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;

    const FIXTURE: &str = include_str!("../tests/fixtures/ita25_week.html");

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    /// Answers exactly one request, then stops listening.
    /// Returns the URL to call and a handle that yields the request text.
    async fn serve_once(status: &'static str, body: &'static str) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: text/html; charset=UTF-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            request
        });
        (format!("http://{addr}/tt"), handle)
    }

    #[test]
    fn nadal_param_is_zero_padded_day_month_year() {
        assert_eq!(nadal_param(d(2026, 9, 30)), "30.09.2026");
        assert_eq!(nadal_param(d(2026, 1, 5)), "05.01.2026");
    }

    #[tokio::test]
    async fn fetch_week_reads_and_parses_a_page() {
        let (url, server) = serve_once("200 OK", FIXTURE).await;
        let source = Siseveeb::with_base_url(&url, 2078).unwrap();

        let timetable = source.fetch_week(d(2026, 9, 30)).await.unwrap();
        assert_eq!(timetable.len(), 20);

        let request = server.await.unwrap();
        assert!(request.starts_with("GET /tt?"));
        assert!(request.contains("oppegrupp=2078"));
        assert!(request.contains("nadal=30.09.2026"));
    }

    #[tokio::test]
    async fn http_error_status_is_a_fetch_error() {
        let (url, _server) = serve_once("503 Service Unavailable", "down").await;
        let source = Siseveeb::with_base_url(&url, 2078).unwrap();
        let err = source.fetch_week(d(2026, 9, 30)).await.err().unwrap();
        assert!(matches!(err, FetchError::Http(_)));
    }

    #[tokio::test]
    async fn unreadable_page_is_a_scrape_error() {
        let (url, _server) = serve_once("200 OK", "<html>nothing here</html>").await;
        let source = Siseveeb::with_base_url(&url, 2078).unwrap();
        let err = source.fetch_week(d(2026, 9, 30)).await.err().unwrap();
        assert!(matches!(
            err,
            FetchError::Scrape(ScrapeError::EventsNotFound)
        ));
    }

    #[tokio::test]
    async fn same_week_is_fetched_once_within_ttl() {
        // The server answers a single request; a second fetch would fail,
        // and a failed refresh would show up as `stale == true`.
        let (url, _server) = serve_once("200 OK", FIXTURE).await;
        let schedule = Schedule::new(
            Siseveeb::with_base_url(&url, 2078).unwrap(),
            Duration::from_secs(60),
        );

        let wednesday = schedule.week_of(d(2026, 9, 30)).await.unwrap();
        let friday = schedule.week_of(d(2026, 10, 2)).await.unwrap();

        assert!(!wednesday.stale);
        assert!(!friday.stale);
        assert_eq!(friday.timetable.len(), 20);
        assert!(Arc::ptr_eq(&wednesday.timetable, &friday.timetable));
    }

    #[tokio::test]
    async fn failed_refresh_serves_old_data_marked_stale() {
        let (url, _server) = serve_once("200 OK", FIXTURE).await;
        let schedule = Schedule::new(
            Siseveeb::with_base_url(&url, 2078).unwrap(),
            Duration::ZERO, // always expired
        );

        let first = schedule.week_of(d(2026, 9, 30)).await.unwrap();
        assert!(!first.stale);

        let second = schedule.week_of(d(2026, 9, 30)).await.unwrap();
        assert!(second.stale);
        assert_eq!(second.timetable.len(), 20);
    }

    #[tokio::test]
    async fn failed_first_fetch_is_an_error() {
        let schedule = Schedule::new(
            Siseveeb::with_base_url("http://127.0.0.1:1/tt", 2078).unwrap(),
            Duration::from_secs(60),
        );
        let err = schedule.week_of(d(2026, 9, 30)).await.err().unwrap();
        assert!(matches!(err, FetchError::Http(_)));
    }

    #[test]
    fn fetch_error_messages_and_source() {
        use std::error::Error;
        let e = FetchError::Scrape(ScrapeError::EventsNotFound);
        assert_eq!(
            e.to_string(),
            "could not read the timetable page: events array not found in page"
        );
        assert!(e.source().is_some());
    }
}
