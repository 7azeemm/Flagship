use super::PUBLIC_PAGES;
use crate::core::product::Product;
use crate::core::product::ProductStatus;
use crate::core::sections::Section;
use crate::core::storage::ProductStorage;
use crate::core::variant::Variant;
use regex::Regex;
use serde_json::{Value, json};
use std::sync::LazyLock;
use strum::IntoEnumIterator;

static DEFAULT_HEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?s)<title>.*?</title>|<meta\s+name="description"[^>]*>"#).unwrap());

pub(super) struct Metadata {
    title: String,
    description: String,
    path: String,
    image: Option<String>,
    pub(super) noindex: bool,
    structured: Option<Value>,
    preview: Option<String>,
}

pub(super) fn origin() -> Option<String> {
    let value = std::env::var("APP_PUBLIC_URL").ok()?;
    let url = reqwest::Url::parse(&value).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none())
    .then(|| url.origin().ascii_serialization())
}

pub(super) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn public_image(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.to_string())
}

fn json_for_html(value: &Value) -> String {
    value
        .to_string()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

fn stock_label(product: &Product) -> &'static str {
    if product.removed_at.is_some() {
        return "Listing removed";
    }
    match product.status {
        ProductStatus::InStock => "In Stock",
        ProductStatus::OutOfStock => "Out of Stock",
        ProductStatus::Arriving => "Arriving",
        ProductStatus::OnDemand => "On Demand",
    }
}

pub(super) fn product_metadata(product: &Product, base: &str) -> Metadata {
    let path = format!("/product/{}", urlencoding::encode(&product.id));
    let description = if product.removed_at.is_some() {
        format!(
            "{} is no longer listed at {}. Explore its recorded history and discover current offers on Flagship.",
            product.title, product.retailer
        )
    } else if product.price > 0 {
        format!(
            "{} — {} TND at {}. {}. Check recorded price history and specifications on Flagship, Tunisia’s tech comparison platform.",
            product.title,
            product.price,
            product.retailer,
            stock_label(product)
        )
    } else {
        format!(
            "{}. Explore specifications and recorded price history at {} on Flagship. Current price unavailable.",
            product.title, product.retailer
        )
    };
    let image = public_image(&product.image);
    let mut structured = json!({ "@context": "https://schema.org", "@type": "Product", "name": product.title, "description": description, "url": format!("{base}{path}") });
    if let Some(image) = &image {
        structured["image"] = json!([image]);
    }
    if product.price > 0 && product.removed_at.is_none() {
        let availability = match product.status {
            ProductStatus::InStock => "https://schema.org/InStock",
            ProductStatus::OutOfStock => "https://schema.org/OutOfStock",
            ProductStatus::Arriving => "https://schema.org/PreOrder",
            ProductStatus::OnDemand => "https://schema.org/BackOrder",
        };
        structured["offers"] = json!({ "@type": "Offer", "url": format!("{base}{path}"), "price": product.price, "priceCurrency": "TND", "availability": availability, "seller": { "@type": "Organization", "name": product.retailer } });
    }
    let preview = format!(
        "<article><h1>{}</h1><p>{}</p><p>Retailer: {} · {}</p><a href=\"/{}\">Browse current offers</a></article>",
        escape(&product.title),
        escape(&description),
        escape(&product.retailer),
        stock_label(product),
        product.section
    );
    Metadata {
        title: format!("{} — Flagship", product.title),
        description,
        path,
        image,
        noindex: product.removed_at.is_some(),
        structured: if product.removed_at.is_none() {
            Some(structured)
        } else {
            None
        },
        preview: Some(preview),
    }
}

pub(super) fn render(template: &str, metadata: &Metadata, base: &str) -> String {
    let canonical = escape(&format!("{base}{}", metadata.path));
    let title = escape(&metadata.title);
    let description = escape(&metadata.description);
    let mut head = format!(
        "<title>{title}</title>\n<meta name=\"description\" content=\"{description}\">\n<link rel=\"canonical\" href=\"{canonical}\">\n<meta property=\"og:type\" content=\"website\">\n<meta property=\"og:site_name\" content=\"Flagship\">\n<meta property=\"og:title\" content=\"{title}\">\n<meta property=\"og:description\" content=\"{description}\">\n<meta property=\"og:url\" content=\"{canonical}\">\n<meta name=\"twitter:title\" content=\"{title}\">\n<meta name=\"twitter:description\" content=\"{description}\">\n"
    );
    if let Some(image) = &metadata.image {
        head.push_str(&format!("<meta property=\"og:image\" content=\"{}\">\n<meta property=\"og:image:alt\" content=\"{title}\">\n<meta name=\"twitter:card\" content=\"summary_large_image\">\n<meta name=\"twitter:image\" content=\"{}\">\n", escape(image), escape(image)));
    } else {
        let image = escape(&format!("{base}/icon.png"));
        head.push_str(&format!("<meta property=\"og:image\" content=\"{image}\">\n<meta property=\"og:image:alt\" content=\"Flagship logo\">\n<meta name=\"twitter:card\" content=\"summary\">\n<meta name=\"twitter:image\" content=\"{image}\">\n"));
    }
    if metadata.noindex {
        head.push_str("<meta name=\"robots\" content=\"noindex, follow\">\n");
    }
    if let Some(structured) = &metadata.structured {
        head.push_str(&format!(
            "<script id=\"flagship-structured-data\" type=\"application/ld+json\">{}</script>\n",
            json_for_html(structured)
        ));
    }
    let html = DEFAULT_HEAD
        .replace_all(template, "")
        .replacen("</head>", &format!("{head}</head>"), 1);
    if let Some(preview) = &metadata.preview {
        html.replacen(
            "<div id=\"root\"></div>",
            &format!("<div id=\"root\">{preview}</div>"),
            1,
        )
    } else {
        html
    }
}

pub(super) fn page_metadata(path: &str) -> Metadata {
    let (title, description): (String, String) = match path {
        "/" => (
            "Flagship — Tech prices in perspective".into(),
            "Compare tech prices across Tunisian retailers. Find recorded deals, explore price history, and create free price alerts.".into(),
        ),
        "/statistics" => (
            "Market insights — Flagship".into(),
            "Explore comparable tech price trends, retailer comparisons, and configuration history in Tunisia.".into(),
        ),
        "/support" => (
            "Support Flagship".into(),
            "Meet the creator and support Flagship, a free platform for comparing tech prices in Tunisia.".into(),
        ),
        "/future-plans" => ("Future plans — Flagship".into(), "Explore and vote for upcoming Flagship features.".into()),
        "/legal" => (
            "Terms & privacy — Flagship".into(),
            "Simple terms of service, retailer data disclaimer, and privacy policy for Flagship.".into(),
        ),
        _ => (
            format!("{} — Flagship", path.trim_start_matches('/')),
            "Compare current offers, specifications, and recorded prices across Tunisian retailers on Flagship.".into(),
        ),
    };
    let category = Section::iter().any(|section| path == format!("/{section}"));
    Metadata {
        title: title.clone(),
        description: description.clone(),
        path: path.into(),
        image: None,
        noindex: !PUBLIC_PAGES.contains(&path) && !category,
        structured: None,
        preview: Some(format!(
            "<main><h1>{}</h1><p>{}</p><nav aria-label=\"Catalog\">{}</nav></main>",
            escape(&title), escape(&description),
            Section::iter().filter(|section| *section != Section::Others)
                .map(|section| format!("<a href=\"/{section}\">{section}</a>"))
                .collect::<Vec<_>>().join(" · ")
        )),
    }
}

pub(super) fn variant_metadata(variant: &Variant, storage: &ProductStorage) -> Metadata {
    let mut offers = variant
        .product_ids
        .iter()
        .filter_map(|id| storage.products.get(id))
        .collect::<Vec<_>>();
    offers.sort_by(|a, b| {
        (a.price <= 0)
            .cmp(&(b.price <= 0))
            .then(a.price.cmp(&b.price))
            .then(a.id.cmp(&b.id))
    });
    let title = variant
        .section
        .config()
        .group
        .iter()
        .filter_map(|field| variant.fields.get(&field.key))
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ");
    let name = if title.is_empty() {
        format!("{} configuration", variant.section)
    } else {
        title
    };
    let description = format!(
        "Compare {} offers for {} across Tunisian retailers. Explore prices, availability, and configuration history on Flagship.",
        offers.len(),
        name
    );
    let image = offers.iter().find_map(|product| public_image(&product.image));
    let noindex = !variant.active || offers.is_empty();
    Metadata {
        title: format!("{name} — Flagship"),
        description,
        path: format!("/variant/{}", urlencoding::encode(&variant.id)),
        image,
        noindex,
        structured: None,
        preview: Some(format!(
            "<article><h1>{}</h1><p>Compare {} recorded retailer offers for this configuration.</p></article>",
            escape(&name),
            offers.len()
        )),
    }
}

pub(super) fn not_found_metadata() -> Metadata {
    Metadata {
        title: "Page not found — Flagship".into(),
        description: "This Flagship page is unavailable. Browse the catalog to find current offers.".into(),
        path: "/".into(),
        image: None,
        noindex: true,
        structured: None,
        preview: Some("<main><h1>Page not found</h1><a href=\"/\">Browse Flagship</a></main>".into()),
    }
}
