CREATE TABLE guest_turns (
 session_token TEXT NOT NULL,
 id TEXT NOT NULL,
 input TEXT NOT NULL,
 language TEXT NOT NULL,
 status TEXT NOT NULL DEFAULT 'running',
 response TEXT,
 created INTEGER NOT NULL,
 PRIMARY KEY(session_token,id)
);
CREATE TABLE guest_tool_events (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 session_token TEXT NOT NULL,
 turn_id TEXT NOT NULL,
 tool TEXT NOT NULL,
 arguments TEXT NOT NULL,
 result TEXT NOT NULL,
 created INTEGER NOT NULL
);
