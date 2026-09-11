use std::path::{Path, PathBuf};

use crate::source::{AgentSource, IngestKind};
use devtone_core::{AgentKind, TinyStr, TokenDelta};
use rusqlite::Connection;

pub struct OpenCodeSource {
    db: PathBuf,
    last: Option<(u32, u32, u32)>,
}

impl OpenCodeSource {
    pub fn new() -> Self {
        Self::with_db(default_db())
    }

    pub fn with_db(path: impl Into<PathBuf>) -> Self {
        Self {
            db: path.into(),
            last: None,
        }
    }
}

fn default_db() -> PathBuf {
    if let Ok(p) = std::env::var("DEVTONE_OPENCODE_DB") {
        return PathBuf::from(p);
    }
    dirs_home().join(".local/share/opencode/opencode.db")
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("."))
}

impl Default for OpenCodeSource {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentSource for OpenCodeSource {
    fn name(&self) -> AgentKind {
        AgentKind::OpenCode
    }
    fn watch_paths(&self) -> Vec<PathBuf> {
        vec![self.db.clone()]
    }
    fn ingest(&mut self, _path: &Path, _kind: IngestKind) -> Option<TokenDelta> {
        let uri = format!("file:{}?mode=ro", self.db.display());
        let conn = Connection::open(&uri).ok()?;
        let mut stmt = conn
            .prepare(
                "SELECT tokens_input, tokens_output, tokens_cache_read, model, time_updated
                 FROM session ORDER BY time_updated DESC LIMIT 1",
            )
            .ok()?;
        let row = stmt
            .query_row([], |r| {
                Ok((
                    r.get::<_, i64>(0).unwrap_or(0) as u32,
                    r.get::<_, i64>(1).unwrap_or(0) as u32,
                    r.get::<_, i64>(2).unwrap_or(0) as u32,
                    r.get::<_, String>(3).unwrap_or_default(),
                    r.get::<_, i64>(4).unwrap_or(0) as u64,
                ))
            })
            .ok()?;
        let (inn, out, cache, model, ts) = row;
        let prev = self.last.replace((out, inn, cache));
        let (d_out, d_in, d_cache) = match prev {
            Some((po, pi, pc)) => (out.saturating_sub(po), inn.saturating_sub(pi), cache.saturating_sub(pc)),
            None => (out, inn, cache),
        };
        Some(TokenDelta {
            agent: AgentKind::OpenCode,
            out: d_out,
            inn: d_in,
            cache_read: d_cache,
            cache_write: 0,
            model: TinyStr::from_str_lossy(&model),
            ts_ms: ts,
            tools: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::OpenCodeSource;
    use crate::source::{AgentSource, IngestKind};
    use rusqlite::Connection;

    fn opencode_db_with_session(inn: i64, out: i64, prompt: &str) -> tempfile::NamedTempFile {
        let f = tempfile::NamedTempFile::new().unwrap();
        let conn = Connection::open(f.path()).unwrap();
        conn.execute_batch(
            "CREATE TABLE session (
                id TEXT,
                tokens_input INTEGER,
                tokens_output INTEGER,
                tokens_cache_read INTEGER,
                tokens_cache_write INTEGER,
                model TEXT,
                time_updated INTEGER
            );
            CREATE TABLE session_input (id TEXT, session_id TEXT, prompt TEXT);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session (id, tokens_input, tokens_output, tokens_cache_read, tokens_cache_write, model, time_updated)
             VALUES ('s1', ?1, ?2, 0, 0, 'qwen', 1000)",
            rusqlite::params![inn, out],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session_input (id, session_id, prompt) VALUES ('i1', 's1', ?1)",
            rusqlite::params![prompt],
        )
        .unwrap();
        f
    }

    #[test]
    fn opencode_sqlite_reads_token_columns_not_prompt() {
        let db = opencode_db_with_session(90, 12, "SECRET_PROMPT");
        let mut src = OpenCodeSource::with_db(db.path());
        let d = src.ingest(db.path(), IngestKind::FileChanged).unwrap();
        assert_eq!(d.out, 12);
        assert_eq!(d.inn, 90);
        assert!(!format!("{d:?}").contains("SECRET_PROMPT"));
    }
}
