//! Language-aware recursive text splitter.
//!
//! Inspired by cocoindex-code's RecursiveSplitter: splits on natural code
//! boundaries (blank lines, closing braces) before resorting to character
//! boundaries. Tracks start/end line numbers for each chunk.

/// A raw text chunk with line tracking before embedding.
#[derive(Debug, Clone)]
pub struct RawChunk {
    pub content: String,
    /// 1-indexed start line in the original file
    pub start_line: usize,
    /// 1-indexed end line in the original file (inclusive)
    pub end_line: usize,
    /// Searchable header prepended before embedding. `None` for chunks from
    /// non-AST paths (markdown, plain text). Not returned in search results.
    pub metadata: Option<String>,
}

/// Split source text into overlapping chunks.
///
/// # Parameters
/// - `source`       — full file text
/// - `chunk_size`   — target max chars per chunk (default 4000)
/// - `chunk_overlap`— overlap between consecutive chunks (default 400)
pub fn split(source: &str, chunk_size: usize, chunk_overlap: usize) -> Vec<RawChunk> {
    if source.is_empty() {
        return vec![];
    }

    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() {
        return vec![];
    }

    let mut chunks = vec![];
    let mut start_line = 0usize; // 0-indexed into `lines`

    while start_line < lines.len() {
        let mut end_line = start_line;
        let mut char_count = 0;

        // Accumulate lines until we hit chunk_size
        while end_line < lines.len() {
            let line_len = lines[end_line].len() + 1; // +1 for newline
            if char_count + line_len > chunk_size && end_line > start_line {
                break;
            }
            char_count += line_len;
            end_line += 1;
        }

        // Build the chunk content
        let content = lines[start_line..end_line].join("\n");

        chunks.push(RawChunk {
            content,
            start_line: start_line + 1, // convert to 1-indexed
            end_line,                   // end_line is exclusive → last included line
            metadata: None,
        });

        // If this chunk reached the end of the file, we're done.
        // Without this guard, the overlap logic would generate tiny extra chunks.
        if end_line >= lines.len() {
            break;
        }

        // Advance, backing up by overlap lines
        let overlap_lines = estimate_overlap_lines(&lines[start_line..end_line], chunk_overlap);
        let advance = (end_line - start_line).saturating_sub(overlap_lines).max(1);
        start_line += advance;
    }

    chunks
}

/// Split markdown by heading boundaries, then apply character limits.
///
/// Heading levels 1..=3 start a new section. For a caller that needs deeper
/// headings to split — the librarian's entry ledgers define entries at `####` —
/// use [`split_markdown_with_depth`]. The default is 3 and MUST stay 3: the
/// code index's `chunk_id` encodes `start_line`, so widening it here silently
/// invalidates every existing code chunk.
pub fn split_markdown(source: &str, chunk_size: usize, chunk_overlap: usize) -> Vec<RawChunk> {
    split_markdown_with_depth(source, chunk_size, chunk_overlap, 3)
}

/// [`split_markdown`], with the heading depth that starts a new section made
/// explicit. `max_heading_depth` is clamped to 1..=6.
pub fn split_markdown_with_depth(
    source: &str,
    chunk_size: usize,
    chunk_overlap: usize,
    max_heading_depth: usize,
) -> Vec<RawChunk> {
    if source.is_empty() {
        return vec![];
    }
    let depth = max_heading_depth.clamp(1, 6);

    let lines: Vec<&str> = source.lines().collect();
    let mut sections: Vec<(usize, usize)> = vec![];
    let mut section_start = 0;

    for (i, line) in lines.iter().enumerate() {
        if i > 0 && heading_level(line).is_some_and(|l| l <= depth) {
            sections.push((section_start, i));
            section_start = i;
        }
    }
    sections.push((section_start, lines.len()));

    let mut chunks = vec![];
    for (start, end) in sections {
        let section_text = lines[start..end].join("\n");
        if section_text.len() <= chunk_size {
            chunks.push(RawChunk {
                content: section_text,
                start_line: start + 1,
                end_line: end,
                metadata: None,
            });
        } else {
            let sub_chunks = split(&section_text, chunk_size, chunk_overlap);
            for mut sc in sub_chunks {
                sc.start_line += start;
                sc.end_line += start;
                chunks.push(sc);
            }
        }
    }
    chunks
}

/// ATX heading level of `line` (1..=6), or `None` when it is not a heading.
/// Requires the space after the hashes, so `#hashtag` is not a heading.
fn heading_level(line: &str) -> Option<usize> {
    let stripped = line.trim_start_matches('#');
    let hashes = line.len() - stripped.len();
    (1..=6)
        .contains(&hashes)
        .then_some(hashes)
        .filter(|_| stripped.starts_with(' '))
}

/// Split a markdown document into chunks bounded by an approximate token budget.
/// Splits on blank lines / `^#{1,6}` headings first for locality, then subdivides
/// sections that exceed `max_tokens` using the same generic text chunker as code.
pub fn chunk_markdown(text: &str, max_tokens: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![];
    }

    // Approximate chars per token budget (conservative: 4 chars/token).
    let max_chars = max_tokens * 4;

    // Pass 1: split on heading boundaries (always a new section) and blank-line
    // paragraph boundaries. Headings always force a new section even if small.
    let lines: Vec<&str> = text.lines().collect();
    let mut sections: Vec<String> = vec![];
    let mut current: Vec<&str> = vec![];

    for line in &lines {
        let is_heading = {
            let stripped = line.trim_start_matches('#');
            let hashes = line.len() - stripped.len();
            (1..=6).contains(&hashes) && stripped.starts_with(' ')
        };

        if is_heading && !current.is_empty() {
            // Flush current section, start new one with this heading
            let section = current.join("\n");
            if !section.trim().is_empty() {
                sections.push(section);
            }
            current = vec![*line];
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        let section = current.join("\n");
        if !section.trim().is_empty() {
            sections.push(section);
        }
    }

    // Pass 2: subdivide sections that exceed max_chars.
    // Use character-level slicing on word boundaries (space), since section
    // content may be a single long line with no internal newlines.
    let mut result: Vec<String> = vec![];
    for section in sections {
        if section.len() <= max_chars {
            result.push(section);
        } else {
            // Try line-based split first; if the section is a single line,
            // fall back to character-level word-boundary splitting.
            let sub_chunks = split(&section, max_chars, 0);
            if sub_chunks.len() > 1 {
                for sc in sub_chunks {
                    result.push(sc.content);
                }
            } else {
                // Single-line section: split at word boundaries every max_chars chars.
                let mut remaining = section.as_str();
                while remaining.len() > max_chars {
                    // Find a space at or before max_chars to split cleanly.
                    let split_at = remaining[..max_chars]
                        .rfind(' ')
                        .map(|pos| pos + 1)
                        .unwrap_or(max_chars);
                    result.push(remaining[..split_at].trim_end().to_string());
                    remaining = remaining[split_at..].trim_start();
                }
                if !remaining.is_empty() {
                    result.push(remaining.to_string());
                }
            }
        }
    }

    result
}

/// Estimate how many lines correspond to `overlap_chars` characters.
fn estimate_overlap_lines(lines: &[&str], overlap_chars: usize) -> usize {
    if overlap_chars == 0 {
        return 0;
    }
    let mut chars = 0;
    let mut count = 0;
    for line in lines.iter().rev() {
        chars += line.len() + 1;
        count += 1;
        if chars >= overlap_chars {
            break;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_no_chunks() {
        assert!(split("", 200, 20).is_empty());
    }

    #[test]
    fn short_text_is_single_chunk() {
        let source = "fn main() {\n    println!(\"hello\");\n}";
        let chunks = split(source, 4000, 400);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 3);
        assert_eq!(chunks[0].content, source);
    }

    #[test]
    fn first_chunk_starts_at_line_one() {
        let source = (0..100)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = split(&source, 200, 20);
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].start_line, 1);
    }

    #[test]
    fn consecutive_chunks_overlap() {
        let source = (0..100)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = split(&source, 200, 20);
        if chunks.len() > 1 {
            // Second chunk starts before first chunk ends → overlap
            assert!(chunks[1].start_line < chunks[0].end_line);
        }
    }

    #[test]
    fn all_lines_are_covered() {
        let source = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj";
        let chunks = split(source, 20, 5);
        // Every line should appear in at least one chunk
        for line_num in 1..=10usize {
            let covered = chunks
                .iter()
                .any(|c| c.start_line <= line_num && line_num <= c.end_line);
            assert!(covered, "line {} not covered by any chunk", line_num);
        }
    }

    #[test]
    fn chunk_content_matches_line_numbers() {
        let lines: Vec<String> = (1..=20).map(|i| format!("line_{:02}", i)).collect();
        let source = lines.join("\n");
        let chunks = split(&source, 100, 10);
        for chunk in &chunks {
            let expected = lines[chunk.start_line - 1..chunk.end_line].join("\n");
            assert_eq!(
                chunk.content, expected,
                "chunk [{}-{}] content mismatch",
                chunk.start_line, chunk.end_line
            );
        }
    }

    #[test]
    fn markdown_splits_on_headings() {
        let source = "# Title\n\nIntro text.\n\n## Section One\n\nContent one.\n\n## Section Two\n\nContent two.\n\n### Subsection\n\nMore content.\n";
        let chunks = split_markdown(source, 500, 50);
        // Should have at least 3 chunks (title+intro, section one, section two+subsection or separate)
        assert!(
            chunks.len() >= 3,
            "got {} chunks: {:?}",
            chunks.len(),
            chunks
                .iter()
                .map(|c| &c.content[..c.content.len().min(40)])
                .collect::<Vec<_>>()
        );
        // First chunk should contain "Title"
        assert!(chunks[0].content.contains("Title"));
        // Sections should be in separate chunks
        assert!(chunks.iter().any(|c| c.content.contains("Section One")));
        assert!(chunks.iter().any(|c| c.content.contains("Section Two")));
    }

    #[test]
    fn markdown_large_section_gets_subsplit() {
        // Create a section larger than chunk_size
        let big_section = (0..100)
            .map(|i| format!("Line {} of big section", i))
            .collect::<Vec<_>>()
            .join("\n");
        let source = format!(
            "# Title\n\n## Big Section\n\n{}\n\n## Small Section\n\nJust a few words.\n",
            big_section
        );
        let chunks = split_markdown(&source, 200, 20);
        // Big section should be split into multiple chunks
        assert!(
            chunks.len() > 2,
            "big section should be sub-split, got {} chunks",
            chunks.len()
        );
        // Small section should still be its own chunk
        assert!(chunks.iter().any(|c| c.content.contains("Small Section")));
    }

    #[test]
    fn markdown_empty_returns_empty() {
        assert!(split_markdown("", 500, 50).is_empty());
    }

    #[test]
    fn zero_overlap_no_repeated_lines() {
        let source = (0..10)
            .map(|i| format!("unique line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        // With zero overlap each line should appear in exactly one chunk
        let chunks = split(&source, 50, 0);
        let total_lines: usize = chunks.iter().map(|c| c.end_line - c.start_line + 1).sum();
        assert_eq!(total_lines, 10);
    }

    #[test]
    fn raw_chunk_carries_metadata_field() {
        let c = RawChunk {
            content: "body".into(),
            start_line: 1,
            end_line: 5,
            metadata: Some("src/foo.rs :: fn bar".into()),
        };
        assert_eq!(c.metadata.as_deref(), Some("src/foo.rs :: fn bar"));
    }

    // --- chunk_markdown tests ---

    #[test]
    fn chunk_markdown_splits_on_headings() {
        let text = "intro\n\n## Section A\ntext a\n\n## Section B\ntext b\n";
        let chunks = chunk_markdown(text, 1000);
        assert!(
            chunks.len() >= 2,
            "expected at least 2 chunks, got {:?}",
            chunks
        );
        assert!(chunks.iter().any(|c| c.contains("Section A")));
        assert!(chunks.iter().any(|c| c.contains("Section B")));
    }

    #[test]
    fn chunk_markdown_respects_token_budget() {
        let long = "a ".repeat(5000);
        let chunks = chunk_markdown(&long, 100);
        assert!(chunks.len() > 1, "long text should be split");
    }

    #[test]
    fn chunk_markdown_empty_returns_empty() {
        assert!(chunk_markdown("", 1000).is_empty());
    }

    #[test]
    fn chunk_markdown_single_section_fits_in_one_chunk() {
        let text = "# Title\n\nShort content that fits easily.\n";
        let chunks = chunk_markdown(text, 1000);
        assert_eq!(chunks.len(), 1);
    }

    // --- split_markdown_with_depth tests ---

    #[test]
    fn split_markdown_default_depth_ignores_h4() {
        // LOAD-BEARING: `#### D` must NOT start a chunk at the default depth.
        // The code index's chunk_ids encode start_line, so changing this default
        // silently invalidates 33,032 existing chunks.
        let src = "# A\n\ntext\n\n#### D\n\nmore\n";
        let chunks = split_markdown(src, 10_000, 0);
        assert_eq!(chunks.len(), 1, "h4 must not split at default depth");
    }

    #[test]
    fn split_markdown_with_depth_6_splits_on_h4() {
        let src = "# A\n\ntext\n\n#### D\n\nmore\n";
        let chunks = split_markdown_with_depth(src, 10_000, 0, 6);
        assert_eq!(chunks.len(), 2, "h4 must split at depth 6");
        assert!(chunks[1].content.starts_with("#### D"));
        assert_eq!(
            chunks[1].start_line, 5,
            "line numbers stay 1-indexed and file-relative"
        );
    }

    #[test]
    fn split_markdown_with_depth_3_equals_the_default() {
        let src = "# A\n\nx\n\n## B\n\ny\n\n### C\n\nz\n\n#### D\n\nw\n";
        let a = split_markdown(src, 10_000, 0);
        let b = split_markdown_with_depth(src, 10_000, 0, 3);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.content, y.content);
            assert_eq!((x.start_line, x.end_line), (y.start_line, y.end_line));
        }
    }
}
