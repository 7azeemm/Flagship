use playwright_rs::{Browser, BrowserContextOptions, GotoOptions, LaunchOptions, Page, Playwright, Route, WaitUntil};
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, HeaderMap, HeaderValue};
use reqwest::{Client, ClientBuilder};
use std::error::Error;
use std::time::Duration;
use tokio::sync::OnceCell;
use tokio::time::sleep;

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(30);

static WEB_CLIENT: OnceCell<WebClient> = OnceCell::const_new();

#[derive(Copy, Clone, Debug)]
pub enum WebClientType {
    HttpClient,
    Browser,
}

pub struct WebClient {
    pub http_client: Client,
    playwright: Playwright,
    browser: Browser,
}

impl WebClient {
    pub async fn init() {
        WEB_CLIENT.get_or_init(Self::initialize).await;
    }

    async fn initialize() -> Self {
        if std::env::args().any(|arg| arg == "--install-browser") {
            playwright_rs::install_browsers_with_deps(Some(&["chromium"]))
                .await.expect("Failed to install Chromium and its dependencies");
        }

        let http_client = ClientBuilder::new()
            .user_agent(USER_AGENT)
            .default_headers({
                let mut headers = HeaderMap::new();
                headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8"));
                headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));
                headers.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=0"));
                headers
            })
            .timeout(TIMEOUT)
            .build()
            .expect("Failed to build HTTP client");

        let playwright = Playwright::launch().await.expect("Failed to launch Playwright");
        let browser = playwright
            .chromium()
            .launch_with_options(LaunchOptions::default().headless(true).args(vec![
                "--disable-gpu".to_string(),
                "--disable-dev-shm-usage".to_string(),
                "--disable-plugins".to_string(),
                "--disable-image-loading".to_string(),
                "--disable-extensions".to_string(),
                "--disable-default-apps".to_string(),
                "--no-service-autorun".to_string(),
                "--disable-sync".to_string(),
                "--metrics-recording-only".to_string(),
                "--disable-background-networking".to_string(),
            ]))
            .await
            .expect("Failed to launch chromium");

        Self { http_client, playwright, browser }
    }

    pub fn get() -> &'static Self {
        WEB_CLIENT.get().expect("Scraper WebClient must be initialized before use")
    }

    pub async fn fetch(url: &str, web_client_type: &WebClientType) -> Result<String, Box<dyn Error>> {
        tracing::debug!(client = ?web_client_type, url, "Fetching retailer page");
        let web_client = Self::get();

        match web_client_type {
            WebClientType::HttpClient => {
                let response = web_client.http_client.get(url).send().await?.error_for_status()?;
                let body = response.text().await?;
                if body.contains("Sorry, you have been blocked") {
                    return Err(std::io::Error::other(format!("Request was blocked by {url}")).into());
                }
                Ok(body)
            }
            WebClientType::Browser => {
                let context = web_client
                    .browser
                    .new_context_with_options(
                        BrowserContextOptions::builder()
                            .user_agent(USER_AGENT.to_string())
                            .build(),
                    )
                    .await?;

                let result: Result<String, String> = match context.new_page().await {
                    Ok(page) => {
                        let result = Self::fetch_browser_page(&page, url).await.map_err(|e| e.to_string());
                        let _ = page.close().await;
                        result
                    }
                    Err(err) => Err(err.to_string()),
                };

                let _ = context.close().await;

                Ok(result?)
            }
        }
    }

    async fn fetch_browser_page(page: &Page, url: &str) -> Result<String, Box<dyn Error>> {
        // Block loading images
        let _ = page
            .route(
                "**/*.{png,jpg,jpeg,gif,svg,webp}",
                Box::new(|route: Route| Box::pin(async move { route.abort(None).await })),
            )
            .await;

        let response = page.goto(url, Some(
            GotoOptions::default()
                .wait_until(WaitUntil::DomContentLoaded)
                .timeout(TIMEOUT)
        )).await?;
        
        if let Some(response) = response.filter(|response| response.status() >= 400) {
            return Err(std::io::Error::other(format!("HTTP {} fetching {url}", response.status())).into());
        }
        sleep(Duration::from_millis(3000)).await;

        let body = page.content().await?;
        if body.contains("Sorry, you have been blocked") {
            return Err(std::io::Error::other(format!("Request was blocked by {url}")).into());
        }

        Ok(body)
    }

    pub async fn cleanup() {
        if let Some(client) = WEB_CLIENT.get() {
            if let Err(err) = client.browser.close().await {
                tracing::error!("Failed to close browser: {err}")
            }
            if let Err(err) = client.playwright.shutdown().await {
                tracing::error!("Failed to shutdown playwright: {err}")
            }
        }
    }
}
