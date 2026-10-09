use crate::scraping::tracking::scrape_error::ProductParseError;
use scraper::{ElementRef, Selector};

pub trait ElementRefExt {
    fn get_text(&self) -> String;
    fn select_elem(&self, selector: &Selector, element: &str) -> Result<ElementRef<'_>, ProductParseError>;
    fn select_text(&self, selector: &Selector, element: &str) -> Result<String, ProductParseError>;
    fn select_attr(&self, attr: &str, element: &str) -> Result<String, ProductParseError>;
}

impl<'a> ElementRefExt for ElementRef<'a> {
    fn get_text(&self) -> String {
        self.text().flat_map(|s| s.split_whitespace()).collect::<Vec<_>>().join(" ")
    }

    fn select_elem(&self, selector: &Selector, element: &str) -> Result<ElementRef<'_>, ProductParseError> {
        self.select(selector).next().ok_or_else(|| ProductParseError::MissingElement { field: element.to_string() })
    }

    fn select_text(&self, selector: &Selector, element: &str) -> Result<String, ProductParseError> {
        Ok(self
            .select(selector)
            .next()
            .ok_or_else(|| ProductParseError::MissingElement { field: element.to_string() })?
            .get_text())
    }

    fn select_attr(&self, attr: &str, element: &str) -> Result<String, ProductParseError> {
        Ok(self
            .attr(attr)
            .ok_or_else(|| ProductParseError::MissingAttribute {
                field: element.to_string(),
                attribute: attr.to_string(),
            })?
            .to_string())
    }
}
