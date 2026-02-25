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

/// Split markdown content by heading boundaries, then apply character limits.
///
/// Each `#`, `##`, or `###` heading starts a new section. Sections that fit
/// within `chunk_size` become a single chunk; oversized sections are sub-split
/// by the regular [`split`] function with line offsets adjusted.
pub fn split_markdown(source: &str, chunk_size: usize, chunk_overlap: usize) -> Vec<RawChunk> {
    if source.is_empty() {
        return vec![];
    }

    let lines: Vec<&str> = source.lines().collect();
    let mut sections: Vec<(usize, usize)> = vec![]; // (start_idx, end_idx) 0-indexed
    let mut section_start = 0;

    for (i, line) in lines.iter().enumerate() {
        if i > 0 && (line.starts_with("## ") || line.starts_with("### ") || line.starts_with("# "))
        {
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
                start_line: start + 1, // 1-indexed
                end_line: end,         // end is exclusive in lines[], so this is the last line
            });
        } else {
            // Section too large — sub-split with regular splitter
            let sub_chunks = split(&section_text, chunk_size, chunk_overlap);
            for mut sc in sub_chunks {
                sc.start_line += start; // adjust to file-level line numbers
                sc.end_line += start;
                chunks.push(sc);
            }
        }
    }
    chunks
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
}
