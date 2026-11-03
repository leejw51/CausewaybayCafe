CREATE TABLE menu (id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL, category TEXT NOT NULL, price INTEGER NOT NULL CHECK(price >= 0), available INTEGER NOT NULL DEFAULT 1, allergens TEXT NOT NULL);
CREATE TABLE sessions (token TEXT PRIMARY KEY, wallet TEXT, table_no INTEGER NOT NULL, expires INTEGER NOT NULL, pending TEXT, cart TEXT NOT NULL DEFAULT '[]');
CREATE TABLE challenges (nonce TEXT PRIMARY KEY, wallet TEXT NOT NULL, message TEXT NOT NULL, expires INTEGER NOT NULL);
CREATE TABLE staff (wallet TEXT PRIMARY KEY);
CREATE TABLE orders (id INTEGER PRIMARY KEY AUTOINCREMENT, session_token TEXT NOT NULL, table_no INTEGER NOT NULL, items TEXT NOT NULL, total INTEGER NOT NULL, status TEXT NOT NULL DEFAULT 'new', created INTEGER NOT NULL, request_id TEXT NOT NULL, UNIQUE(session_token, request_id));
CREATE TABLE messages (id INTEGER PRIMARY KEY AUTOINCREMENT, session_token TEXT NOT NULL, role TEXT NOT NULL, content TEXT NOT NULL);
CREATE TABLE audit (id INTEGER PRIMARY KEY AUTOINCREMENT, wallet TEXT, action TEXT NOT NULL, detail TEXT NOT NULL, created INTEGER NOT NULL);
CREATE INDEX orders_session ON orders(session_token);
CREATE INDEX messages_session ON messages(session_token, id);
INSERT INTO menu VALUES
('milk-tea','Hong Kong milk tea','Silky, strong tea with evaporated milk. Hot or iced.','Drinks',2800,1,'milk'),
('yuenyeung','Yuenyeung','The best of both worlds: coffee meets milk tea.','Drinks',3200,1,'milk'),
('lemon-tea','Iced lemon tea','Fresh lemon, fragrant black tea, a little sunshine.','Drinks',2600,1,''),
('pineapple-bun','Butter pineapple bun','A golden crackly top and a generous slice of butter.','Bakery',2400,1,'wheat, egg, milk'),
('egg-tart','Egg tart','Flaky pastry with a soft, golden custard centre.','Bakery',1800,1,'wheat, egg, milk'),
('french-toast','Hong Kong French toast','Golden toast, peanut butter, syrup. A little indulgence.','Kitchen',4200,1,'wheat, egg, milk, peanut'),
('macaroni','Breakfast macaroni','Comforting soup with macaroni, ham and a fried egg.','Kitchen',4800,1,'wheat, egg'),
('rice','Tomato baked rice','Tomato sauce, melted cheese and a warm bowl of rice.','Kitchen',6200,1,'milk');
