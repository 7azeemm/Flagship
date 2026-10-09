use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum FetchError {
    #[error("Request failed: {message}")]
    Request { message: String },
    #[error("The page returned no products")]
    EmptyProductPage,
}

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum PaginationError {
    #[error("Pagination value was not found")]
    MissingValue,
    #[error("Invalid pagination value `{value}`")]
    InvalidValue { value: String },
}

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum UrlError {
    #[error("URL is empty")]
    Empty,
    #[error("URL is malformed or contains credentials: `{value}`")]
    Malformed { value: String },
    #[error("URL must start with http:// or https://: `{value}`")]
    InvalidScheme { value: String },
    #[error("URL contains spaces: `{value}`")]
    ContainsSpaces { value: String },
}

impl UrlError {
    fn fingerprint(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Malformed { .. } => "malformed",
            Self::InvalidScheme { .. } => "invalid_scheme",
            Self::ContainsSpaces { .. } => "contains_spaces",
        }
    }
}

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum ProductParseError {
    #[error("{field} element not found")]
    MissingElement { field: String },
    #[error("{attribute} attribute not found on {field}")]
    MissingAttribute { field: String, attribute: String },
    #[error("Image URL not found")]
    MissingImageUrl,
    #[error(transparent)]
    InvalidUrl(#[from] UrlError),
    #[error("Failed to parse price `{value}`")]
    InvalidPrice { value: String },
    #[error("Unknown product status `{value}`")]
    UnknownStatus { value: String },
    #[error("{message}")]
    Other { message: String },
}

impl ProductParseError {
    fn fingerprint(&self) -> String {
        match self {
            Self::MissingElement { field } => format!("missing_element:{field}"),
            Self::MissingAttribute { field, attribute } => format!("missing_attribute:{field}:{attribute}"),
            Self::MissingImageUrl => "missing_image_url".into(),
            Self::InvalidUrl(error) => format!("invalid_url:{}", error.fingerprint()),
            Self::InvalidPrice { .. } => "invalid_price".into(),
            Self::UnknownStatus { .. } => "unknown_status".into(),
            Self::Other { message } => format!("other:{message}"),
        }
    }
}

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum ProductFetchError {
    #[error("Product fetch request failed: {message}")]
    FetchFailed { message: String },
    #[error("Product page no longer exists")]
    ProductMissing,
    #[error("{field} was missing on the product page")]
    ContentMissing { field: String },
}

impl ProductFetchError {
    pub fn skip_product(&self) -> bool {
        matches!(self, Self::ProductMissing)
    }

    fn fingerprint(&self) -> String {
        match self {
            Self::FetchFailed { .. } => "fetch_failed".into(),
            Self::ProductMissing => "product_missing".into(),
            Self::ContentMissing { field } => format!("content_missing:{field}"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScrapeErrorKind {
    FetchFailed(FetchError),
    PageCountParseFailed(PaginationError),
    ParseFailed { url: Option<String>, error: ProductParseError },
    ProductFetchFailed { url: String, title: String, error: ProductFetchError },
}

impl ScrapeErrorKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FetchFailed(_) => "Fetch failed",
            Self::PageCountParseFailed(_) => "Page count failed",
            Self::ParseFailed { .. } => "Product parse failed",
            Self::ProductFetchFailed { .. } => "Product fetch failed",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::FetchFailed(error) => error.to_string(),
            Self::PageCountParseFailed(error) => error.to_string(),
            Self::ParseFailed { error, .. } => error.to_string(),
            Self::ProductFetchFailed { error, .. } => error.to_string(),
        }
    }

    pub fn fingerprint(&self) -> String {
        match self {
            Self::FetchFailed(FetchError::Request { .. }) => "fetch:request".into(),
            Self::FetchFailed(FetchError::EmptyProductPage) => "fetch:empty_page".into(),
            Self::PageCountParseFailed(PaginationError::MissingValue) => "pagination:missing_value".into(),
            Self::PageCountParseFailed(PaginationError::InvalidValue { .. }) => "pagination:invalid_value".into(),
            Self::ParseFailed { error, .. } => format!("product:{}", error.fingerprint()),
            Self::ProductFetchFailed { error, .. } => format!("product_fetch:{}", error.fingerprint()),
        }
    }

    pub fn target_url(&self) -> Option<&str> {
        match self {
            Self::ParseFailed { url, .. } => url.as_deref(),
            Self::ProductFetchFailed { url, .. } => Some(url),
            _ => None,
        }
    }

    pub fn fails_scope(&self) -> bool {
        // A listing that failed to parse is not evidence of a retailer removal.
        // Preserve the scope until a complete scan can establish what disappeared.
        matches!(self, Self::FetchFailed(_) | Self::PageCountParseFailed(_) | Self::ParseFailed { .. })
    }
}
