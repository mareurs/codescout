use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use tantivy::{
    collector::TopDocs,
    directory::MmapDirectory,
    query::QueryParser,
    schema::{
        Field, IndexRecordOption, Schema, SchemaBuilder, TextFieldIndexing, TextOptions,
        FAST, STORED,
    },
    tokenizer::{Token, TokenStream, Tokenizer},
    Index, IndexSettings, IndexWriter, TantivyDocument,
};

pub use crate::embed::fusion::BM25Result;

// ── Tokenizer ────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct CodeTokenizer;

pub struct CodeTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl TokenStream for CodeTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }
    fn token(&self) -> &Token {
        &self.tokens[self.index - 1]
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index - 1]
    }
}

impl Tokenizer for CodeTokenizer {
    type TokenStream<'a> = CodeTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> CodeTokenStream {
        let raw = tokenize_code_text(text);
        let tokens = raw
            .into_iter()
            .enumerate()
            .map(|(pos, text)| Token {
                offset_from: 0,
                offset_to: 0,
                position: pos,
                text,
                position_length: 1,
            })
            .collect();
        CodeTokenStream { tokens, index: 0 }
    }
}

fn split_camel_case(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_uppercase() && chars[i - 1].is_lowercase() {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
        }
        current.push(c);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts.into_iter().map(|s| s.to_lowercase()).collect()
}

pub fn tokenize_code_text(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if word.is_empty() {
            continue;
        }
        for part in word.split('_').filter(|s| !s.is_empty()) {
            tokens.extend(split_camel_case(part));
        }
    }
    tokens
}

// ── Schema ───────────────────────────────────────────────────────────────────

fn build_schema() -> (Schema, Field, Field, Field, Field) {
    todo!("Task 3")
}

fn tantivy_dir(root: &Path) -> std::path::PathBuf {
    root.join(".codescout").join("tantivy")
}

// ── BM25Index ────────────────────────────────────────────────────────────────

pub struct BM25Index {
    index: Index,
    chunk_id: Field,
    content: Field,
    file_path: Field,
    metadata: Field,
}

impl BM25Index {
    pub fn open(root: &Path) -> Result<Option<Self>> {
        todo!("Task 3")
    }

    pub fn build(root: &Path, conn: &Connection) -> Result<Self> {
        todo!("Task 3")
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<BM25Result>> {
        todo!("Task 3")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_splits_camel_case() {
        assert_eq!(
            tokenize_code_text("parseJsonObject"),
            vec!["parse", "json", "object"]
        );
    }

    #[test]
    fn tokenizer_splits_snake_case() {
        assert_eq!(tokenize_code_text("open_db"), vec!["open", "db"]);
    }

    #[test]
    fn tokenizer_splits_file_path() {
        assert_eq!(
            tokenize_code_text("src/embed/index.rs"),
            vec!["src", "embed", "index", "rs"]
        );
    }

    #[test]
    fn tokenizer_handles_mixed_ident() {
        assert_eq!(
            tokenize_code_text("impl Tool for SemanticSearch"),
            vec!["impl", "tool", "for", "semantic", "search"]
        );
    }

    #[test]
    fn tokenizer_strips_empty_tokens() {
        let tokens = tokenize_code_text("  spaces  and__double  ");
        assert!(!tokens.iter().any(|t| t.is_empty()), "no empty tokens");
    }
}
