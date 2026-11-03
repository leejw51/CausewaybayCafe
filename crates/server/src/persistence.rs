//! SQLite is authoritative; a transactional outbox feeds the durable JSONL journal.
use crate::*;
use std::{
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

pub struct Journal {
    path: PathBuf,
    lock: Mutex<()>,
}
impl Journal {
    pub async fn setup(db: &SqlitePool, database: &str) -> Result<Option<Self>> {
        if database == ":memory:" {
            return Ok(None);
        }
        let tables = [
            "menu",
            "orders",
            "staff",
            "shop",
            "cafe_tables",
            "payments",
            "ai_usage",
            "menu_drafts",
            "image_jobs",
            "test_faucet_credits",
            "service_calls",
            "audit",
            "guest_turns",
            "guest_tool_events",
            "floorplan_drafts",
            "service_simulation",
            "simulated_orders",
        ];
        let mut tx = db.begin_with("BEGIN IMMEDIATE").await.map_err(err)?;
        sqlx::query("CREATE TABLE IF NOT EXISTS persistence_events (id INTEGER PRIMARY KEY AUTOINCREMENT, payload TEXT NOT NULL)").execute(&mut *tx).await.map_err(err)?;
        for table in tables {
            let columns = sqlx::query(&format!("PRAGMA table_info({table})"))
                .fetch_all(&mut *tx)
                .await
                .map_err(err)?;
            // Session bearer tokens must never be copied into the journal.
            let names: Vec<String> = columns
                .iter()
                .map(|r| r.get::<String, _>("name"))
                .filter(|n| n != "session_token" && n != "account")
                .collect();
            for (operation, source) in [("insert", "NEW"), ("update", "NEW"), ("delete", "OLD")] {
                sqlx::query(&format!(
                    "DROP TRIGGER IF EXISTS journal_{table}_{operation}"
                ))
                .execute(&mut *tx)
                .await
                .map_err(err)?;
                let fields = names
                    .iter()
                    .map(|n| format!("'{n}',{source}.\"{n}\""))
                    .collect::<Vec<_>>()
                    .join(",");
                let record = format!("json_object({fields})");
                sqlx::query(&format!("CREATE TRIGGER IF NOT EXISTS journal_{table}_{operation} AFTER {operation} ON {table} BEGIN INSERT INTO persistence_events(payload) VALUES(json_object('version',1,'table','{table}','operation','{operation}','rowid',{source}.rowid,'created',unixepoch(),'record',{record})); END")).execute(&mut *tx).await.map_err(err)?;
            }
        }
        tx.commit().await.map_err(err)?;
        Ok(Some(Self {
            path: PathBuf::from(database).with_extension("jsonl"),
            lock: Mutex::new(()),
        }))
    }
    pub async fn flush(&self, db: &SqlitePool) -> Result<()> {
        let _guard = self.lock.lock().await;
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&self.path).map_err(err)?;
        // Recover an interrupted final write; committed events remain in SQLite for replay.
        // Read only the tail, rather than reloading a growing journal on each request.
        let mut offset = file.metadata().map_err(err)?.len();
        let mut bytes = Vec::new();
        while offset > 0 {
            let size = offset.min(8192) as usize;
            offset -= size as u64;
            file.seek(SeekFrom::Start(offset)).map_err(err)?;
            let mut chunk = vec![0; size];
            file.read_exact(&mut chunk).map_err(err)?;
            chunk.extend_from_slice(&bytes);
            bytes = chunk;
            if bytes.iter().filter(|b| **b == b'\n').count() >= 2 {
                break;
            }
        }
        let end = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        file.set_len(offset + end as u64).map_err(err)?;
        let last = bytes[..end]
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .next_back();
        let mut cursor = match last {
            Some(line) => serde_json::from_slice::<Value>(line).map_err(err)?["sequence"]
                .as_i64()
                .ok_or("Invalid JSONL sequence")?,
            None => 0,
        };
        file.seek(SeekFrom::End(0)).map_err(err)?;
        loop {
            let rows = sqlx::query(
                "SELECT id,payload FROM persistence_events WHERE id>? ORDER BY id LIMIT 256",
            )
            .bind(cursor)
            .fetch_all(db)
            .await
            .map_err(err)?;
            if rows.is_empty() {
                break;
            }
            for row in rows {
                cursor = row.get(0);
                let mut value: Value =
                    serde_json::from_str(&row.get::<String, _>(1)).map_err(err)?;
                value["sequence"] = json!(cursor);
                let mut line = serde_json::to_vec(&value).map_err(err)?;
                line.push(b'\n');
                file.write_all(&line).map_err(err)?;
            }
            file.sync_all().map_err(err)?;
        }
        Ok(())
    }
}
impl App {
    pub async fn flush_journal(&self) -> Result<()> {
        if let Some(journal) = &self.journal {
            journal.flush(&self.db).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn journal_recovers_partial_writes_and_excludes_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("cafe.sqlite");
        let app = init(database.to_str().unwrap(), "", true).await.unwrap();
        let session = app
            .call(Rpc {
                method: "guest".into(),
                token: String::new(),
                payload: json!({}),
            })
            .await
            .unwrap();
        let token = session["token"].as_str().unwrap();
        sqlx::query("UPDATE sessions SET wallet='journal-test-wallet' WHERE token=?")
            .bind(token)
            .execute(&app.db)
            .await
            .unwrap();
        app.call(Rpc {
            method: "faucet_claim".into(),
            token: token.into(),
            payload: json!({}),
        })
        .await
        .unwrap();
        let path = database.with_extension("jsonl");
        let first = std::fs::read_to_string(&path).unwrap();
        assert!(!first.contains(token));
        let lines: Vec<Value> = first
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["record"]["balance"], 100000);
        // A rolled-back database write must not become a journal event.
        let mut tx = app.db.begin().await.unwrap();
        sqlx::query("UPDATE test_faucet_credits SET balance=2000")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.rollback().await.unwrap();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"sequence\":")
            .unwrap();
        app.flush_journal().await.unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
        // Committed but not exported changes are recovered after restart, exactly once.
        sqlx::query("UPDATE test_faucet_credits SET balance=3000")
            .execute(&app.db)
            .await
            .unwrap();
        app.db.close().await;
        let reopened = init(database.to_str().unwrap(), "", true).await.unwrap();
        reopened.flush_journal().await.unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        let lines: Vec<Value> = content
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1]["operation"], "update");
        assert_eq!(lines[1]["record"]["balance"], 3000);
    }
}
