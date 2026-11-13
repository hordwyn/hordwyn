//! Markdown to HTML for the text pages (`CommonMark` plus tables and footnotes).

use pulldown_cmark::{Options, Parser, html};

/// Render `source` to HTML. The content directory is ours, so raw HTML in it is kept.
#[must_use]
pub fn render(source: &str) -> String {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES | Options::ENABLE_STRIKETHROUGH;
    let mut out = String::new();
    html::push_html(&mut out, Parser::new_ext(source, options));
    // a code block scrolls sideways on a narrow screen: the keyboard must reach it (WCAG 2.1.1)
    out.replace("<pre>", "<pre tabindex=\"0\">")
}

/// Render one line of Markdown without the paragraph around it.
#[must_use]
pub fn render_inline(source: &str) -> String {
    let html = render(source);
    let html = html.trim();
    html.strip_prefix("<p>")
        .and_then(|h| h.strip_suffix("</p>"))
        .unwrap_or(html)
        .to_string()
}

/// Escape text for an HTML attribute or element.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_lists_links_and_tables() {
        let html = render("- one\n- [two](/x/)\n\n| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(html.contains("<ul>"), "{html}");
        assert!(html.contains("<a href=\"/x/\">two</a>"), "{html}");
        assert!(html.contains("<table>"), "{html}");
    }

    #[test]
    fn code_blocks_can_be_scrolled_from_the_keyboard() {
        let html = render("```sh\nls\n```\n");
        assert!(html.contains("<pre tabindex=\"0\"><code"), "{html}");
    }

    #[test]
    fn an_inline_line_keeps_its_markup_and_loses_the_paragraph() {
        assert_eq!(
            render_inline("<i lang=\"ang\">a</i>: *b*"),
            "<i lang=\"ang\">a</i>: <em>b</em>"
        );
    }

    #[test]
    fn escapes_markup() {
        assert_eq!(escape("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&#39;");
    }
}
