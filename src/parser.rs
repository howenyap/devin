use scraper::{Html, Selector};
use url::Url;

/// Parsed result from an HTML page.
pub struct ParseResult {
    /// The page title, if found.
    pub title: Option<String>,
    /// All discovered absolute URLs from `<a href="...">` tags.
    pub links: Vec<Url>,
}

/// Parse an HTML document and extract links and metadata.
pub fn parse(html: &str, base_url: &Url) -> ParseResult {
    let document = Html::parse_document(html);

    let title_sel = Selector::parse("title").expect("valid selector: title");
    let title = document
        .select(&title_sel)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string());

    let link_sel = Selector::parse("a[href]").expect("valid selector: a[href]");
    let links = document
        .select(&link_sel)
        .filter_map(|el| {
            let href = el.value().attr("href")?;
            resolve_url(href, base_url)
        })
        .collect();

    ParseResult { title, links }
}

/// Resolve a potentially relative URL against a base URL.
/// Only keeps http/https URLs.
fn resolve_url(href: &str, base: &Url) -> Option<Url> {
    let url = base.join(href).ok()?;
    match url.scheme() {
        "http" | "https" => Some(url),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_title_and_links() {
        let html = r#"
        <html>
            <head><title>Test Page</title></head>
            <body>
                <a href="/about">About</a>
                <a href="https://other.com/page">Other</a>
                <a href="mailto:hi@test.com">Email</a>
            </body>
        </html>
        "#;
        let base = Url::parse("https://example.com/index.html").unwrap();
        let result = parse(html, &base);

        assert_eq!(result.title.as_deref(), Some("Test Page"));
        assert_eq!(result.links.len(), 2);
        assert_eq!(
            result.links[0].as_str(),
            "https://example.com/about"
        );
        assert_eq!(result.links[1].as_str(), "https://other.com/page");
    }
}
