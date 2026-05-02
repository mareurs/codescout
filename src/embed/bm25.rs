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

pub fn tokenize_code_text(_text: &str) -> Vec<String> {
    todo!("Task 2")
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
}
