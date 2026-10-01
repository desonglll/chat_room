//! TG-408: Open Graph / `<title>` extraction from the first bytes of an HTML page. A small
//! scanner rather than an HTML parser dependency: it only needs `<meta>` attributes and the
//! title, and it never executes or follows anything it reads.

use reqwest::Url;

use super::LinkPreview;

const MAX_TITLE: usize = 200;
const MAX_DESCRIPTION: usize = 400;

/// Decodes the handful of entities that appear in titles and descriptions.
fn decode_entities(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end) = tail.find(';').filter(|end| *end <= 10) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let entity = &tail[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(character) => {
                out.push(character);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn clean(value: &str, max_chars: usize) -> String {
    let collapsed = decode_entities(value)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut out: String = collapsed.chars().take(max_chars).collect();
    if collapsed.chars().count() > max_chars {
        out.push('…');
    }
    out
}

/// The value of attribute `name` in one tag's source (`<meta ...>`), case-insensitive name.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(name) {
        let at = from + found;
        from = at + name.len();
        let before_ok = at == 0 || lower.as_bytes()[at - 1].is_ascii_whitespace();
        let after = lower[from..].trim_start();
        if !before_ok || !after.starts_with('=') {
            continue;
        }
        let value_start = tag.len() - after.len() + 1;
        let value = tag[value_start..].trim_start();
        return match value.chars().next()? {
            quote @ ('"' | '\'') => value[1..].split(quote).next().map(str::to_string),
            _ => value
                .split(|c: char| c.is_ascii_whitespace() || c == '>')
                .next()
                .map(str::to_string),
        };
    }
    None
}

/// Extract a preview from `html` fetched from `page` (for resolving a relative `og:image`).
/// `None` when the page offers neither a title nor a description.
pub fn parse_preview(html: &str, page: &Url) -> Option<LinkPreview> {
    let mut preview = LinkPreview {
        url: page.to_string(),
        site_name: page.host_str().unwrap_or_default().to_string(),
        title: String::new(),
        description: String::new(),
        image_url: None,
    };
    let mut og_title = None;
    let lower = html.to_ascii_lowercase();
    let mut cursor = 0;
    while let Some(found) = lower[cursor..].find("<meta") {
        let start = cursor + found;
        let end = lower[start..]
            .find('>')
            .map_or(lower.len(), |end| start + end);
        let tag = &html[start..end];
        cursor = end;
        let key = attribute(tag, "property")
            .or_else(|| attribute(tag, "name"))
            .map(|key| key.to_ascii_lowercase());
        let Some(content) = attribute(tag, "content") else {
            continue;
        };
        match key.as_deref() {
            Some("og:title") => og_title = Some(clean(&content, MAX_TITLE)),
            Some("og:description") => preview.description = clean(&content, MAX_DESCRIPTION),
            Some("description") if preview.description.is_empty() => {
                preview.description = clean(&content, MAX_DESCRIPTION)
            }
            Some("og:site_name") => preview.site_name = clean(&content, 80),
            Some("og:image") | Some("og:image:url") if preview.image_url.is_none() => {
                // Only an absolute http(s) image is kept; the browser loads it, not the server.
                preview.image_url = page
                    .join(content.trim())
                    .ok()
                    .filter(|image| matches!(image.scheme(), "http" | "https"))
                    .map(|image| image.to_string());
            }
            _ => {}
        }
    }
    preview.title = og_title.unwrap_or_else(|| {
        lower
            .find("<title")
            .and_then(|start| lower[start..].find('>').map(|open| start + open + 1))
            .and_then(|from| {
                lower[from..]
                    .find("</title")
                    .map(|close| clean(&html[from..from + close], MAX_TITLE))
            })
            .unwrap_or_default()
    });
    (!preview.title.is_empty() || !preview.description.is_empty()).then_some(preview)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_graph_wins_and_falls_back_to_title_and_description() {
        let page = Url::parse("https://example.com/post/1").unwrap();
        let html = r#"<html><head><title>Plain &amp; title</title>
            <meta name="description" content="Fallback text">
            <META PROPERTY="og:title" CONTENT="OG &quot;title&quot;">
            <meta property='og:image' content='/img/cover.png'>
            <meta property="og:site_name" content="Example">
            </head></html>"#;
        let preview = parse_preview(html, &page).unwrap();
        assert_eq!(preview.title, "OG \"title\"");
        assert_eq!(preview.description, "Fallback text");
        assert_eq!(preview.site_name, "Example");
        assert_eq!(
            preview.image_url.as_deref(),
            Some("https://example.com/img/cover.png")
        );

        let bare = parse_preview("<title>\n  Only   title </title>", &page).unwrap();
        assert_eq!(bare.title, "Only title");
        assert_eq!(bare.site_name, "example.com");
        assert!(parse_preview("<p>nothing</p>", &page).is_none());
    }

    #[test]
    fn non_http_images_and_overlong_text_are_contained() {
        let page = Url::parse("https://example.com/").unwrap();
        let long = "x".repeat(1000);
        let html = format!(
            r#"<meta property="og:image" content="javascript:alert(1)"><meta property="og:title" content="{long}">"#
        );
        let preview = parse_preview(&html, &page).unwrap();
        assert!(preview.image_url.is_none());
        assert_eq!(preview.title.chars().count(), MAX_TITLE + 1);
        assert_eq!(
            decode_entities("a &#x4e2d;&#25991; &bogus; &"),
            "a 中文 &bogus; &"
        );
    }
}
