//! MK Sigma's Learn section: articles on the procedures and on the ideas behind them, in each
//! language, written for MK Sigma. The Σ shows a calculation on the user's data; an article
//! explains what it means and when to use it.
//!
//! Articles are Markdown files under `articles/<language>/<id>.md`, embedded, in a small
//! subset: `# Title`, `## Section`, paragraphs (the first is the article's summary), `- ` lists,
//! `> ` notes, `**strong**`, `*emphasis*`, `` `syntax` ``, links `[text](id)` to another article
//! or `[text](dialog:ttest)` to a procedure's dialog, `$inline$` and `$$display$$` formulas in a
//! subset of LaTeX (see `tex`), and ```` ```syntax ```` blocks: an example to run on the sample
//! data. A test reads every article in every language, so nothing in one fails to show.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod tex;

use mksigma_i18n::Lang;
use mksigma_output::math::Math;

/// What an article is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// From a question to the procedure that answers it.
    Guide,
    Procedure,
    Concept,
}

/// Where a link goes.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Article(String),
    /// A procedure's dialog, by the ribbon's name for it (`descriptives`, `ttest`…).
    Dialog(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Strong(String),
    Emphasis(String),
    /// Syntax or a name, in a fixed-width font.
    Code(String),
    Link {
        text: String,
        target: Target,
    },
    Math(Math),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading(String),
    Paragraph(Vec<Inline>),
    List(Vec<Vec<Inline>>),
    Note(Vec<Inline>),
    Math(Math),
    /// Syntax to run on the sample data.
    Example(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Article {
    pub id: &'static str,
    pub kind: Kind,
    pub title: String,
    /// Its first paragraph, as plain text, for the list of articles.
    pub summary: String,
    pub blocks: Vec<Block>,
}

/// The dialogs a link can open, by name.
pub const DIALOGS: [&str; 14] = [
    "descriptives",
    "frequencies",
    "explore",
    "crosstabs",
    "ttest",
    "oneway",
    "correlations",
    "regression",
    "logistic",
    "nonparametric",
    "histogram",
    "bar",
    "scatter",
    "boxplot",
];

/// Every article: its id, kind, and its source in English and in Portuguese.
const SOURCES: [(&str, Kind, &str, &str); 5] = [
    ("which-test", Kind::Guide, include_str!("../articles/en/which-test.md"), include_str!("../articles/pt-BR/which-test.md")),
    ("descriptives", Kind::Procedure, include_str!("../articles/en/descriptives.md"), include_str!("../articles/pt-BR/descriptives.md")),
    ("t-test", Kind::Procedure, include_str!("../articles/en/t-test.md"), include_str!("../articles/pt-BR/t-test.md")),
    ("crosstabs", Kind::Procedure, include_str!("../articles/en/crosstabs.md"), include_str!("../articles/pt-BR/crosstabs.md")),
    ("p-value", Kind::Concept, include_str!("../articles/en/p-value.md"), include_str!("../articles/pt-BR/p-value.md")),
];

/// Every article in `lang`, in the order the Learn page lists them. One that can't be read
/// (the tests make sure none) is left out rather than shown broken.
pub fn articles(lang: Lang) -> Vec<Article> {
    SOURCES.iter().filter_map(|(id, kind, en, pt)| read(id, *kind, if lang == Lang::PtBr { pt } else { en }).ok()).collect()
}

/// The article `id` in `lang`.
pub fn article(lang: Lang, id: &str) -> Option<Article> {
    let (id, kind, en, pt) = SOURCES.iter().find(|(i, ..)| *i == id)?;
    read(id, *kind, if lang == Lang::PtBr { pt } else { en }).ok()
}

/// The article about a procedure, by its command's name (`DESCRIPTIVES`, `T-TEST`…), if one
/// has been written.
pub fn for_command(command: &str) -> Option<&'static str> {
    let id = match command.to_ascii_uppercase().as_str() {
        "DESCRIPTIVES" => "descriptives",
        "T-TEST" => "t-test",
        "CROSSTABS" => "crosstabs",
        _ => return None,
    };
    SOURCES.iter().find(|(i, ..)| *i == id).map(|(i, ..)| *i)
}

/// Reads an article; the error names the line it couldn't.
pub fn read(id: &'static str, kind: Kind, source: &str) -> Result<Article, String> {
    let mut title = None;
    let mut blocks = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let mut list: Vec<Vec<Inline>> = Vec::new();
    let mut example: Option<Vec<String>> = None;
    let flush = |paragraph: &mut Vec<String>, list: &mut Vec<Vec<Inline>>, blocks: &mut Vec<Block>| -> Result<(), String> {
        if !paragraph.is_empty() {
            blocks.push(Block::Paragraph(inlines(&paragraph.join(" "))?));
            paragraph.clear();
        }
        if !list.is_empty() {
            blocks.push(Block::List(std::mem::take(list)));
        }
        Ok(())
    };
    for (number, line) in source.lines().enumerate() {
        let at = |e: String| format!("{id}, line {}: {e}", number + 1);
        if let Some(lines) = example.as_mut() {
            if line.trim_start().starts_with("```") {
                blocks.push(Block::Example(lines.join("\n")));
                example = None;
            } else {
                lines.push(line.to_string());
            }
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            flush(&mut paragraph, &mut list, &mut blocks).map_err(at)?;
            example = Some(Vec::new());
        } else if trimmed.is_empty() {
            flush(&mut paragraph, &mut list, &mut blocks).map_err(at)?;
        } else if let Some(text) = trimmed.strip_prefix("# ") {
            title = Some(text.trim().to_string());
        } else if let Some(text) = trimmed.strip_prefix("## ") {
            flush(&mut paragraph, &mut list, &mut blocks).map_err(at)?;
            blocks.push(Block::Heading(text.trim().to_string()));
        } else if let Some(text) = trimmed.strip_prefix("- ") {
            if !paragraph.is_empty() {
                flush(&mut paragraph, &mut Vec::new(), &mut blocks).map_err(at)?;
            }
            list.push(inlines(text).map_err(at)?);
        } else if let Some(text) = trimmed.strip_prefix("> ") {
            flush(&mut paragraph, &mut list, &mut blocks).map_err(at)?;
            blocks.push(Block::Note(inlines(text).map_err(at)?));
        } else if let Some(formula) = trimmed.strip_prefix("$$").and_then(|t| t.strip_suffix("$$")) {
            flush(&mut paragraph, &mut list, &mut blocks).map_err(at)?;
            blocks.push(Block::Math(tex::parse(formula).map_err(at)?));
        } else if !list.is_empty() && line.starts_with("  ") {
            // A list item continued on an indented line.
            if let Some(last) = list.last_mut() {
                last.push(Inline::Text(" ".into()));
                last.extend(inlines(trimmed).map_err(at)?);
            }
        } else {
            if !list.is_empty() {
                flush(&mut Vec::new(), &mut list, &mut blocks).map_err(at)?;
            }
            paragraph.push(trimmed.to_string());
        }
    }
    if example.is_some() {
        return Err(format!("{id}: a ``` block that isn't closed"));
    }
    flush(&mut paragraph, &mut list, &mut blocks)?;
    let title = title.ok_or_else(|| format!("{id}: no # title"))?;
    let summary = blocks.iter().find_map(|b| if let Block::Paragraph(p) = b { Some(plain(p)) } else { None }).unwrap_or_default();
    Ok(Article { id, kind, title, summary, blocks })
}

/// The text of inline pieces, without their styles.
pub fn plain(pieces: &[Inline]) -> String {
    pieces
        .iter()
        .map(|p| match p {
            Inline::Text(s) | Inline::Strong(s) | Inline::Emphasis(s) | Inline::Code(s) => s.clone(),
            Inline::Link { text, .. } => text.clone(),
            Inline::Math(m) => m.to_latex(),
        })
        .collect()
}

/// A paragraph's text into its styled pieces.
fn inlines(text: &str) -> Result<Vec<Inline>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut plain_text = String::new();
    let mut i = 0;
    // The text up to `end` from `start`, and the index after it.
    let until = |start: usize, end: &str| -> Option<(String, usize)> {
        let rest: String = chars.get(start..)?.iter().collect();
        let k = rest.find(end)?;
        Some((rest[..k].to_string(), start + rest[..k].chars().count() + end.chars().count()))
    };
    while let Some(&c) = chars.get(i) {
        let piece = match c {
            '*' if chars.get(i + 1) == Some(&'*') => until(i + 2, "**").map(|(s, next)| (Inline::Strong(s), next)),
            '*' => until(i + 1, "*").map(|(s, next)| (Inline::Emphasis(s), next)),
            '`' => until(i + 1, "`").map(|(s, next)| (Inline::Code(s), next)),
            '$' => match until(i + 1, "$") {
                Some((s, next)) => Some((Inline::Math(tex::parse(&s)?), next)),
                None => None,
            },
            '[' => until(i + 1, "](").and_then(|(label, after)| {
                until(after, ")").map(|(target, next)| {
                    let target = match target.strip_prefix("dialog:") {
                        Some(kind) => Target::Dialog(kind.to_string()),
                        None => Target::Article(target),
                    };
                    (Inline::Link { text: label, target }, next)
                })
            }),
            _ => None,
        };
        match piece {
            Some((piece, next)) => {
                if !plain_text.is_empty() {
                    out.push(Inline::Text(std::mem::take(&mut plain_text)));
                }
                out.push(piece);
                i = next;
            }
            None => {
                plain_text.push(c);
                i += 1;
            }
        }
    }
    if !plain_text.is_empty() {
        out.push(Inline::Text(plain_text));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links(blocks: &[Block]) -> Vec<Target> {
        let mut out = Vec::new();
        let mut take = |pieces: &[Inline]| {
            out.extend(pieces.iter().filter_map(|p| if let Inline::Link { target, .. } = p { Some(target.clone()) } else { None }));
        };
        for b in blocks {
            match b {
                Block::Paragraph(p) | Block::Note(p) => take(p),
                Block::List(items) => items.iter().for_each(|i| take(i)),
                _ => {}
            }
        }
        out
    }

    #[test]
    fn every_article_reads_in_every_language_and_links_to_what_exists() {
        for lang in Lang::ALL {
            for (id, kind, en, pt) in SOURCES {
                let article = read(id, kind, if lang == Lang::PtBr { pt } else { en }).unwrap_or_else(|e| panic!("{}: {e}", lang.code()));
                assert!(!article.summary.is_empty(), "{id} has a summary");
                for target in links(&article.blocks) {
                    match target {
                        Target::Article(to) => assert!(SOURCES.iter().any(|(i, ..)| *i == to), "{id} links to {to}"),
                        Target::Dialog(kind) => {
                            assert!(DIALOGS.contains(&kind.as_str()), "{id}: dialog {kind}")
                        }
                    }
                }
            }
            assert_eq!(articles(lang).len(), SOURCES.len());
        }
    }

    #[test]
    fn the_same_examples_in_every_language() {
        let examples = |source: &str| -> Vec<String> {
            read("x", Kind::Concept, source)
                .map(|a| a.blocks.into_iter().filter_map(|b| if let Block::Example(e) = b { Some(e) } else { None }).collect())
                .unwrap_or_default()
        };
        for (id, _, en, pt) in SOURCES {
            assert_eq!(examples(en), examples(pt), "{id}: syntax is the same in every language");
        }
    }

    /// Every example runs on the sample data, so its button never fails.
    #[test]
    fn every_example_runs_on_the_sample_data() {
        for (id, kind, en, _) in SOURCES {
            for block in read(id, kind, en).map(|a| a.blocks).unwrap_or_default() {
                if let Block::Example(syntax) = block {
                    let mut session = mksigma_engine::Session::with_sample().unwrap_or_default();
                    assert!(session.run_syntax(&syntax).is_ok(), "{id}: {syntax}");
                    assert!(!session.output.items.is_empty(), "{id}: {syntax} shows something");
                }
            }
        }
    }

    #[test]
    fn markdown_pieces() {
        let a = read(
            "x",
            Kind::Concept,
            "# T\n\nOne **two** *three* `CMD` [see](p-value) $x^{2}$ end.\n\n- a\n- b\n\n> note\n\n$$\\bar{x}$$\n\n```syntax\nDESCRIPTIVES age.\n```",
        )
        .unwrap();
        assert_eq!(a.title, "T");
        assert_eq!(a.summary, "One two three CMD see {x}^{2} end.");
        assert!(matches!(&a.blocks[1], Block::List(items) if items.len() == 2));
        assert!(matches!(&a.blocks[2], Block::Note(_)));
        assert!(matches!(&a.blocks[3], Block::Math(_)));
        assert_eq!(a.blocks[4], Block::Example("DESCRIPTIVES age.".into()));
        assert!(read("x", Kind::Concept, "# T\n\n$\\nope$").is_err());
    }
}
