use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use tantivy::{
    collector::TopDocs,
    directory::MmapDirectory,
    query::QueryParser,
    schema::{
        Field, IndexRecordOption, Schema, SchemaBuilder, TextFieldIndexing, TextOptions, FAST,
        STORED,
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
        if i > 0 && c.is_uppercase() && chars[i - 1].is_lowercase() && !current.is_empty() {
            parts.push(std::mem::take(&mut current));
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
    let mut b = SchemaBuilder::new();
    let chunk_id = b.add_u64_field("chunk_id", FAST | STORED);
    let content = b.add_text_field(
        "content",
        TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("code")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored(),
    );
    let file_path = b.add_text_field(
        "file_path",
        TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("code")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored(),
    );
    let metadata = b.add_text_field(
        "metadata",
        TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("code")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored(),
    );
    (b.build(), chunk_id, content, file_path, metadata)
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
        let dir_path = tantivy_dir(root);
        if !dir_path.exists() {
            return Ok(None);
        }
        let dir = MmapDirectory::open(&dir_path)?;
        if !Index::exists(&dir)? {
            return Ok(None);
        }
        let (_, chunk_id, content, file_path, metadata) = build_schema();
        let index = Index::open(dir)?;
        index.tokenizers().register("code", CodeTokenizer);
        Ok(Some(Self {
            index,
            chunk_id,
            content,
            file_path,
            metadata,
        }))
    }

    pub fn build(root: &Path, conn: &Connection) -> Result<Self> {
        let dir_path = tantivy_dir(root);
        if dir_path.exists() {
            std::fs::remove_dir_all(&dir_path)?;
        }
        std::fs::create_dir_all(&dir_path)?;

        let (schema, chunk_id, content, file_path, metadata) = build_schema();
        let dir = MmapDirectory::open(&dir_path)?;
        let index = Index::create(dir, schema, IndexSettings::default())?;
        index.tokenizers().register("code", CodeTokenizer);

        let mut writer: IndexWriter = index.writer(50_000_000)?;

        let mut stmt = conn.prepare(
            "SELECT id, content, file_path, COALESCE(metadata, '') \
             FROM chunks WHERE source = 'project'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)? as u64,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (id, cont, fp, meta) = row?;
            let mut doc = TantivyDocument::default();
            doc.add_u64(chunk_id, id);
            doc.add_text(content, &cont);
            doc.add_text(file_path, &fp);
            doc.add_text(metadata, &meta);
            writer.add_document(doc)?;
        }
        writer.commit()?;

        Ok(Self {
            index,
            chunk_id,
            content,
            file_path,
            metadata,
        })
    }

    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<BM25Result>> {
        use tantivy::schema::Value as _;
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let mut parser = QueryParser::for_index(
            &self.index,
            vec![self.content, self.file_path, self.metadata],
        );
        parser.set_field_boost(self.content, 1.0);
        parser.set_field_boost(self.file_path, 1.5);
        parser.set_field_boost(self.metadata, 2.0);

        let (query, _) = parser.parse_query_lenient(query_str);
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        top_docs
            .iter()
            .enumerate()
            .map(|(rank, (score, addr))| {
                let doc: TantivyDocument = searcher.doc(*addr)?;
                let chunk_id = doc
                    .get_first(self.chunk_id)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                Ok(BM25Result {
                    chunk_id,
                    score: *score,
                    rank: rank + 1,
                })
            })
            .collect()
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

    #[test]
    fn build_and_search_roundtrip() {
        use rusqlite::Connection;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let conn = Connection::open(":memory:").unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (
                id       INTEGER PRIMARY KEY,
                content  TEXT NOT NULL,
                file_path TEXT NOT NULL,
                metadata TEXT,
                source   TEXT NOT NULL DEFAULT 'project'
            );
            INSERT INTO chunks VALUES
              (1,'fn parse_model(name: &str) -> Result<EmbeddingModel>','src/embed/local.rs','fn parse_model','project'),
              (2,'struct SearchResult { file_path: String }','src/embed/schema.rs','struct SearchResult','project'),
              (3,'fn open_db(project_root: &Path) -> Result<Connection>','src/embed/index.rs','fn open_db','project');",
        ).unwrap();

        let idx = BM25Index::build(root, &conn).unwrap();
        let results = idx.search("parse model embedding", 10).unwrap();
        assert!(!results.is_empty(), "expected results");
        assert_eq!(
            results[0].chunk_id, 1,
            "parse_model chunk should rank first"
        );
    }

    #[test]
    fn build_excludes_library_source() {
        use rusqlite::Connection;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let conn = Connection::open(":memory:").unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (
                id INTEGER PRIMARY KEY, content TEXT NOT NULL,
                file_path TEXT NOT NULL, metadata TEXT,
                source TEXT NOT NULL DEFAULT 'project'
            );
            INSERT INTO chunks VALUES (1,'project chunk','proj.rs',NULL,'project');
            INSERT INTO chunks VALUES (2,'library chunk','lib.rs',NULL,'library');",
        )
        .unwrap();

        let idx = BM25Index::build(dir.path(), &conn).unwrap();
        let results = idx.search("library chunk", 10).unwrap();
        assert!(
            !results.iter().any(|r| r.chunk_id == 2),
            "library chunks must not appear"
        );
    }

    #[test]
    fn open_returns_none_when_absent() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let result = BM25Index::open(dir.path()).unwrap();
        assert!(result.is_none());
    }
}
