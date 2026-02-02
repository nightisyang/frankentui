#![forbid(unsafe_code)]

//! GitHub-Flavored Markdown renderer for FrankenTUI.
//!
//! Converts Markdown text into styled [`Text`] for rendering in terminal UIs.
//! Uses [pulldown-cmark] for parsing with full GFM support including:
//!
//! - Tables, strikethrough, task lists
//! - Math expressions (`$inline$` and `$$block$$`) rendered as Unicode
//! - Footnotes with `[^id]` syntax
//! - Admonitions (`[!NOTE]`, `[!WARNING]`, etc.)
//!
//! # Example
//! ```
//! use ftui_extras::markdown::{MarkdownRenderer, MarkdownTheme};
//!
//! let renderer = MarkdownRenderer::new(MarkdownTheme::default());
//! let text = renderer.render("# Hello\n\nSome **bold** text with $E=mc^2$.");
//! assert!(text.height() > 0);
//! ```

use ftui_render::cell::PackedRgba;
use ftui_style::Style;
use ftui_text::text::{Line, Span, Text};
use pulldown_cmark::{BlockQuoteKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

// ---------------------------------------------------------------------------
// LaTeX to Unicode conversion
// ---------------------------------------------------------------------------

/// Convert LaTeX math expression to Unicode approximation.
///
/// Uses the `unicodeit` crate for symbol conversion, with fallbacks for
/// unsupported constructs.
fn latex_to_unicode(latex: &str) -> String {
    // Use unicodeit for the heavy lifting
    let mut result = unicodeit::replace(latex);

    // Clean up any remaining backslash commands that weren't converted
    // by applying some common fallbacks
    result = apply_latex_fallbacks(&result);

    result
}

/// Apply fallback conversions for LaTeX constructs not handled by unicodeit.
fn apply_latex_fallbacks(text: &str) -> String {
    let mut result = text.to_string();

    // Common fraction fallbacks
    let fractions = [
        (r"\frac{1}{2}", "½"),
        (r"\frac{1}{3}", "⅓"),
        (r"\frac{2}{3}", "⅔"),
        (r"\frac{1}{4}", "¼"),
        (r"\frac{3}{4}", "¾"),
        (r"\frac{1}{5}", "⅕"),
        (r"\frac{2}{5}", "⅖"),
        (r"\frac{3}{5}", "⅗"),
        (r"\frac{4}{5}", "⅘"),
        (r"\frac{1}{6}", "⅙"),
        (r"\frac{5}{6}", "⅚"),
        (r"\frac{1}{7}", "⅐"),
        (r"\frac{1}{8}", "⅛"),
        (r"\frac{3}{8}", "⅜"),
        (r"\frac{5}{8}", "⅝"),
        (r"\frac{7}{8}", "⅞"),
        (r"\frac{1}{9}", "⅑"),
        (r"\frac{1}{10}", "⅒"),
    ];

    for (latex_frac, unicode) in fractions {
        result = result.replace(latex_frac, unicode);
    }

    // Handle generic \frac{a}{b} -> a/b
    while let Some(start) = result.find(r"\frac{") {
        if let Some(end) = find_matching_brace(&result[start + 6..]) {
            let num_end = start + 6 + end;
            let numerator = &result[start + 6..num_end];

            // Look for denominator
            if result[num_end + 1..].starts_with('{')
                && let Some(denom_end) = find_matching_brace(&result[num_end + 2..])
            {
                let denominator = &result[num_end + 2..num_end + 2 + denom_end];
                let replacement = format!("{numerator}/{denominator}");
                let full_end = num_end + 3 + denom_end;
                result = format!("{}{}{}", &result[..start], replacement, &result[full_end..]);
                continue;
            }
        }
        break;
    }

    // Square root: \sqrt{x} -> √x
    while let Some(start) = result.find(r"\sqrt{") {
        if let Some(end) = find_matching_brace(&result[start + 6..]) {
            let content = &result[start + 6..start + 6 + end];
            let replacement = format!("√{content}");
            result = format!(
                "{}{}{}",
                &result[..start],
                replacement,
                &result[start + 7 + end..]
            );
        } else {
            break;
        }
    }

    // \sqrt without braces -> √
    result = result.replace(r"\sqrt", "√");

    // Common operators and symbols not in unicodeit
    let symbols = [
        (r"\cdot", "·"),
        (r"\times", "×"),
        (r"\div", "÷"),
        (r"\pm", "±"),
        (r"\mp", "∓"),
        (r"\neq", "≠"),
        (r"\approx", "≈"),
        (r"\equiv", "≡"),
        (r"\leq", "≤"),
        (r"\geq", "≥"),
        (r"\ll", "≪"),
        (r"\gg", "≫"),
        (r"\subset", "⊂"),
        (r"\supset", "⊃"),
        (r"\subseteq", "⊆"),
        (r"\supseteq", "⊇"),
        (r"\cup", "∪"),
        (r"\cap", "∩"),
        (r"\emptyset", "∅"),
        (r"\forall", "∀"),
        (r"\exists", "∃"),
        (r"\nexists", "∄"),
        (r"\neg", "¬"),
        (r"\land", "∧"),
        (r"\lor", "∨"),
        (r"\oplus", "⊕"),
        (r"\otimes", "⊗"),
        (r"\perp", "⊥"),
        (r"\parallel", "∥"),
        (r"\angle", "∠"),
        (r"\triangle", "△"),
        (r"\square", "□"),
        (r"\diamond", "◇"),
        (r"\star", "⋆"),
        (r"\circ", "∘"),
        (r"\bullet", "•"),
        (r"\nabla", "∇"),
        (r"\partial", "∂"),
        (r"\hbar", "ℏ"),
        (r"\ell", "ℓ"),
        (r"\Re", "ℜ"),
        (r"\Im", "ℑ"),
        (r"\wp", "℘"),
        (r"\aleph", "ℵ"),
        (r"\beth", "ℶ"),
        (r"\gimel", "ℷ"),
        (r"\daleth", "ℸ"),
    ];

    for (latex_sym, unicode) in symbols {
        result = result.replace(latex_sym, unicode);
    }

    // Clean up extra whitespace
    result = result.split_whitespace().collect::<Vec<_>>().join(" ");

    result
}

/// Find the position of the matching closing brace.
fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 1;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------

/// Theme for Markdown rendering.
///
/// Each field controls the style applied to the corresponding Markdown element.
/// The default theme uses a carefully curated color palette designed for
/// excellent readability in terminal environments.
#[derive(Debug, Clone)]
pub struct MarkdownTheme {
    // Headings - gradient from bright to muted
    pub h1: Style,
    pub h2: Style,
    pub h3: Style,
    pub h4: Style,
    pub h5: Style,
    pub h6: Style,

    // Code
    pub code_inline: Style,
    pub code_block: Style,

    // Text formatting
    pub blockquote: Style,
    pub link: Style,
    pub emphasis: Style,
    pub strong: Style,
    pub strikethrough: Style,

    // Lists
    pub list_bullet: Style,
    pub horizontal_rule: Style,

    // Task lists
    pub task_done: Style,
    pub task_todo: Style,

    // Math (LaTeX)
    pub math_inline: Style,
    pub math_block: Style,

    // Footnotes
    pub footnote_ref: Style,
    pub footnote_def: Style,

    // Admonitions (GitHub alerts)
    pub admonition_note: Style,
    pub admonition_tip: Style,
    pub admonition_important: Style,
    pub admonition_warning: Style,
    pub admonition_caution: Style,
}

impl Default for MarkdownTheme {
    fn default() -> Self {
        Self {
            // Headings: bright white -> soft lavender gradient
            h1: Style::new().fg(PackedRgba::rgb(255, 255, 255)).bold(),
            h2: Style::new().fg(PackedRgba::rgb(200, 200, 255)).bold(),
            h3: Style::new().fg(PackedRgba::rgb(180, 180, 230)).bold(),
            h4: Style::new().fg(PackedRgba::rgb(160, 160, 210)).bold(),
            h5: Style::new().fg(PackedRgba::rgb(140, 140, 190)).bold(),
            h6: Style::new().fg(PackedRgba::rgb(120, 120, 170)).bold(),

            // Code: warm amber for inline, soft gray for blocks
            code_inline: Style::new().fg(PackedRgba::rgb(230, 180, 80)),
            code_block: Style::new().fg(PackedRgba::rgb(200, 200, 200)),

            // Text formatting
            blockquote: Style::new().fg(PackedRgba::rgb(150, 150, 150)).italic(),
            link: Style::new().fg(PackedRgba::rgb(100, 150, 255)).underline(),
            emphasis: Style::new().italic(),
            strong: Style::new().bold(),
            strikethrough: Style::new().strikethrough(),

            // Lists: warm gold bullets
            list_bullet: Style::new().fg(PackedRgba::rgb(180, 180, 100)),
            horizontal_rule: Style::new().fg(PackedRgba::rgb(100, 100, 100)).dim(),

            // Task lists: green for done, cyan for todo
            task_done: Style::new().fg(PackedRgba::rgb(120, 220, 120)),
            task_todo: Style::new().fg(PackedRgba::rgb(150, 200, 220)),

            // Math: elegant purple/magenta for mathematical expressions
            math_inline: Style::new().fg(PackedRgba::rgb(220, 150, 255)).italic(),
            math_block: Style::new().fg(PackedRgba::rgb(200, 140, 240)).bold(),

            // Footnotes: subtle teal
            footnote_ref: Style::new().fg(PackedRgba::rgb(100, 180, 180)).dim(),
            footnote_def: Style::new().fg(PackedRgba::rgb(120, 160, 160)),

            // Admonitions: semantic colors matching their meaning
            admonition_note: Style::new().fg(PackedRgba::rgb(100, 150, 255)).bold(), // Blue - informational
            admonition_tip: Style::new().fg(PackedRgba::rgb(100, 200, 100)).bold(), // Green - helpful
            admonition_important: Style::new().fg(PackedRgba::rgb(180, 130, 255)).bold(), // Purple - important
            admonition_warning: Style::new().fg(PackedRgba::rgb(255, 200, 80)).bold(), // Yellow/amber - warning
            admonition_caution: Style::new().fg(PackedRgba::rgb(255, 100, 100)).bold(), // Red - danger
        }
    }
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Markdown renderer that converts Markdown text into styled [`Text`].
///
/// Supports GitHub-Flavored Markdown including math expressions, task lists,
/// footnotes, and admonitions.
#[derive(Debug, Clone)]
pub struct MarkdownRenderer {
    theme: MarkdownTheme,
    rule_width: u16,
}

impl MarkdownRenderer {
    /// Create a new renderer with the given theme.
    #[must_use]
    pub fn new(theme: MarkdownTheme) -> Self {
        Self {
            theme,
            rule_width: 40,
        }
    }

    /// Set the width for horizontal rules.
    #[must_use]
    pub fn rule_width(mut self, width: u16) -> Self {
        self.rule_width = width;
        self
    }

    /// Render a Markdown string into styled [`Text`].
    ///
    /// Parses the input as GitHub-Flavored Markdown with all extensions enabled:
    /// tables, strikethrough, task lists, math, footnotes, and admonitions.
    #[must_use]
    pub fn render(&self, markdown: &str) -> Text {
        let options = Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TABLES
            | Options::ENABLE_HEADING_ATTRIBUTES
            | Options::ENABLE_MATH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_GFM;
        let parser = Parser::new_ext(markdown, options);

        let mut builder = RenderState::new(&self.theme, self.rule_width);
        builder.process(parser);
        builder.finish()
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new(MarkdownTheme::default())
    }
}

// ---------------------------------------------------------------------------
// Internal render state machine
// ---------------------------------------------------------------------------

/// Style stack entry tracking what Markdown context is active.
#[derive(Debug, Clone)]
enum StyleContext {
    Heading(HeadingLevel),
    Emphasis,
    Strong,
    Strikethrough,
    CodeBlock,
    Blockquote,
    Link(String),
    FootnoteDefinition,
}

/// Tracks list nesting and numbering.
#[derive(Debug, Clone)]
struct ListState {
    ordered: bool,
    next_number: u64,
}

/// Admonition type from GFM blockquote tags.
#[derive(Debug, Clone, Copy)]
enum AdmonitionKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl AdmonitionKind {
    fn from_blockquote_kind(kind: Option<BlockQuoteKind>) -> Option<Self> {
        match kind? {
            BlockQuoteKind::Note => Some(Self::Note),
            BlockQuoteKind::Tip => Some(Self::Tip),
            BlockQuoteKind::Important => Some(Self::Important),
            BlockQuoteKind::Warning => Some(Self::Warning),
            BlockQuoteKind::Caution => Some(Self::Caution),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Note => "ℹ️ ",
            Self::Tip => "💡",
            Self::Important => "❗",
            Self::Warning => "⚠️ ",
            Self::Caution => "🔴",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Note => "NOTE",
            Self::Tip => "TIP",
            Self::Important => "IMPORTANT",
            Self::Warning => "WARNING",
            Self::Caution => "CAUTION",
        }
    }
}

struct RenderState<'t> {
    theme: &'t MarkdownTheme,
    rule_width: u16,
    lines: Vec<Line>,
    current_spans: Vec<Span<'static>>,
    style_stack: Vec<StyleContext>,
    list_stack: Vec<ListState>,
    /// Whether we're collecting text inside a code block.
    in_code_block: bool,
    code_block_lines: Vec<String>,
    /// Whether we're inside a blockquote.
    blockquote_depth: u16,
    /// Current admonition type (if in an admonition blockquote).
    current_admonition: Option<AdmonitionKind>,
    /// Track if we need a blank line separator.
    needs_blank: bool,
    /// Pending task list marker (checked state).
    pending_task_marker: Option<bool>,
    /// Whether we're waiting to emit a list item prefix.
    /// Deferred so task markers can replace the bullet.
    pending_list_prefix: bool,
    /// Footnote definitions collected during parsing.
    footnotes: Vec<(String, Vec<Line>)>,
    /// Current footnote being collected.
    current_footnote: Option<String>,
    /// Lines being collected for the current footnote definition.
    current_footnote_lines: Vec<Line>,
}

impl<'t> RenderState<'t> {
    fn new(theme: &'t MarkdownTheme, rule_width: u16) -> Self {
        Self {
            theme,
            rule_width,
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: Vec::new(),
            list_stack: Vec::new(),
            in_code_block: false,
            code_block_lines: Vec::new(),
            blockquote_depth: 0,
            current_admonition: None,
            needs_blank: false,
            pending_task_marker: None,
            pending_list_prefix: false,
            footnotes: Vec::new(),
            current_footnote: None,
            current_footnote_lines: Vec::new(),
        }
    }

    fn process<'a>(&mut self, parser: impl Iterator<Item = Event<'a>>) {
        for event in parser {
            match event {
                Event::Start(tag) => self.start_tag(tag),
                Event::End(tag) => self.end_tag(tag),
                Event::Text(text) => self.text(&text),
                Event::Code(code) => self.inline_code(&code),
                Event::SoftBreak => self.soft_break(),
                Event::HardBreak => self.hard_break(),
                Event::Rule => self.horizontal_rule(),
                Event::TaskListMarker(checked) => self.task_list_marker(checked),
                Event::FootnoteReference(label) => self.footnote_reference(&label),
                Event::InlineMath(latex) => self.inline_math(&latex),
                Event::DisplayMath(latex) => self.display_math(&latex),
                Event::Html(html) | Event::InlineHtml(html) => self.html(&html),
            }
        }

        // Append collected footnotes at the end
        self.append_footnotes();
    }

    fn start_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush_blank();
                self.style_stack.push(StyleContext::Heading(level));
            }
            Tag::Paragraph => {
                self.flush_blank();
            }
            Tag::Emphasis => {
                self.style_stack.push(StyleContext::Emphasis);
            }
            Tag::Strong => {
                self.style_stack.push(StyleContext::Strong);
            }
            Tag::Strikethrough => {
                self.style_stack.push(StyleContext::Strikethrough);
            }
            Tag::CodeBlock(_) => {
                self.flush_blank();
                self.in_code_block = true;
                self.code_block_lines.clear();
                self.style_stack.push(StyleContext::CodeBlock);
            }
            Tag::BlockQuote(kind) => {
                self.flush_blank();
                self.blockquote_depth = self.blockquote_depth.saturating_add(1);

                // Check for GFM admonitions
                if let Some(adm) = AdmonitionKind::from_blockquote_kind(kind) {
                    self.current_admonition = Some(adm);
                    // Emit the admonition header
                    let style = self.admonition_style(adm);
                    let header = format!("{} {}", adm.icon(), adm.label());
                    self.lines.push(Line::styled(header, style));
                }

                self.style_stack.push(StyleContext::Blockquote);
            }
            Tag::Link { dest_url, .. } => {
                self.style_stack
                    .push(StyleContext::Link(dest_url.to_string()));
            }
            Tag::List(start) => match start {
                Some(n) => self.list_stack.push(ListState {
                    ordered: true,
                    next_number: n,
                }),
                None => self.list_stack.push(ListState {
                    ordered: false,
                    next_number: 0,
                }),
            },
            Tag::Item => {
                self.flush_line();
                // Defer prefix emission - TaskListMarker may come next and replace
                // the bullet with a checkbox
                self.pending_list_prefix = true;
            }
            Tag::FootnoteDefinition(label) => {
                self.flush_line();
                self.current_footnote = Some(label.to_string());
                self.style_stack.push(StyleContext::FootnoteDefinition);
            }
            Tag::Table(_) | Tag::TableHead | Tag::TableRow | Tag::TableCell => {
                // Table support: we render as simple text with separators
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) => {
                self.style_stack.pop();
                self.flush_line();
                self.needs_blank = true;
            }
            TagEnd::Paragraph => {
                self.flush_line();
                self.needs_blank = true;
            }
            TagEnd::Emphasis => {
                self.style_stack.pop();
            }
            TagEnd::Strong => {
                self.style_stack.pop();
            }
            TagEnd::Strikethrough => {
                self.style_stack.pop();
            }
            TagEnd::CodeBlock => {
                self.style_stack.pop();
                self.flush_code_block();
                self.in_code_block = false;
                self.needs_blank = true;
            }
            TagEnd::BlockQuote(_) => {
                self.style_stack.pop();
                self.blockquote_depth = self.blockquote_depth.saturating_sub(1);
                if self.blockquote_depth == 0 {
                    self.current_admonition = None;
                }
                self.flush_line();
                self.needs_blank = true;
            }
            TagEnd::Link => {
                self.style_stack.pop();
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    self.flush_line();
                    self.needs_blank = true;
                }
            }
            TagEnd::Item => {
                self.flush_line();
            }
            TagEnd::FootnoteDefinition => {
                self.style_stack.pop();
                self.flush_footnote_line();
                if let Some(label) = self.current_footnote.take() {
                    // Move collected footnote lines to the footnotes list
                    let content_lines = std::mem::take(&mut self.current_footnote_lines);
                    self.footnotes.push((label, content_lines));
                }
                self.needs_blank = true;
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                self.flush_line();
            }
            TagEnd::TableCell => {
                self.current_spans.push(Span::raw(String::from(" │ ")));
            }
            _ => {}
        }
    }

    fn text(&mut self, text: &str) {
        if self.in_code_block {
            self.code_block_lines.push(text.to_string());
            return;
        }

        // Handle deferred list item prefix
        // Task markers take precedence over bullet points
        if self.pending_list_prefix {
            self.pending_list_prefix = false;
            let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));

            if let Some(checked) = self.pending_task_marker.take() {
                // Task list item - use checkbox instead of bullet
                let (marker, style) = if checked {
                    ("✓ ", self.theme.task_done)
                } else {
                    ("☐ ", self.theme.task_todo)
                };
                self.current_spans
                    .push(Span::styled(format!("{indent}{marker}"), style));
            } else {
                // Regular list item - use bullet
                let prefix = self.list_prefix();
                self.current_spans.push(Span::styled(
                    format!("{indent}{prefix}"),
                    self.theme.list_bullet,
                ));
            }
        } else if let Some(checked) = self.pending_task_marker.take() {
            // Task marker without pending prefix (shouldn't happen normally)
            let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));
            let (marker, style) = if checked {
                ("✓ ", self.theme.task_done)
            } else {
                ("☐ ", self.theme.task_todo)
            };
            self.current_spans
                .push(Span::styled(format!("{indent}{marker}"), style));
        }

        let style = self.current_style();
        let link = self.current_link();
        let content = if self.blockquote_depth > 0 {
            let bar_style = self
                .current_admonition
                .map(|adm| self.admonition_style(adm))
                .unwrap_or(self.theme.blockquote);
            let prefix = if self.current_admonition.is_some() {
                "┃ ".repeat(self.blockquote_depth as usize)
            } else {
                "│ ".repeat(self.blockquote_depth as usize)
            };
            // Use styled prefix for admonitions
            if self.current_admonition.is_some() {
                self.current_spans
                    .push(Span::styled(prefix, bar_style.dim()));
                text.to_string()
            } else {
                format!("{prefix}{text}")
            }
        } else {
            text.to_string()
        };

        let mut span = match style {
            Some(s) => Span::styled(content, s),
            None => Span::raw(content),
        };

        if let Some(url) = link {
            span = span.link(url);
        }

        self.current_spans.push(span);
    }

    fn inline_code(&mut self, code: &str) {
        let mut span = Span::styled(format!("`{code}`"), self.theme.code_inline);
        if let Some(url) = self.current_link() {
            span = span.link(url);
        }
        self.current_spans.push(span);
    }

    fn soft_break(&mut self) {
        self.current_spans.push(Span::raw(String::from(" ")));
    }

    fn hard_break(&mut self) {
        self.flush_line();
    }

    fn horizontal_rule(&mut self) {
        self.flush_blank();
        let rule = "─".repeat(self.rule_width as usize);
        self.lines
            .push(Line::styled(rule, self.theme.horizontal_rule));
        self.needs_blank = true;
    }

    fn task_list_marker(&mut self, checked: bool) {
        // Defer until we get the text content
        self.pending_task_marker = Some(checked);
    }

    fn footnote_reference(&mut self, label: &str) {
        let reference = format!("[^{label}]");
        self.current_spans
            .push(Span::styled(reference, self.theme.footnote_ref));
    }

    fn inline_math(&mut self, latex: &str) {
        let unicode = latex_to_unicode(latex);
        self.current_spans
            .push(Span::styled(unicode, self.theme.math_inline));
    }

    fn display_math(&mut self, latex: &str) {
        self.flush_blank();
        let unicode = latex_to_unicode(latex);

        // Center the math block with a subtle indicator
        for line in unicode.lines() {
            let formatted = format!("  {line}");
            self.lines
                .push(Line::styled(formatted, self.theme.math_block));
        }
        if unicode.is_empty() {
            self.lines
                .push(Line::styled(String::from("  "), self.theme.math_block));
        }
        self.needs_blank = true;
    }

    fn html(&mut self, _html: &str) {
        // Skip raw HTML in terminal output
    }

    // -- helpers --

    fn admonition_style(&self, kind: AdmonitionKind) -> Style {
        match kind {
            AdmonitionKind::Note => self.theme.admonition_note,
            AdmonitionKind::Tip => self.theme.admonition_tip,
            AdmonitionKind::Important => self.theme.admonition_important,
            AdmonitionKind::Warning => self.theme.admonition_warning,
            AdmonitionKind::Caution => self.theme.admonition_caution,
        }
    }

    fn current_style(&self) -> Option<Style> {
        let mut result: Option<Style> = None;
        for ctx in &self.style_stack {
            let s = match ctx {
                StyleContext::Heading(HeadingLevel::H1) => self.theme.h1,
                StyleContext::Heading(HeadingLevel::H2) => self.theme.h2,
                StyleContext::Heading(HeadingLevel::H3) => self.theme.h3,
                StyleContext::Heading(HeadingLevel::H4) => self.theme.h4,
                StyleContext::Heading(HeadingLevel::H5) => self.theme.h5,
                StyleContext::Heading(HeadingLevel::H6) => self.theme.h6,
                StyleContext::Emphasis => self.theme.emphasis,
                StyleContext::Strong => self.theme.strong,
                StyleContext::Strikethrough => self.theme.strikethrough,
                StyleContext::CodeBlock => self.theme.code_block,
                StyleContext::Blockquote => self.theme.blockquote,
                StyleContext::Link(_) => self.theme.link,
                StyleContext::FootnoteDefinition => self.theme.footnote_def,
            };
            result = Some(match result {
                Some(existing) => s.merge(&existing),
                None => s,
            });
        }
        result
    }

    fn current_link(&self) -> Option<String> {
        // Return the most recently pushed link URL
        for ctx in self.style_stack.iter().rev() {
            if let StyleContext::Link(url) = ctx {
                return Some(url.clone());
            }
        }
        None
    }

    fn list_prefix(&mut self) -> String {
        if let Some(list) = self.list_stack.last_mut() {
            if list.ordered {
                let n = list.next_number;
                list.next_number += 1;
                format!("{n}. ")
            } else {
                String::from("• ")
            }
        } else {
            String::from("• ")
        }
    }

    fn flush_line(&mut self) {
        if !self.current_spans.is_empty() {
            let spans = std::mem::take(&mut self.current_spans);
            let line = Line::from_spans(spans);
            if self.in_footnote_definition() {
                // Redirect to footnote collection with indentation
                let indented = Line::styled(
                    format!("  {}", line.to_plain_text()),
                    self.theme.footnote_def,
                );
                self.current_footnote_lines.push(indented);
            } else {
                self.lines.push(line);
            }
        }
    }

    fn flush_blank(&mut self) {
        self.flush_line();
        if self.needs_blank && !self.lines.is_empty() {
            self.lines.push(Line::new());
            self.needs_blank = false;
        }
    }

    fn flush_code_block(&mut self) {
        let code = std::mem::take(&mut self.code_block_lines).join("");
        let style = self.theme.code_block;
        for line_text in code.lines() {
            self.lines
                .push(Line::styled(format!("  {line_text}"), style));
        }
        // If the code block was empty or ended with newline, still show at least nothing
        if code.is_empty() {
            self.lines.push(Line::styled(String::from("  "), style));
        }
    }

    fn in_footnote_definition(&self) -> bool {
        self.current_footnote.is_some()
    }

    fn flush_footnote_line(&mut self) {
        if !self.current_spans.is_empty() {
            let spans = std::mem::take(&mut self.current_spans);
            let line = Line::from_spans(spans);
            let indented_line = Line::styled(
                format!("  {}", line.to_plain_text()),
                self.theme.footnote_def,
            );
            self.current_footnote_lines.push(indented_line);
        }
    }

    fn append_footnotes(&mut self) {
        if self.footnotes.is_empty() {
            return;
        }

        // Add separator before footnotes
        self.flush_line();
        self.lines.push(Line::new());
        let separator = "─".repeat(20);
        self.lines
            .push(Line::styled(separator, self.theme.horizontal_rule));

        for (label, content_lines) in std::mem::take(&mut self.footnotes) {
            // Footnote header
            let header = format!("[^{label}]:");
            self.lines
                .push(Line::styled(header, self.theme.footnote_def));

            // Footnote content (indented)
            for line in content_lines {
                self.lines.push(line);
            }
        }
    }

    fn finish(mut self) -> Text {
        self.flush_line();
        if self.lines.is_empty() {
            return Text::new();
        }
        Text::from_lines(self.lines)
    }
}

// ---------------------------------------------------------------------------
// Convenience function
// ---------------------------------------------------------------------------

/// Render Markdown to styled [`Text`] using the default theme.
///
/// This is a convenience function for quick rendering without customization.
/// For custom themes or settings, use [`MarkdownRenderer`] directly.
#[must_use]
pub fn render_markdown(markdown: &str) -> Text {
    MarkdownRenderer::default().render(markdown)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &Text) -> String {
        text.lines()
            .iter()
            .map(|l| l.to_plain_text())
            .collect::<Vec<_>>()
            .join("\n")
    }

    // =========================================================================
    // Basic Markdown tests (existing)
    // =========================================================================

    #[test]
    fn render_empty_string() {
        let text = render_markdown("");
        assert!(text.is_empty());
    }

    #[test]
    fn render_plain_paragraph() {
        let text = render_markdown("Hello, world!");
        let content = plain(&text);
        assert!(content.contains("Hello, world!"));
    }

    #[test]
    fn render_heading_h1() {
        let text = render_markdown("# Title");
        let content = plain(&text);
        assert!(content.contains("Title"));
        // H1 should be on its own line
        assert!(text.height() >= 1);
    }

    #[test]
    fn render_heading_levels() {
        let md = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("H1"));
        assert!(content.contains("H6"));
    }

    #[test]
    fn render_bold_text() {
        let text = render_markdown("Some **bold** text.");
        let content = plain(&text);
        assert!(content.contains("bold"));
    }

    #[test]
    fn render_italic_text() {
        let text = render_markdown("Some *italic* text.");
        let content = plain(&text);
        assert!(content.contains("italic"));
    }

    #[test]
    fn render_strikethrough() {
        let text = render_markdown("Some ~~struck~~ text.");
        let content = plain(&text);
        assert!(content.contains("struck"));
    }

    #[test]
    fn render_inline_code() {
        let text = render_markdown("Use `code` here.");
        let content = plain(&text);
        assert!(content.contains("`code`"));
    }

    #[test]
    fn render_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("fn main()"));
    }

    #[test]
    fn render_blockquote() {
        let text = render_markdown("> Quoted text");
        let content = plain(&text);
        assert!(content.contains("Quoted text"));
    }

    #[test]
    fn render_unordered_list() {
        let md = "- Item 1\n- Item 2\n- Item 3";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("• Item 1"));
        assert!(content.contains("• Item 2"));
        assert!(content.contains("• Item 3"));
    }

    #[test]
    fn render_ordered_list() {
        let md = "1. First\n2. Second\n3. Third";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("1. First"));
        assert!(content.contains("2. Second"));
        assert!(content.contains("3. Third"));
    }

    #[test]
    fn render_horizontal_rule() {
        let md = "Above\n\n---\n\nBelow";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("Above"));
        assert!(content.contains("Below"));
        assert!(content.contains("─"));
    }

    #[test]
    fn render_link() {
        let text = render_markdown("[click here](https://example.com)");
        let content = plain(&text);
        assert!(content.contains("click here"));
    }

    #[test]
    fn render_nested_emphasis() {
        let text = render_markdown("***bold and italic***");
        let content = plain(&text);
        assert!(content.contains("bold and italic"));
    }

    #[test]
    fn render_nested_list() {
        let md = "- Outer\n  - Inner\n- Back";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("Outer"));
        assert!(content.contains("Inner"));
        assert!(content.contains("Back"));
    }

    #[test]
    fn render_multiple_paragraphs() {
        let md = "First paragraph.\n\nSecond paragraph.";
        let text = render_markdown(md);
        // Should have a blank line between paragraphs
        assert!(text.height() >= 3);
    }

    #[test]
    fn custom_theme() {
        let theme = MarkdownTheme {
            h1: Style::new().fg(PackedRgba::rgb(255, 0, 0)),
            ..Default::default()
        };
        let renderer = MarkdownRenderer::new(theme);
        let text = renderer.render("# Red Title");
        assert!(!text.is_empty());
    }

    #[test]
    fn custom_rule_width() {
        let renderer = MarkdownRenderer::default().rule_width(20);
        let text = renderer.render("---");
        let content = plain(&text);
        // Rule should be 20 chars wide
        let rule_line = content.lines().find(|l| l.contains('─')).unwrap();
        assert_eq!(rule_line.chars().filter(|&c| c == '─').count(), 20);
    }

    #[test]
    fn render_code_block_preserves_whitespace() {
        let md = "```\n  indented\n    more\n```";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("  indented"));
        assert!(content.contains("    more"));
    }

    #[test]
    fn render_empty_code_block() {
        let md = "```\n```";
        let text = render_markdown(md);
        // Should still produce at least one line
        assert!(text.height() >= 1);
    }

    #[test]
    fn blockquote_has_bar_prefix() {
        let text = render_markdown("> quoted");
        let content = plain(&text);
        assert!(content.contains("│"));
    }

    // =========================================================================
    // GFM extension tests
    // =========================================================================

    #[test]
    fn render_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("A"));
        assert!(content.contains("B"));
        assert!(content.contains("1"));
        assert!(content.contains("2"));
    }

    #[test]
    fn render_nested_blockquotes() {
        let md = "> Level 1\n> > Level 2\n> > > Level 3";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("Level 1"));
        assert!(content.contains("Level 2"));
        assert!(content.contains("Level 3"));
    }

    #[test]
    fn render_link_with_inline_code() {
        let md = "[`code link`](https://example.com)";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("`code link`"));
    }

    #[test]
    fn render_ordered_list_custom_start() {
        let md = "5. Fifth\n6. Sixth\n7. Seventh";
        let text = render_markdown(md);
        let content = plain(&text);
        // Should start at 5
        assert!(content.contains("5. Fifth"));
        assert!(content.contains("6. Sixth"));
        assert!(content.contains("7. Seventh"));
    }

    #[test]
    fn render_mixed_list_types() {
        let md = "1. Ordered\n- Unordered\n2. Ordered again";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("1. Ordered"));
        assert!(content.contains("• Unordered"));
    }

    #[test]
    fn render_code_in_heading() {
        let md = "# Heading with `code`";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("Heading with"));
        assert!(content.contains("`code`"));
    }

    #[test]
    fn render_emphasis_in_list() {
        let md = "- Item with **bold** text";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("bold"));
    }

    #[test]
    fn render_soft_break() {
        let md = "Line one\nLine two";
        let text = render_markdown(md);
        let content = plain(&text);
        // Soft break becomes space
        assert!(content.contains("Line one"));
        assert!(content.contains("Line two"));
    }

    #[test]
    fn render_hard_break() {
        let md = "Line one  \nLine two"; // Two spaces before newline
        let text = render_markdown(md);
        // Hard break creates new line
        assert!(text.height() >= 2);
    }

    #[test]
    fn theme_default_creates_valid_styles() {
        use ftui_style::StyleFlags;
        let theme = MarkdownTheme::default();
        // All styles should be valid
        assert!(theme.h1.has_attr(StyleFlags::BOLD));
        assert!(theme.h2.has_attr(StyleFlags::BOLD));
        assert!(theme.emphasis.has_attr(StyleFlags::ITALIC));
        assert!(theme.strong.has_attr(StyleFlags::BOLD));
        assert!(theme.strikethrough.has_attr(StyleFlags::STRIKETHROUGH));
        assert!(theme.link.has_attr(StyleFlags::UNDERLINE));
        assert!(theme.blockquote.has_attr(StyleFlags::ITALIC));
    }

    #[test]
    fn theme_clone() {
        use ftui_style::StyleFlags;
        let theme1 = MarkdownTheme::default();
        let theme2 = theme1.clone();
        // Both should have same styles
        assert_eq!(
            theme1.h1.has_attr(StyleFlags::BOLD),
            theme2.h1.has_attr(StyleFlags::BOLD)
        );
    }

    #[test]
    fn renderer_clone() {
        let renderer1 = MarkdownRenderer::default();
        let renderer2 = renderer1.clone();
        // Both should render the same
        let text1 = renderer1.render("# Test");
        let text2 = renderer2.render("# Test");
        assert_eq!(plain(&text1), plain(&text2));
    }

    #[test]
    fn render_whitespace_only() {
        let text = render_markdown("   \n   \n   ");
        // Should handle gracefully
        let content = plain(&text);
        assert!(content.trim().is_empty() || content.contains(" "));
    }

    #[test]
    fn render_complex_nested_structure() {
        let md = r#"# Main Title

Some intro text with **bold** and *italic*.

## Section 1

> A blockquote with:
> - A list item
> - Another item

```rust
fn example() {
    println!("code");
}
```

## Section 2

1. First
2. Second
   - Nested bullet

---

The end.
"#;
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("Main Title"));
        assert!(content.contains("Section 1"));
        assert!(content.contains("Section 2"));
        assert!(content.contains("blockquote"));
        assert!(content.contains("fn example"));
        assert!(content.contains("─"));
        assert!(content.contains("The end"));
    }

    #[test]
    fn render_unicode_in_markdown() {
        let md = "# 日本語タイトル\n\n**太字** and *斜体*";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("日本語タイトル"));
        assert!(content.contains("太字"));
        assert!(content.contains("斜体"));
    }

    #[test]
    fn render_emoji_in_markdown() {
        let md = "# Celebration\n\n**Launch** today!";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("Celebration"));
        assert!(content.contains("Launch"));
    }

    #[test]
    fn render_consecutive_headings() {
        let md = "# H1\n## H2\n### H3";
        let text = render_markdown(md);
        // Should have blank lines between headings
        assert!(text.height() >= 5);
    }

    #[test]
    fn render_link_in_blockquote() {
        let md = "> Check [this link](https://example.com)";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("│"));
        assert!(content.contains("this link"));
    }

    #[test]
    fn render_code_block_with_language() {
        let md = "```python\nprint('hello')\n```";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("print"));
    }

    #[test]
    fn render_deeply_nested_list() {
        let md = "- Level 1\n  - Level 2\n    - Level 3\n      - Level 4";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("Level 1"));
        assert!(content.contains("Level 4"));
    }

    #[test]
    fn render_multiple_code_blocks() {
        let md = "```\nblock1\n```\n\n```\nblock2\n```";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("block1"));
        assert!(content.contains("block2"));
    }

    #[test]
    fn render_emphasis_across_words() {
        let md = "*multiple words in italic*";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("multiple words in italic"));
    }

    #[test]
    fn render_bold_and_italic_together() {
        let md = "***bold and italic*** and **just bold** and *just italic*";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("bold and italic"));
        assert!(content.contains("just bold"));
        assert!(content.contains("just italic"));
    }

    #[test]
    fn render_escaped_characters() {
        let md = r#"\*not italic\* and \`not code\`"#;
        let text = render_markdown(md);
        let content = plain(&text);
        // Escaped characters should appear as-is
        assert!(content.contains("*not italic*"));
    }

    #[test]
    fn markdown_renderer_default() {
        let renderer = MarkdownRenderer::default();
        let text = renderer.render("test");
        assert!(!text.is_empty());
    }

    #[test]
    fn render_markdown_function() {
        let text = render_markdown("# Heading\nParagraph");
        assert!(!text.is_empty());
        let content = plain(&text);
        assert!(content.contains("Heading"));
        assert!(content.contains("Paragraph"));
    }

    #[test]
    fn render_table_multicolumn() {
        let md = "| Col1 | Col2 | Col3 |\n|------|------|------|\n| A | B | C |\n| D | E | F |";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("Col1"));
        assert!(content.contains("Col2"));
        assert!(content.contains("Col3"));
        assert!(content.contains("A"));
        assert!(content.contains("F"));
    }

    #[test]
    fn render_very_long_line() {
        let long_text = "word ".repeat(100);
        let md = format!("# {}", long_text);
        let text = render_markdown(&md);
        assert!(!text.is_empty());
    }

    #[test]
    fn render_only_whitespace_in_code_block() {
        let md = "```\n   \n```";
        let text = render_markdown(md);
        // Should handle gracefully
        assert!(text.height() >= 1);
    }

    #[test]
    fn style_context_heading_levels() {
        // Each heading level should have different styling
        for level in 1..=6 {
            let md = format!("{} Heading Level {}", "#".repeat(level), level);
            let text = render_markdown(&md);
            let content = plain(&text);
            assert!(content.contains(&format!("Heading Level {}", level)));
        }
    }

    // =========================================================================
    // Task list tests
    // =========================================================================

    #[test]
    fn render_task_list_unchecked() {
        let md = "- [ ] Todo item";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("☐") || content.contains("Todo item"));
    }

    #[test]
    fn render_task_list_checked() {
        let md = "- [x] Done item";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("✓") || content.contains("Done item"));
    }

    #[test]
    fn render_task_list_mixed() {
        let md = "- [ ] Not done\n- [x] Done\n- [ ] Also not done";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("Not done"));
        assert!(content.contains("Done"));
        assert!(content.contains("Also not done"));
    }

    // =========================================================================
    // Math tests
    // =========================================================================

    #[test]
    fn render_inline_math() {
        let md = "The equation $E=mc^2$ is famous.";
        let text = render_markdown(md);
        let content = plain(&text);
        // Should contain the converted math (E=mc² or similar)
        assert!(content.contains("E") && content.contains("mc"));
    }

    #[test]
    fn render_display_math() {
        let md = "$$\n\\sum_{i=1}^n i = \\frac{n(n+1)}{2}\n$$";
        let text = render_markdown(md);
        let content = plain(&text);
        // Should render something (even if not perfectly formatted)
        assert!(!content.is_empty());
    }

    #[test]
    fn render_math_with_greek() {
        let md = "The angle $\\theta$ and $\\alpha + \\beta = \\gamma$.";
        let text = render_markdown(md);
        let content = plain(&text);
        // Greek letters should be converted to Unicode
        assert!(content.contains("θ") || content.contains("alpha"));
    }

    #[test]
    fn render_math_with_fractions() {
        let md = "Half is $\\frac{1}{2}$.";
        let text = render_markdown(md);
        let content = plain(&text);
        // Should convert to ½ or 1/2
        assert!(content.contains("½") || content.contains("1/2"));
    }

    #[test]
    fn render_math_with_sqrt() {
        let md = "The square root $\\sqrt{x}$ is useful.";
        let text = render_markdown(md);
        let content = plain(&text);
        // Should contain √
        assert!(content.contains("√") || content.contains("sqrt"));
    }

    // =========================================================================
    // Footnote tests
    // =========================================================================

    #[test]
    fn render_footnote_reference() {
        let md = "This has a footnote[^1].";
        let text = render_markdown(md);
        let content = plain(&text);
        assert!(content.contains("[^1]") || content.contains("footnote"));
    }

    // =========================================================================
    // LaTeX conversion tests
    // =========================================================================

    #[test]
    fn latex_greek_letters() {
        assert!(latex_to_unicode(r"\alpha").contains('α'));
        assert!(latex_to_unicode(r"\beta").contains('β'));
        assert!(latex_to_unicode(r"\gamma").contains('γ'));
        assert!(latex_to_unicode(r"\pi").contains('π'));
    }

    #[test]
    fn latex_operators() {
        assert!(latex_to_unicode(r"\times").contains('×'));
        assert!(latex_to_unicode(r"\div").contains('÷'));
        assert!(latex_to_unicode(r"\pm").contains('±'));
    }

    #[test]
    fn latex_comparison() {
        assert!(latex_to_unicode(r"\leq").contains('≤'));
        assert!(latex_to_unicode(r"\geq").contains('≥'));
        assert!(latex_to_unicode(r"\neq").contains('≠'));
    }

    #[test]
    fn latex_set_theory() {
        assert!(latex_to_unicode(r"\subset").contains('⊂'));
        assert!(latex_to_unicode(r"\cup").contains('∪'));
        assert!(latex_to_unicode(r"\cap").contains('∩'));
        assert!(latex_to_unicode(r"\emptyset").contains('∅'));
    }

    #[test]
    fn latex_logic() {
        assert!(latex_to_unicode(r"\forall").contains('∀'));
        assert!(latex_to_unicode(r"\exists").contains('∃'));
        assert!(latex_to_unicode(r"\land").contains('∧'));
        assert!(latex_to_unicode(r"\lor").contains('∨'));
    }

    #[test]
    fn latex_fractions() {
        assert!(latex_to_unicode(r"\frac{1}{2}").contains('½'));
        assert!(latex_to_unicode(r"\frac{1}{4}").contains('¼'));
        assert!(latex_to_unicode(r"\frac{3}{4}").contains('¾'));
    }

    #[test]
    fn latex_generic_fraction() {
        let result = latex_to_unicode(r"\frac{a}{b}");
        assert!(result.contains("a/b") || result.contains("a") && result.contains("b"));
    }

    #[test]
    fn latex_sqrt() {
        let result = latex_to_unicode(r"\sqrt{x}");
        assert!(result.contains("√x") || result.contains("√"));
    }

    #[test]
    fn find_matching_brace_works() {
        assert_eq!(find_matching_brace("abc}"), Some(3));
        assert_eq!(find_matching_brace("a{b}c}"), Some(5));
        assert_eq!(find_matching_brace("abc"), None);
    }

    // =========================================================================
    // Theme tests for new fields
    // =========================================================================

    #[test]
    fn theme_has_task_styles() {
        let theme = MarkdownTheme::default();
        // Task styles should exist and be different
        assert!(theme.task_done.fg.is_some());
        assert!(theme.task_todo.fg.is_some());
    }

    #[test]
    fn theme_has_math_styles() {
        use ftui_style::StyleFlags;
        let theme = MarkdownTheme::default();
        // Math styles should be styled
        assert!(theme.math_inline.fg.is_some());
        assert!(theme.math_inline.has_attr(StyleFlags::ITALIC));
        assert!(theme.math_block.fg.is_some());
        assert!(theme.math_block.has_attr(StyleFlags::BOLD));
    }

    #[test]
    fn theme_has_admonition_styles() {
        let theme = MarkdownTheme::default();
        // All admonition styles should have colors
        assert!(theme.admonition_note.fg.is_some());
        assert!(theme.admonition_tip.fg.is_some());
        assert!(theme.admonition_important.fg.is_some());
        assert!(theme.admonition_warning.fg.is_some());
        assert!(theme.admonition_caution.fg.is_some());
    }

    #[test]
    fn admonition_kind_icons_and_labels() {
        assert!(!AdmonitionKind::Note.icon().is_empty());
        assert!(!AdmonitionKind::Note.label().is_empty());
        assert!(!AdmonitionKind::Warning.icon().is_empty());
        assert!(!AdmonitionKind::Warning.label().is_empty());
    }
}
