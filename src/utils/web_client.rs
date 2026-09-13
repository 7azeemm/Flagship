use axum::http::HeaderMap;
use once_cell::sync::OnceCell;
use playwright_rs::{
    Browser, BrowserContextOptions, GotoOptions, LaunchOptions, Page, Playwright, Route, WaitUntil,
};
use reqwest::{Client, ClientBuilder};
use std::collections::HashMap;
use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;

pub const USER_AGENT: &'static str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(30);

static WEB_CLIENT: OnceCell<WebClient> = OnceCell::new();

#[derive(Copy, Clone, Debug)]
pub enum WebClientType {
    HttpClient,
    Browser,
}

pub struct WebClient {
    pub http_client: Client,
    pub playwright: Playwright,
    pub browser: Browser,
}

impl WebClient {
    pub async fn init() {
        let http_client = ClientBuilder::new()
            .user_agent(USER_AGENT)
            .default_headers({
                let mut headers = HeaderMap::new();
                headers.insert("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8,".parse().unwrap());
                headers.insert("Accept-Language", "en-US,en;q=0.5".parse().unwrap());
                headers.insert("Accept-Encoding", "gzip, deflate, br, zstd".parse().unwrap());
                headers.insert("Cache-Control", "max-age=0".parse().unwrap());
                headers.insert("Connection", "keep-alive".parse().unwrap());
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

        let _ = WEB_CLIENT.set(WebClient {
            http_client,
            playwright,
            browser,
        });
    }

    pub fn get() -> &'static Self {
        WEB_CLIENT.get().unwrap()
    }

    pub async fn fetch(url: &str, web_client_type: &WebClientType) -> Result<String, Box<dyn Error>> {
        println!("Sending {web_client_type:?} request to `{url}`");
        let web_client = WEB_CLIENT.get().unwrap();

        match web_client_type {
            WebClientType::HttpClient => {
                let response = web_client.http_client.get(url).send().await?.error_for_status()?;
                let body = response.text().await?;
                if body.contains("Sorry, you have been blocked") {
                    return Err(
                        std::io::Error::other(format!("Request was blocked by {url}")).into(),
                    );
                }
                Ok(body)
            }
            WebClientType::Browser => {
                let context = web_client
                    .browser
                    .new_context_with_options(
                        BrowserContextOptions::builder()
                            .user_agent(USER_AGENT.to_string())
                            .extra_http_headers(HashMap::from([(
                                "Accept-Language".to_string(),
                                "en-US,en;q=0.5".to_string(),
                            )]))
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

        page.goto(url, Some(
            GotoOptions::default()
                .wait_until(WaitUntil::DomContentLoaded)
                .timeout(TIMEOUT),
        )).await?;
        sleep(Duration::from_millis(3000)).await;

        let body = page.content().await?;
        if body.contains("Sorry, you have been blocked") {
            return Err(std::io::Error::other(format!("Request was blocked by {url}")).into());
        }

        Ok(body)
    }

    pub async fn cleanup() -> Result<(), Box<dyn Error>> {
        if let Some(client) = WEB_CLIENT.get() {
            let _ = client.browser.close().await;
            let _ = client.playwright.shutdown().await;
        }
        Ok(())
    }
}