-- Secret values are deliberately excluded from the JSONL business journal.
CREATE TABLE provider_secrets (name TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE order_visuals (id TEXT PRIMARY KEY, basket_key TEXT NOT NULL UNIQUE, session_token TEXT NOT NULL, prompt TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending', image_path TEXT, error TEXT, created INTEGER NOT NULL);
