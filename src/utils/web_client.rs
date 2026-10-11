use playwright_rs::{
    Browser, BrowserContext, BrowserContextOptions, ConnectOverCdpOptions, GotoOptions, LaunchOptions, Page,
    Playwright, WaitUntil,
};
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, HeaderMap, HeaderValue};
use reqwest::{Client, ClientBuilder};
use scraper::{Html, Selector};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::{Mutex, OnceCell, Semaphore};
use tokio::time::{Instant, sleep, sleep_until};

pub const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(30);
const CHALLENGE_TIMEOUT: Duration = Duration::from_secs(45);
const RETAILER_PROXY: &str = "socks5h://127.0.0.1:40000";
const BROWSER_CDP_ENDPOINT: &str = "http://127.0.0.1:9224";
const TAB_REQUEST_LIMIT: usize = 20;
static WEB_CLIENT: OnceCell<WebClient> = OnceCell::const_new();

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
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

/// Production defaults are constants; the benchmark can override them in its own process.
#[derive(Clone, Copy, Debug)]
pub struct FetchLimits {
    pub browser: usize,
    pub http: usize,
    pub browser_per_host: usize,
    pub http_per_host: usize,
    pub proxied_start_gap: Duration,
}

impl Default for FetchLimits {
    fn default() -> Self {
        Self {
            browser: 4,
            http: 20,
            browser_per_host: 4,
            http_per_host: 4,
            proxied_start_gap: Duration::from_millis(250),
        }
    }
}

impl FetchLimits {
    pub fn per_host(self, client: WebClientType) -> usize {
        if client.is_browser() {
            self.browser_per_host
        } else {
            self.http_per_host
        }
    }
}

struct RetailerSession {
    slots: Semaphore,
    next_request: Mutex<Instant>,
}

struct PooledPage {
    page: Page,
    requests: usize,
}

// Cancellation must close a checked-out tab instead of leaving it in Chromium.
struct PageLease(Option<PooledPage>);

impl Drop for PageLease {
    fn drop(&mut self) {
        if let Some(page) = self.0.take() {
            tokio::spawn(async move {
                let _ = page.page.close().await;
            });
        }
    }
}

#[derive(Default)]
struct RequestCounter {
    active: AtomicUsize,
    peak: AtomicUsize,
}

struct RequestGuard<'a>(&'a RequestCounter);

impl RequestCounter {
    fn enter(&self) -> RequestGuard<'_> {
        let active = self.active.fetch_add(1, Ordering::Relaxed) + 1;
        self.peak.fetch_max(active, Ordering::Relaxed);
        RequestGuard(self)
    }
}

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::Relaxed);
    }
}

pub struct WebClient {
    pub http_client: Client,
    proxy_client: Client,
    playwright: Playwright,
    browser: Browser,
    persistent_browser: Mutex<Option<PersistentBrowserState>>,
    browser_context: BrowserContext,
    browser_pages: Mutex<HashMap<WebClientType, Vec<PooledPage>>>,
    retailer_sessions: Mutex<HashMap<(String, WebClientType), Arc<RetailerSession>>>,
    browser_slots: Semaphore,
    http_slots: Semaphore,
    pub limits: FetchLimits,
    browser_requests: RequestCounter,
    http_requests: RequestCounter,
}

impl WebClient {
    pub async fn init() {
        Self::init_with_limits(FetchLimits::default()).await;
    }

    pub async fn init_with_limits(limits: FetchLimits) {
        assert!(limits.browser > 0 && limits.http > 0 && limits.browser_per_host > 0 && limits.http_per_host > 0);
        WEB_CLIENT.get_or_init(|| Self::initialize(limits)).await;
    }

    async fn initialize(limits: FetchLimits) -> Self {
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
                "--disable-back-forward-cache".into(),
            ]))
            .await
            .expect("Failed to launch chromium");
        let browser_context = browser
            .new_context_with_options(
                BrowserContextOptions::builder()
                    .user_agent(USER_AGENT.to_string())
                    .build(),
            )
            .await
            .expect("Failed to create browser context");
        tracing::info!(browser = limits.browser, http = limits.http, "Fetch limits");
        Self {
            http_client,
            proxy_client,
            playwright,
            browser,
            browser_context,
            browser_pages: Mutex::new(HashMap::new()),
            persistent_browser: Mutex::new(None),
            retailer_sessions: Mutex::new(HashMap::new()),
            browser_slots: Semaphore::new(limits.browser),
            http_slots: Semaphore::new(limits.http),
            limits,
            browser_requests: RequestCounter::default(),
            http_requests: RequestCounter::default(),
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
        WEB_CLIENT
            .get()
            .expect("Scraper WebClient must be initialized before use")
    }

    pub fn peak_requests(&self) -> (usize, usize) {
        (
            self.browser_requests.peak.load(Ordering::Relaxed),
            self.http_requests.peak.load(Ordering::Relaxed),
        )
    }

    async fn session(
        &self,
        url: &str,
        client: WebClientType,
    ) -> Result<Arc<RetailerSession>, Box<dyn Error + Send + Sync>> {
        let host = reqwest::Url::parse(url)?
            .host_str()
            .ok_or("Retailer URL has no host")?
            .to_owned();
        Ok(self
            .retailer_sessions
            .lock()
            .await
            .entry((host, client))
            .or_insert_with(|| {
                Arc::new(RetailerSession {
                    slots: Semaphore::new(self.limits.per_host(client)),
                    next_request: Mutex::new(Instant::now()),
                })
            })
            .clone())
    }

    /// API-based retailers use the same HTTP limits as HTML fetches.
    pub async fn http_request(
        url: &str,
        request: reqwest::RequestBuilder,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let client = Self::get();
        let session = client.session(url, WebClientType::HttpClient).await?;
        let _host = session.slots.acquire().await?;
        let _global = client.http_slots.acquire().await?;
        let _request = client.http_requests.enter();
        Ok(request.send().await?.error_for_status()?.text().await?)
    }

    pub async fn fetch(
        url: &str,
        client_type: &WebClientType,
        ready: &[&Selector],
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        tracing::debug!(client = ?client_type, url, "Fetching retailer page");
        let client = Self::get();
        let waiting = Instant::now();
        let session = client.session(url, *client_type).await?;
        // Acquire the host slot first so one busy retailer cannot occupy every global slot while waiting.
        let _host_slot = session.slots.acquire().await?;
        let _global_slot = if client_type.is_browser() {
            client.browser_slots.acquire().await?
        } else {
            client.http_slots.acquire().await?
        };
        
        if matches!(
            client_type,
            WebClientType::ProxyHttpClient | WebClientType::PersistentBrowser
        ) {
            let mut next = session.next_request.lock().await;
            sleep_until(*next).await;
            *next = Instant::now() + client.limits.proxied_start_gap;
        }
        tracing::debug!(url, queued_ms = waiting.elapsed().as_millis(), "Request slot acquired");
        
        let _request = if client_type.is_browser() {
            client.browser_requests.enter()
        } else {
            client.http_requests.enter()
        };
        
        match client_type {
            WebClientType::HttpClient => Self::fetch_http(&client.http_client, url).await,
            WebClientType::ProxyHttpClient => Self::fetch_http(&client.proxy_client, url).await,
            WebClientType::Browser | WebClientType::PersistentBrowser => {
                client.fetch_browser(url, *client_type, ready).await
            }
        }
    }

    async fn fetch_browser(
        &self,
        url: &str,
        client_type: WebClientType,
        ready: &[&Selector],
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let context = if client_type == WebClientType::PersistentBrowser {
            self.persistent_context().await?
        } else {
            self.browser_context.clone()
        };
        // Share tabs across hosts; cookies remain in the browser context.
        // A per-host pool retains many idle renderers during a full scan.
        let pooled = self
            .browser_pages
            .lock()
            .await
            .entry(client_type)
            .or_default()
            .pop()
            .filter(|page| !page.page.is_closed());
        let page = match pooled {
            Some(page) => page,
            None => PooledPage {
                page: context.new_page().await?,
                requests: 0,
            },
        };
        
        let mut lease = PageLease(Some(page));
        let page = &lease.0.as_ref().unwrap().page;
        let result = Self::fetch_browser_page(page, url, ready, client_type == WebClientType::PersistentBrowser).await;
        
        // Free document scripts/images between requests; cookies and HTTP cache stay in the shared context.
        // Periodic retirement also releases accumulated Playwright request/response objects.
        if result.is_ok()
            && lease.0.as_ref().unwrap().requests + 1 < TAB_REQUEST_LIMIT
            && page.goto("about:blank", Some(GotoOptions::default().timeout(TIMEOUT))).await.is_ok()
        {
            let mut pooled = lease.0.take().unwrap();
            pooled.requests += 1;
            self.browser_pages
                .lock()
                .await
                .entry(client_type)
                .or_default()
                .push(pooled);
        }
        
        if let Some(retired) = lease.0.take() {
            let _ = retired.page.close().await;
        }
        result
    }

    async fn fetch_http(client: &Client, url: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let response = client.get(url).send().await?.error_for_status()?;
        let challenged = response
            .headers()
            .get("cf-mitigated")
            .is_some_and(|value| value == "challenge");
        let body = response.text().await?;
        if challenged || challenge_page(&body) {
            return Err(std::io::Error::other("Cloudflare challenge or block").into());
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
                Some(
                    GotoOptions::default()
                        .wait_until(WaitUntil::DomContentLoaded)
                        .timeout(TIMEOUT),
                ),
            )
            .await?;

        let status = response.as_ref().map(|response| response.status());
        let challenged = response.as_ref().is_some_and(|response| {
            response
                .headers()
                .get("cf-mitigated")
                .is_some_and(|value| value == "challenge")
        });

        if !persistent && let Some(status) = status.filter(|status| *status >= 400) {
            return Err(std::io::Error::other(format!("HTTP {status}")).into());
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
            if blocked && !persistent {
                return Err(std::io::Error::other("Cloudflare challenge or block").into());
            }
            saw_challenge |= blocked;
            if !blocked
                && !saw_challenge
                && let Some(status) = status.filter(|status| *status >= 400)
            {
                return Err(std::io::Error::other(format!("HTTP {status}")).into());
            }
            if !blocked && page_ready(&body, ready) {
                tracing::debug!(url, initial_status = ?status, saw_challenge, "Retailer browser page ready");
                return Ok(body);
            }

            if Instant::now() >= deadline {
                return Err(std::io::Error::other(format!(
                    "{} timed out after {}s (HTTP {})",
                    if saw_challenge {
                        "Cloudflare challenge"
                    } else {
                        "Page content"
                    },
                    CHALLENGE_TIMEOUT.as_secs(),
                    status.map_or_else(|| "unknown".into(), |status| status.to_string()),
                ))
                .into());
            }
            sleep(Duration::from_millis(200)).await;
        }
    }

    pub async fn cleanup() {
        if let Some(client) = WEB_CLIENT.get() {
            for pages in client.browser_pages.lock().await.values_mut() {
                for page in pages.drain(..) {
                    let _ = page.page.close().await;
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
