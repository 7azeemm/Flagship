use playwright_rs::{
    Browser, BrowserContext, BrowserContextOptions, ConnectOverCdpOptions, GotoOptions, LaunchOptions, Page, Playwright, Route, WaitUntil,
};
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, HeaderMap, HeaderValue};
use reqwest::{Client, ClientBuilder};
use scraper::{Html, Selector};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, OnceCell, Semaphore};
use tokio::time::{Instant, sleep, sleep_until};

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(30);
const CHALLENGE_TIMEOUT: Duration = Duration::from_secs(45);
const PROXIED_REQUEST_GAP: Duration = Duration::from_secs(2);
const RETAILER_PROXY: &str = "socks5h://127.0.0.1:40000";
const BROWSER_CDP_ENDPOINT: &str = "http://127.0.0.1:9224";
static WEB_CLIENT: OnceCell<WebClient> = OnceCell::const_new();

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WebClientType {
    HttpClient,
    Browser,
    ProxyHttpClient,
    PersistentBrowser,
}

impl WebClientType {
    pub fn is_browser(self) -> bool {
        matches!(self, Self::Browser | Self::PersistentBrowser)
    }
}

struct PersistentBrowserState {
    browser: Browser,
    context: BrowserContext,
}

struct RetailerSession {
    page: Option<Page>,
    next_request: Instant,
}

pub struct WebClient {
    pub http_client: Client,
    proxy_client: Client,
    playwright: Playwright,
    browser: Browser,
    persistent_browser: Mutex<Option<PersistentBrowserState>>,
    retailer_sessions: Mutex<HashMap<String, Arc<Mutex<RetailerSession>>>>,
    persistent_browser_slots: Semaphore,
}

impl WebClient {
    pub async fn init() {
        WEB_CLIENT.get_or_init(Self::initialize).await;
    }

    async fn initialize() -> Self {
        let http_client = Self::http_builder().build().expect("Failed to build HTTP client");
        let proxy_client = Self::http_builder()
            .proxy(reqwest::Proxy::all(RETAILER_PROXY).expect("Invalid retailer proxy constant"))
            .build()
            .expect("Failed to build retailer proxy client");
        let playwright = Playwright::launch().await.expect("Failed to launch Playwright");
        let browser = playwright
            .chromium()
            .launch_with_options(LaunchOptions::default().headless(true).args(vec![
                "--disable-gpu".into(),
                "--disable-dev-shm-usage".into(),
                "--disable-plugins".into(),
                "--disable-image-loading".into(),
                "--disable-extensions".into(),
                "--disable-default-apps".into(),
                "--no-service-autorun".into(),
                "--disable-sync".into(),
                "--metrics-recording-only".into(),
                "--disable-background-networking".into(),
            ]))
            .await
            .expect("Failed to launch chromium");
        tracing::info!("Retailer fetching initialized");
        Self {
            http_client,
            proxy_client,
            playwright,
            browser,
            persistent_browser: Mutex::new(None),
            retailer_sessions: Mutex::new(HashMap::new()),
            persistent_browser_slots: Semaphore::new(2),
        }
    }

    fn http_builder() -> ClientBuilder {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8"),
        );
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=0"));
        ClientBuilder::new()
            .user_agent(USER_AGENT)
            .default_headers(headers)
            .timeout(TIMEOUT)
    }

    pub fn get() -> &'static Self {
        WEB_CLIENT.get().expect("Scraper WebClient must be initialized before use")
    }

    pub async fn fetch(url: &str, client_type: &WebClientType, ready: &[&Selector]) -> Result<String, Box<dyn Error + Send + Sync>> {
        tracing::debug!(client = ?client_type, url, "Fetching retailer page");
        let client = Self::get();
        match client_type {
            WebClientType::HttpClient => Self::fetch_http(&client.http_client, url).await,
            WebClientType::Browser => {
                let context = client
                    .browser
                    .new_context_with_options(BrowserContextOptions::builder().user_agent(USER_AGENT.to_string()).build())
                    .await?;
                let result: Result<String, String> = match context.new_page().await {
                    Ok(page) => {
                        let _ = page
                            .route(
                                "**/*.{png,jpg,jpeg,gif,svg,webp}",
                                Box::new(|route: Route| Box::pin(async move { route.abort(None).await })),
                            )
                            .await;
                        let result = Self::fetch_browser_page(&page, url, &[], false).await.map_err(|e| e.to_string());
                        let _ = page.close().await;
                        result
                    }
                    Err(error) => Err(error.to_string()),
                };
                let _ = context.close().await;
                Ok(result?)
            }
            WebClientType::ProxyHttpClient | WebClientType::PersistentBrowser => {
                let host = reqwest::Url::parse(url)?.host_str().ok_or("Retailer URL has no host")?.to_string();
                let session = client
                    .retailer_sessions
                    .lock()
                    .await
                    .entry(host)
                    .or_insert_with(|| {
                        Arc::new(Mutex::new(RetailerSession {
                            page: None,
                            next_request: Instant::now(),
                        }))
                    })
                    .clone();
                let mut session = session.lock().await;
                sleep_until(session.next_request).await;
                let result = if *client_type == WebClientType::ProxyHttpClient {
                    Self::fetch_http(&client.proxy_client, url).await
                } else {
                    async {
                        let _slot = client.persistent_browser_slots.acquire().await?;
                        let context = client.persistent_context().await?;
                        if session.page.as_ref().is_none_or(Page::is_closed) {
                            session.page = Some(context.new_page().await?);
                        }
                        let result = Self::fetch_browser_page(session.page.as_ref().unwrap(), url, ready, true).await;
                        // Retire failed tabs; cookies stay in the persistent browser context.
                        if result.is_err() && let Some(page) = session.page.take() {
                            let _ = page.close().await;
                        }
                        result
                    }
                    .await
                };
                session.next_request = Instant::now() + PROXIED_REQUEST_GAP;
                result
            }
        }
    }

    async fn fetch_http(client: &Client, url: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let response = client.get(url).send().await?.error_for_status()?;
        let challenged = response.headers().get("cf-mitigated").is_some_and(|value| value == "challenge");
        let body = response.text().await?;
        if challenged || challenge_page(&body) {
            return Err(std::io::Error::other(format!("Retailer challenge/block fetching {url}")).into());
        }
        Ok(body)
    }

    async fn persistent_context(&self) -> Result<BrowserContext, Box<dyn Error + Send + Sync>> {
        let mut connection = self.persistent_browser.lock().await;
        if connection.as_ref().is_none_or(|state| !state.browser.is_connected()) {
            // An unavailable external browser must not stop the API from starting.
            let browser = self
                .playwright
                .chromium()
                .connect_over_cdp(BROWSER_CDP_ENDPOINT, ConnectOverCdpOptions::new().timeout(10_000.0))
                .await?;
            let context = browser
                .contexts()
                .into_iter()
                .next()
                .ok_or("CDP browser has no persistent context")?;
            *connection = Some(PersistentBrowserState { browser, context });
            tracing::info!("Connected to persistent retailer browser");
        }
        Ok(connection.as_ref().unwrap().context.clone())
    }

    async fn fetch_browser_page(
        page: &Page,
        url: &str,
        ready: &[&Selector],
        persistent: bool,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let response = page
            .goto(
                url,
                Some(GotoOptions::default().wait_until(WaitUntil::DomContentLoaded).timeout(TIMEOUT)),
            )
            .await?;

        let status = response.as_ref().map(|response| response.status());
        let challenged = response
            .as_ref()
            .is_some_and(|response| response.headers().get("cf-mitigated").is_some_and(|value| value == "challenge"));

        if !persistent {
            if let Some(status) = status.filter(|status| *status >= 400) {
                return Err(std::io::Error::other(format!("HTTP {status} fetching {url}")).into());
            }
            sleep(Duration::from_secs(3)).await;
            let body = page.content().await?;
            if challenge_page(&body) {
                return Err(std::io::Error::other(format!("Retailer challenge/block fetching {url}")).into());
            }
            return Ok(body);
        }

        let deadline = Instant::now() + CHALLENGE_TIMEOUT;
        let mut saw_challenge = challenged;
        loop {
            let body = match page.content().await {
                Ok(body) => body,
                // Cloudflare can replace the document between polling and serializing it.
                Err(error) if navigation_in_progress(&error.to_string()) && Instant::now() < deadline => {
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };

            let blocked = challenge_page(&body);
            saw_challenge |= blocked;
            if !blocked && status.is_some_and(|status| status >= 400) && !saw_challenge {
                return Err(std::io::Error::other(format!("HTTP {} fetching {url}", status.unwrap())).into());
            }
            if !blocked && page_ready(&body, ready) {
                tracing::debug!(url, initial_status = ?status, saw_challenge, "Retailer browser page ready");
                return Ok(body);
            }

            if Instant::now() >= deadline {
                return Err(std::io::Error::other(format!(
                    "Retailer page did not become ready within 45s (initial HTTP {status:?}, challenge={saw_challenge}): {url}"
                ))
                .into());
            }
            sleep(Duration::from_secs(1)).await;
        }
    }

    pub async fn cleanup() {
        if let Some(client) = WEB_CLIENT.get() {
            for session in client.retailer_sessions.lock().await.values() {
                if let Some(page) = session.lock().await.page.take() {
                    let _ = page.close().await;
                }
            }
            // Systemd owns the external browser; driver shutdown disconnects from it.
            if let Err(error) = client.browser.close().await {
                tracing::error!(%error, "Failed to close browser");
            }
            if let Err(error) = client.playwright.shutdown().await {
                tracing::error!(%error, "Failed to shut down Playwright");
            }
        }
    }
}

fn challenge_page(body: &str) -> bool {
    body.contains("window._cf_chl_opt")
        || body.contains("<title>Just a moment")
        || body.contains("<title>Un instant")
        || body.contains("<title>Attention Required!")
        || body.contains("Sorry, you have been blocked")
}

fn navigation_in_progress(message: &str) -> bool {
    message.contains("page is navigating") || message.contains("Execution context was destroyed")
}

fn page_ready(body: &str, selectors: &[&Selector]) -> bool {
    if selectors.is_empty() {
        return !body.trim().is_empty();
    }
    let html = Html::parse_document(body);
    selectors.iter().any(|selector| html.select(selector).next().is_some())
}
