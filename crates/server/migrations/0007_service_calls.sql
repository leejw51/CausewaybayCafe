CREATE TABLE service_calls (id INTEGER PRIMARY KEY AUTOINCREMENT, session_token TEXT NOT NULL, table_no INTEGER NOT NULL, note TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'open', created INTEGER NOT NULL);
CREATE UNIQUE INDEX one_open_service_call ON service_calls(session_token) WHERE status='open';
