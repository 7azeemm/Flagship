//! Uses real retailer fetchers and parsers, without database initialization or catalog writes.
use flagship::core::product::Product;
use flagship::core::sections::Section;
use flagship::scraping::retailers::{RETAILERS, Retailer};
use flagship::scraping::scanner::CACHED_PRODUCT_INFO;
use flagship::scraping::tracking::scan_metrics::PageMetrics;
use flagship::scraping::tracking::scrape_error::ScrapeErrorKind;
use flagship::utils::web_client::{FetchLimits, WebClient};
use futures::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize)]
struct Category {
    retailer: String,
    url: String,
    section: Section,
    #[serde(default)]
    expected_errors: Vec<String>,
    #[serde(default)]
    expected_products: usize,
}

#[derive(Serialize, Deserialize)]
struct Fixture {
    categories: Vec<Category>,
    products: Vec<Product>,
}

fn retailer(name: &str) -> &'static dyn Retailer {
    RETAILERS
        .iter()
        .find(|site| site.name() == name)
        .expect("Unknown fixture retailer")
        .as_ref()
}

fn option(args: &[String], name: &str, default: &str) -> String {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .unwrap_or_else(|| default.into())
}

async fn details(products: &[Product]) -> (PageMetrics, Vec<serde_json::Value>, f64) {
    let started = Instant::now();
    let results = stream::iter(products.iter().cloned().map(|mut product| async move {
        let started = Instant::now();
        let mut metrics = PageMetrics::default();
        let result = retailer(&product.retailer)
            .fetch_product_page(&mut product, &mut metrics)
            .await;
        let cached = CACHED_PRODUCT_INFO.read().await.contains_key(&product.url);
        (
            metrics,
            json!({"retailer":product.retailer, "url":product.url,
            "elapsed_ms":started.elapsed().as_secs_f64()*1000.0,
            "error":result.err().map(|error| error.to_string()), "cached":cached}),
        )
    }))
    .buffer_unordered(64)
    .collect::<Vec<_>>()
    .await;
    let mut metrics = PageMetrics::default();
    let mut records = Vec::new();
    for (item, record) in results {
        metrics.add(&item);
        records.push(record);
    }
    (metrics, records, started.elapsed().as_secs_f64())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .compact()
        .with_target(false)
        .with_timer(flagship::utils::logger::LogTime)
        .with_ansi(false)
        .with_env_filter("info")
        .init();
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 || !["prepare", "run"].contains(&args[1].as_str()) {
        eprintln!(
            "Usage: scan-benchmark prepare FIXTURE [--samples 8] [--retailers Mytek,MediaVision,CyberInfo,Lazari,Batam,SkyMil-Shop,Agora,Tunisianet]\n       scan-benchmark run FIXTURE --browser 4 --per-host 2 --http 20 --http-per-host 4 --gap-ms 250 --output RESULT.json"
        );
        std::process::exit(2);
    }
    let defaults = FetchLimits::default();
    let limits = FetchLimits {
        browser: option(&args, "--browser", &defaults.browser.to_string()).parse()?,
        browser_per_host: option(&args, "--per-host", &defaults.browser_per_host.to_string()).parse()?,
        http: option(&args, "--http", &defaults.http.to_string()).parse()?,
        http_per_host: option(&args, "--http-per-host", &defaults.http_per_host.to_string()).parse()?,
        proxied_start_gap: Duration::from_millis(
            option(&args, "--gap-ms", &defaults.proxied_start_gap.as_millis().to_string()).parse()?,
        ),
    };
    WebClient::init_with_limits(limits).await;
    let result = tokio::select! {
        result = execute(&args, limits) => result,
        _ = interrupted() => Err("Benchmark interrupted".into()),
    };
    WebClient::cleanup().await;
    result
}

async fn interrupted() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! { _ = terminate.recv() => {}, _ = tokio::signal::ctrl_c() => {} }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn execute(args: &[String], limits: FetchLimits) -> Result<(), Box<dyn Error + Send + Sync>> {
    if args[1] == "prepare" {
        let names = option(
            args,
            "--retailers",
            "Mytek,MediaVision,CyberInfo,Lazari,Batam,SkyMil-Shop,Agora,Tunisianet",
        );
        let samples: usize = option(args, "--samples", "8").parse()?;
        let mut categories = Vec::new();
        let mut groups = Vec::new();
        for name in names.split(',') {
            let site = retailer(name);
            let &(section, url) = site
                .config()
                .sections
                .iter()
                .find(|(section, _)| *section == Section::Laptop)
                .or_else(|| site.config().sections.first())
                .ok_or("Retailer has no categories")?;
            let (report, products, _) = site.scrape_page(url, 1, section).await;
            if products.is_empty()
                || report
                    .errors
                    .iter()
                    .any(|error| matches!(error.error, ScrapeErrorKind::FetchFailed(_)))
            {
                return Err(format!("Fixture category failed: {name}: {:?}", report.errors).into());
            }
            tracing::info!(retailer = name, count = products.len(), "Benchmark category prepared");
            categories.push(Category {
                retailer: name.into(),
                url: url.into(),
                section,
                expected_errors: {
                    let mut errors: Vec<_> = report.errors.iter().map(|error| format!("{:?}", error.error)).collect();
                    errors.sort();
                    errors
                },
                expected_products: products.len(),
            });
            groups.push(products.into_iter().take(samples).collect::<Vec<_>>());
        }
        // Interleave hosts so every concurrency configuration sees the same fair workload.
        let mut products = Vec::new();
        for index in 0..samples {
            for group in &groups {
                if let Some(product) = group.get(index) {
                    products.push(product.clone());
                }
            }
        }
        std::fs::write(&args[2], serde_json::to_vec_pretty(&Fixture { categories, products })?)?;
        return Ok(());
    }
    let fixture: Fixture = serde_json::from_slice(&std::fs::read(&args[2])?)?;
    if fixture.products.is_empty() {
        return Err("Fixture has no products".into());
    }
    let category_start = Instant::now();
    let categories = stream::iter(fixture.categories.iter().map(|category| async move {
        let (report, products, _) = retailer(&category.retailer)
            .scrape_page(&category.url, 1, category.section)
            .await;
        let mut errors: Vec<_> = report.errors.iter().map(|error| format!("{:?}", error.error)).collect();
        errors.sort();
        json!({"retailer": category.retailer, "products":products.len(), "errors":report.errors,
            "consistent":errors == category.expected_errors && (category.expected_products == 0 || products.len() == category.expected_products),
            "elapsed_ms":report.duration.as_secs_f64()*1000.0})
    }))
    .buffer_unordered(64)
    .collect::<Vec<_>>()
    .await;
    let category_seconds = category_start.elapsed().as_secs_f64();
    let cold_rounds: usize = option(args, "--cold-rounds", "1").parse()?;
    if cold_rounds == 0 {
        return Err("--cold-rounds must be positive".into());
    }
    let mut cold_metrics = PageMetrics::default();
    let mut cold = Vec::new();
    let mut cold_seconds = 0.0;
    for _ in 0..cold_rounds {
        CACHED_PRODUCT_INFO.write().await.clear();
        let (metrics, records, seconds) = details(&fixture.products).await;
        cold_metrics.add(&metrics);
        cold.extend(records);
        cold_seconds += seconds;
    }
    let (warm_metrics, warm, warm_seconds) = details(&fixture.products).await;
    // Verify expiry and coalescing using real requests, outside the timed workload.
    let sample = fixture.products.first().unwrap();
    if let Some(cached) = CACHED_PRODUCT_INFO.write().await.get_mut(&sample.url) {
        cached.timestamp = chrono::Utc::now() - chrono::Duration::days(15);
    }
    let (expired_metrics, expired, _) = details(std::slice::from_ref(sample)).await;
    CACHED_PRODUCT_INFO.write().await.remove(&sample.url);
    let (duplicate_metrics, duplicates, _) = details(&vec![sample.clone(); 8]).await;
    let (peak_browser, peak_http) = WebClient::get().peak_requests();
    let valid = cold
        .iter()
        .all(|item| item["error"].is_null() && item["cached"] == true)
        && categories
            .iter()
            .all(|item| item["products"].as_u64().unwrap_or(0) > 0 && item["consistent"] == true)
        && warm_metrics.product_page_requests == 0
        && warm_metrics.cache_hits == fixture.products.len()
        && expired_metrics.product_page_requests == 1
        && duplicate_metrics.product_page_requests == 1
        && duplicate_metrics.cache_hits == 7
        && duplicates.iter().all(|item| item["error"].is_null())
        && peak_browser <= limits.browser
        && peak_http <= limits.http;
    let output = json!({"valid":valid, "limits":{"browser":limits.browser, "per_host":limits.browser_per_host,
        "http":limits.http, "http_per_host":limits.http_per_host, "gap_ms":limits.proxied_start_gap.as_millis()},
        "products":fixture.products.len(), "cold_rounds":cold_rounds,
        "category_seconds":category_seconds, "cold_seconds":cold_seconds,
        "warm_seconds":warm_seconds, "cold_metrics":cold_metrics, "warm_metrics":warm_metrics,
        "expired_metrics":expired_metrics, "duplicate_metrics":duplicate_metrics,
        "peak_browser_requests":peak_browser, "peak_http_requests":peak_http,
        "categories":categories, "cold":cold, "warm":warm, "expired":expired, "duplicates":duplicates});
    std::fs::write(
        option(args, "--output", "benchmark-result.json"),
        serde_json::to_vec_pretty(&output)?,
    )?;
    println!(
        "BENCHMARK_RESULT valid={valid} browser={} per_host={} http={} categories={category_seconds:.3}s cold={cold_seconds:.3}s warm={warm_seconds:.3}s",
        limits.browser, limits.browser_per_host, limits.http
    );
    if !valid {
        return Err("Benchmark validation failed; inspect the result JSON".into());
    }
    Ok(())
}
