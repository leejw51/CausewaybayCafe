use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use cafe_shared::{identity, Rpc};
use serde_json::{json, Value};
use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
pub mod ai;
pub mod drafts;
pub mod events;
pub mod faucet;
pub mod floorplan;
pub mod group_order;
mod guest_agent;
pub mod guest_media;
pub mod guest_tools;
pub mod harness;
pub mod management;
pub mod persistence;
pub mod provider;
pub mod simulation;
pub mod transport;
pub mod usage;
pub type Result<T> = std::result::Result<T, String>;
#[derive(Clone)]
pub struct App {
    pub db: SqlitePool,
    pub journal: Option<Arc<persistence::Journal>>,
    pub media_dir: std::path::PathBuf,
    pub changes: tokio::sync::watch::Sender<u64>,
    pub owner: String,
    pub faucet_enabled: bool,
    pub client: reqwest::Client,
    pub locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    pub budget: Arc<Mutex<HashMap<String, (i64, u32)>>>,
}
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
pub fn uid() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub async fn init(path: &str, owner: &str, simulation: bool) -> Result<App> {
    if path != ":memory:" {
        if let Some(p) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(p).map_err(err)?;
        }
    }
    let options = SqliteConnectOptions::from_str(path)
        .map_err(err)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(10));
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(if path == ":memory:" { 1 } else { 5 })
        .connect_with(options)
        .await
        .map_err(err)?;
    sqlx::migrate!().run(&db).await.map_err(err)?;
    sqlx::query("UPDATE shop SET simulation=? WHERE id=1 AND simulation IS NULL")
        .bind(simulation)
        .execute(&db)
        .await
        .map_err(err)?;
    let journal = persistence::Journal::setup(&db, path).await?.map(Arc::new);
    if let Some(journal) = &journal {
        journal.flush(&db).await?;
    }
    let owner = if owner.is_empty() {
        String::new()
    } else {
        identity::normalize_address(owner)?
    };
    Ok(App {
        db,
        journal,
        media_dir: std::path::Path::new(path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("media"),
        changes: tokio::sync::watch::channel(0).0,
        owner,
        faucet_enabled: simulation || std::env::var("CAFE_FAUCET_ENABLED").as_deref() == Ok("1"),
        client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(err)?,
        locks: Default::default(),
        budget: Default::default(),
    })
}
pub fn router(app: App) -> Router {
    Router::new()
        .route("/api/rpc", post(rpc))
        .route("/api/events", post(events::subscribe))
        .route("/health", get(|| async { "ok" }))
        .route("/qr", get(qr))
        .nest_service(
            "/media",
            tower_http::services::ServeDir::new(app.media_dir.clone()),
        )
        .fallback_service(
            tower_http::services::ServeDir::new("dist").append_index_html_on_directories(true),
        )
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(
            tower_http::set_header::SetResponseHeaderLayer::if_not_present(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("no-store"),
            ),
        )
        .with_state(app)
}
async fn qr(State(app): State<App>, Query(q): Query<HashMap<String, String>>) -> Response {
    match app
        .qr_svg(q.get("table").map(String::as_str).unwrap_or("1"))
        .await
    {
        Ok(svg) => ([(header::CONTENT_TYPE, "image/svg+xml")], svg).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn rpc(State(app): State<App>, bytes: Bytes) -> Response {
    let result = match cafe_shared::decode(&bytes) {
        Ok(r) => app.call(r).await,
        Err(e) => Err(e),
    };
    let payload = match result {
        Ok(v) => json!({"ok":true,"data":v}),
        Err(e) => json!({"ok":false,"error":e}),
    };
    match cafe_shared::encode(&Rpc {
        method: "result".into(),
        token: String::new(),
        payload,
    }) {
        Ok(b) => ([(header::CONTENT_TYPE, "application/x-capnp")], b).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
#[derive(Clone)]
pub struct Session {
    pub token: String,
    pub role: String,
    pub wallet: Option<String>,
    pub table: i64,
}
impl App {
    pub async fn session(&self, token: &str) -> Result<Session> {
        let r = sqlx::query("SELECT wallet,table_no FROM sessions WHERE token=? AND expires>?")
            .bind(token)
            .bind(now())
            .fetch_optional(&self.db)
            .await
            .map_err(err)?
            .ok_or("Session expired. Please sign in again.")?;
        let wallet: Option<String> = r.get(0);
        let role = if wallet.as_deref() == Some(self.owner.as_str()) && !self.owner.is_empty() {
            "owner"
        } else if let Some(w) = &wallet {
            if sqlx::query("SELECT 1 FROM staff WHERE wallet=?")
                .bind(w)
                .fetch_optional(&self.db)
                .await
                .map_err(err)?
                .is_some()
            {
                "staff"
            } else {
                "guest"
            }
        } else {
            "guest"
        };
        Ok(Session {
            token: token.into(),
            role: role.into(),
            wallet,
            table: r.get(1),
        })
    }
    async fn create_session(&self, wallet: Option<String>, table: i64) -> Result<Value> {
        let token = uid();
        sqlx::query("INSERT INTO sessions(token,wallet,table_no,expires) VALUES(?,?,?,?)")
            .bind(&token)
            .bind(wallet)
            .bind(table)
            .bind(now() + 43200)
            .execute(&self.db)
            .await
            .map_err(err)?;
        Ok(json!({"token":token,"role":self.session(&token).await?.role}))
    }
    pub async fn call(&self, r: Rpc) -> Result<Value> {
        let result = self.call_inner(r).await;
        // Never report a committed mutation as failed solely because its export failed.
        // The transactional outbox survives and is retried on the next request/startup.
        if let Err(error) = self.flush_journal().await {
            eprintln!("JSONL export pending: {error}");
        }
        result
    }
    async fn call_inner(&self, r: Rpc) -> Result<Value> {
        let p = r.payload;
        match r.method.as_str() {
            "guest" => {
                let table = self.resolve_guest_table(&p).await?;
                return self.create_session(None, table).await;
            }
            "challenge" => {
                let wallet = identity::normalize_address(s(&p, "wallet")?)?;
                let nonce = uid();
                let msg = format!(
                    "CAUSEWAYBAY CAFE login\nWallet: {wallet}\nNonce: {nonce}\nExpires: {}",
                    now() + 120
                );
                sqlx::query("DELETE FROM challenges WHERE expires<?")
                    .bind(now())
                    .execute(&self.db)
                    .await
                    .map_err(err)?;
                sqlx::query("INSERT INTO challenges VALUES(?,?,?,?)")
                    .bind(&nonce)
                    .bind(wallet)
                    .bind(&msg)
                    .bind(now() + 120)
                    .execute(&self.db)
                    .await
                    .map_err(err)?;
                return Ok(json!({"nonce":nonce,"message":msg}));
            }
            "login" => {
                let row = sqlx::query(
                    "DELETE FROM challenges WHERE nonce=? AND expires>? RETURNING wallet,message",
                )
                .bind(s(&p, "nonce")?)
                .bind(now())
                .fetch_optional(&self.db)
                .await
                .map_err(err)?
                .ok_or("Challenge expired or already used")?;
                let wallet: String = row.get(0);
                if identity::recover(row.get::<String, _>(1).as_str(), s(&p, "signature")?)?
                    != wallet
                {
                    return Err("Invalid signature".into());
                }
                return self.create_session(Some(wallet), 1).await;
            }
            _ => {}
        }
        let session = self.session(&r.token).await?;
        // Serialize actions per session, including chat; prevents duplicate cart confirmations.
        let lock = {
            let mut locks = self.locks.lock().await;
            locks.entry(r.token.clone()).or_default().clone()
        };
        let _guard = lock.lock().await;
        let result = match r.method.as_str() {
            "state" => self.snapshot(&session).await,
            "floorplan_draft" => {
                self.limit(&session).await?;
                self.floorplan_draft(&session, &p).await
            }
            "simulation_control" => self.simulation_control(&session, &p).await,
            "order_visual" => {
                self.limit(&session).await?;
                self.request_order_visual(&session).await
            }
            "faucet_claim" => self.claim_faucet(&session).await,
            "table_select" => {
                require(&session, "staff")?;
                let id = p["table"].as_i64().ok_or("Table required")?;
                let active: bool = sqlx::query_scalar("SELECT active FROM cafe_tables WHERE id=?")
                    .bind(id)
                    .fetch_optional(&self.db)
                    .await
                    .map_err(err)?
                    .unwrap_or(false);
                if !active {
                    return Err("Table unavailable".into());
                }
                sqlx::query("UPDATE sessions SET table_no=? WHERE token=?")
                    .bind(id)
                    .bind(&session.token)
                    .execute(&self.db)
                    .await
                    .map_err(err)?;
                Ok(json!({"reply":"Table selected"}))
            }
            "agent_run" => {
                self.agent_run(&session, p["running"].as_bool().ok_or("running required")?)
                    .await
            }
            "menu_draft" => {
                require(&session, "staff")?;
                self.limit(&session).await?;
                self.generate_draft(&session, &p).await
            }
            "draft_save" => self.save_draft(&session, &p).await,
            "draft_publish" => self.publish_draft(&session, &p).await,
            "image_retry" => self.retry_image(&session, &p).await,
            "print_qr" => {
                require(&session, "owner")?;
                Ok(
                    json!({"reply":"QR print sheet is ready.","ui":"print_qr","tables":self.tables().await?,"shop":self.shop().await?}),
                )
            }
            "logout" => {
                sqlx::query("DELETE FROM sessions WHERE token=?")
                    .bind(&r.token)
                    .execute(&self.db)
                    .await
                    .map_err(err)?;
                Ok(json!({}))
            }
            "menu_list" => self.menu_list().await,
            "order_cancel" => self.cancel_order(&session, &p).await,
            "provider_key_set" => self.set_provider_key(&session, &p).await,
            "staff_call" => self.call_staff(&session, &p).await,
            "service_resolve" => self.resolve_call(&session, &p).await,
            "recommend_group" => self.recommend_group(&session, &p).await,
            "choose_menu" => self.choose_menu(&session, &p).await,
            "cancel_menu" | "can_menu" => self.cancel_menu(&session, &p).await,
            "simulation_order" => self.simulation_order(&session, &p).await,
            "current_progress" => self.current_progress(&session).await,
            "cart_add" => self.cart_add(&session, &p).await,
            "cart_remove" => {
                let index = p["index"].as_u64().ok_or("Basket index required")? as usize;
                let raw: String = sqlx::query_scalar("SELECT cart FROM sessions WHERE token=?")
                    .bind(&session.token)
                    .fetch_one(&self.db)
                    .await
                    .map_err(err)?;
                let mut cart: Vec<Value> = serde_json::from_str(&raw).map_err(err)?;
                if cart.get(index).is_none_or(|item| item != &p["item"]) {
                    return Err("Basket changed. Refresh and try again.".into());
                }
                cart.remove(index);
                sqlx::query("UPDATE sessions SET cart=?,pending=NULL WHERE token=?")
                    .bind(json!(cart).to_string())
                    .bind(&session.token)
                    .execute(&self.db)
                    .await
                    .map_err(err)?;
                Ok(json!({"reply":"Removed from your basket."}))
            }
            "cart_clear" => {
                sqlx::query("UPDATE sessions SET cart='[]',pending=NULL WHERE token=?")
                    .bind(&r.token)
                    .execute(&self.db)
                    .await
                    .map_err(err)?;
                Ok(json!({"reply":"Basket cleared."}))
            }
            "checkout" => self.checkout(&session, s(&p, "request_id")?).await,
            "confirm" => self.confirm(&session).await,
            "cancel" => {
                sqlx::query("UPDATE sessions SET pending=NULL WHERE token=?")
                    .bind(&r.token)
                    .execute(&self.db)
                    .await
                    .map_err(err)?;
                Ok(json!({"reply":"Cancelled."}))
            }
            "action" => self.propose(&session, p).await,
            "chat" => {
                self.limit(&session).await?;
                let result = self.chat(&session, &p).await;
                if session.role == "guest" && p["language"] == "ko" {
                    result.map_err(|e| {
                        if e.contains("Insufficient wallet") {"지갑 잔액이 부족해요. Faucet에서 충전해 주세요. 주문은 접수되지 않았어요.".into()}
                        else if e.contains("budget reached") || e.contains("rate limit") {"지금 AI 사용 한도에 도달했어요. 메뉴 버튼을 이용하거나 직원을 불러주세요.".into()}
                        else if e.contains("AI") || e.contains("MUSE") {"지금 대화 연결이 원활하지 않아요. 잠시 후 다시 말씀해 주세요. 메뉴 버튼으로도 주문할 수 있어요.".into()}
                        else if e.contains("closed") {"지금은 새 주문을 받지 않고 있어요. 직원을 불러주세요.".into()}
                        else if e.contains(" / ") {e.split(" / ").find(|s|s.chars().any(|c|('가'..='힣').contains(&c))).unwrap_or("요청을 처리하지 못했어요. 직원을 불러주세요.").into()}
                        else {"요청을 처리하지 못했어요. 메뉴와 수량을 확인하거나 직원을 불러주세요.".into()}
                    })
                } else {
                    result
                }
            }
            "transcribe" => {
                self.limit(&session).await?;
                self.transcribe(&session, &p).await
            }
            _ => Err("Unknown method".into()),
        };
        if result.is_ok()
            && matches!(
                r.method.as_str(),
                "checkout"
                    | "confirm"
                    | "chat"
                    | "agent_run"
                    | "draft_save"
                    | "image_retry"
                    | "logout"
            )
        {
            self.notify_change();
        }
        result
    }
    async fn limit(&self, s: &Session) -> Result<()> {
        let mut b = self.budget.lock().await;
        let e = b.entry(s.token.clone()).or_insert((now(), 0));
        if now() - e.0 >= 60 {
            *e = (now(), 0);
        }
        if e.1 >= 15 {
            return Err("Please wait a moment before asking again.".into());
        }
        e.1 += 1;
        Ok(())
    }
    pub async fn menu(&self) -> Result<Value> {
        let rows = sqlx::query("SELECT * FROM menu WHERE archived=0 ORDER BY rowid")
            .fetch_all(&self.db)
            .await
            .map_err(err)?;
        Ok(Value::Array(rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"description":r.get::<String,_>("description"),"category":r.get::<String,_>("category"),"price":r.get::<i64,_>("price"),"available":r.get::<bool,_>("available"),"allergens":r.get::<String,_>("allergens"),"image_path":r.get::<Option<String>,_>("image_path")})).collect()))
    }
    pub async fn snapshot(&self, s: &Session) -> Result<Value> {
        let row = sqlx::query("SELECT cart,pending FROM sessions WHERE token=?")
            .bind(&s.token)
            .fetch_one(&self.db)
            .await
            .map_err(err)?;
        let orders = sqlx::query(
            "SELECT * FROM orders WHERE (?!='guest' OR session_token=?) ORDER BY id DESC LIMIT 100",
        )
        .bind(&s.role)
        .bind(&s.token)
        .fetch_all(&self.db)
        .await
        .map_err(err)?;
        let orders:Vec<Value>=orders.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"table":r.get::<i64,_>("table_no"),"items":serde_json::from_str::<Value>(&r.get::<String,_>("items")).unwrap_or(json!([])),"total":r.get::<i64,_>("total"),"status":r.get::<String,_>("status"),"created":r.get::<i64,_>("created"),"version":r.get::<i64,_>("version"),"payment_status":r.get::<String,_>("payment_status"),"payment_method":r.get::<Option<String>,_>("payment_method"),"paid_amount":r.get::<i64,_>("paid_amount"),"note":r.get::<String,_>("note")})).collect();
        let messages=sqlx::query("SELECT role,content FROM (SELECT id,role,content FROM messages WHERE session_token=? ORDER BY id DESC LIMIT 40) ORDER BY id").bind(&s.token).fetch_all(&self.db).await.map_err(err)?;
        let messages: Vec<Value> = messages
            .iter()
            .map(|r| json!({"role":r.get::<String,_>(0),"content":r.get::<String,_>(1)}))
            .collect();
        let dashboard = if s.role != "guest" {
            let r=sqlx::query("SELECT COUNT(*),COALESCE(SUM(CASE WHEN payment_status='paid' THEN paid_amount ELSE 0 END),0),COALESCE(SUM(CASE WHEN status IN ('new','preparing','ready','served') THEN 1 ELSE 0 END),0) FROM orders WHERE created>=?").bind(now()-86400).fetch_one(&self.db).await.map_err(err)?;
            json!({"orders":r.get::<i64,_>(0),"sales":r.get::<i64,_>(1),"active":r.get::<i64,_>(2)})
        } else {
            Value::Null
        };
        let staff_wallets = if s.role == "owner" {
            sqlx::query_scalar::<_, String>("SELECT wallet FROM staff")
                .fetch_all(&self.db)
                .await
                .map_err(err)?
        } else {
            vec![]
        };
        Ok(
            json!({"floorplan_draft":self.floorplan_state(s).await?,"service_simulation":self.simulation_state(s).await?,"order_visual":self.order_visual(s).await?,"provider_config":self.provider_config(s).await?,"service_calls":self.service_calls(s).await?,"role":s.role,"wallet":s.wallet,"faucet":self.faucet_state(s).await?,"table":s.table,"menu":self.menu().await?,"cart":serde_json::from_str::<Value>(&row.get::<String,_>(0)).map_err(err)?,"pending":row.get::<Option<String>,_>(1).and_then(|v|serde_json::from_str::<Value>(&v).ok()),"orders":orders,"messages":messages,"dashboard":dashboard,"staff":staff_wallets,"team":self.team(s).await?,"shop":self.shop().await?,"tables":self.tables().await?,"audit":self.audit(s).await?,"usage":self.usage(s).await?,"drafts":self.drafts(s).await?,"simulation":self.shop().await?["simulation"],"ai_provider":std::env::var("CAFE_AI_PROVIDER").unwrap_or("meta".into())}),
        )
    }
    async fn cart_add(&self, ses: &Session, p: &Value) -> Result<Value> {
        let qty = p["quantity"].as_i64().unwrap_or(1);
        if !(1..=20).contains(&qty) {
            return Err("Choose 1–20 items".into());
        }
        let id = s(p, "id")?;
        let menu = self.menu().await?;
        let item = menu
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == id && m["available"] == true)
            .ok_or("That item is unavailable")?;
        let raw: String = sqlx::query_scalar("SELECT cart FROM sessions WHERE token=?")
            .bind(&ses.token)
            .fetch_one(&self.db)
            .await
            .map_err(err)?;
        let mut cart: Vec<Value> = serde_json::from_str(&raw).map_err(err)?;
        if cart.len() >= 30 {
            return Err("Basket limit reached".into());
        }
        let note = p["note"].as_str().unwrap_or("");
        if note.len() > 300 {
            return Err("Note is too long".into());
        }
        cart.push(
            json!({"id":id,"name":item["name"],"price":item["price"],"quantity":qty,"note":note}),
        );
        sqlx::query("UPDATE sessions SET cart=?,pending=NULL WHERE token=?")
            .bind(serde_json::to_string(&cart).map_err(err)?)
            .bind(&ses.token)
            .execute(&self.db)
            .await
            .map_err(err)?;
        Ok(
            json!({"reply":format!("Added {qty} × {}. Review your basket, then confirm your order.",item["name"].as_str().unwrap())}),
        )
    }
    async fn checkout(&self, s: &Session, request_id: &str) -> Result<Value> {
        self.checkout_items(s, request_id, None).await
    }
    async fn checkout_items(
        &self,
        s: &Session,
        request_id: &str,
        direct: Option<Vec<Value>>,
    ) -> Result<Value> {
        if request_id.len() > 80 || request_id.is_empty() {
            return Err("Invalid request ID".into());
        }
        // Reserve the write lock before reading to avoid SQLite read-to-write upgrade deadlocks.
        let mut tx = self.db.begin_with("BEGIN IMMEDIATE").await.map_err(err)?;
        let open: bool = sqlx::query_scalar("SELECT is_open FROM shop WHERE id=1")
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?;
        if !open {
            return Err("The café is closed for new orders.".into());
        }
        let active: bool = sqlx::query_scalar("SELECT active FROM cafe_tables WHERE id=?")
            .bind(s.table)
            .fetch_optional(&mut *tx)
            .await
            .map_err(err)?
            .unwrap_or(false);
        if !active {
            return Err("This table is not accepting orders.".into());
        }
        if let Some(id) = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM orders WHERE session_token=? AND request_id=?",
        )
        .bind(&s.token)
        .bind(request_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(err)?
        {
            return Ok(json!({"reply":format!("Order #{id} is already placed."),"order_id":id}));
        }
        let raw: String = sqlx::query_scalar("SELECT cart FROM sessions WHERE token=?")
            .bind(&s.token)
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?;
        let is_direct = direct.is_some();
        let mut cart: Vec<Value> = match direct {
            Some(items) => items,
            None => serde_json::from_str(&raw).map_err(err)?,
        };
        if cart.is_empty() {
            return Err("Your basket is empty".into());
        }
        let mut total = 0;
        for item in &mut cart {
            let r = sqlx::query(
                "SELECT price,name FROM menu WHERE id=? AND available=1 AND archived=0",
            )
            .bind(item["id"].as_str().unwrap_or(""))
            .fetch_optional(&mut *tx)
            .await
            .map_err(err)?
            .ok_or("An item is no longer available. Clear your basket and choose again.")?;
            let price: i64 = r.get(0);
            if item["price"].as_i64() != Some(price) {
                return Err(
                    "A price has changed. Clear your basket and review the new price.".into(),
                );
            }
            total += price * item["quantity"].as_i64().ok_or("Invalid quantity")?;
            item["name"] = json!(r.get::<String, _>(1));
        }
        let id=sqlx::query("INSERT INTO orders(session_token,table_no,items,total,created,request_id,simulation) VALUES(?,?,?,?,?,?,(SELECT simulation FROM shop WHERE id=1))").bind(&s.token).bind(s.table).bind(serde_json::to_string(&cart).map_err(err)?).bind(total).bind(now()).bind(request_id).execute(&mut *tx).await.map_err(err)?.last_insert_rowid();
        let mut paid = false;
        if s.role == "guest" && self.faucet_enabled {
            if let Some(wallet) = &s.wallet {
                let account = format!("wallet:{wallet}");
                if sqlx::query("UPDATE test_faucet_credits SET balance=balance-? WHERE account=? AND balance>=?").bind(total).bind(&account).bind(total).execute(&mut *tx).await.map_err(err)?.rows_affected()!=1 {
                    return Err("Insufficient wallet balance. Get credits from Faucet first. No order was placed. / 잔액이 부족합니다. Faucet에서 충전해 주세요. 주문은 접수되지 않았습니다.".into());
                }
                sqlx::query("UPDATE orders SET payment_status='paid',paid_amount=?,paid_at=?,payment_method='wallet',payment_reference=? WHERE id=?").bind(total).bind(now()).bind(&account).bind(id).execute(&mut *tx).await.map_err(err)?;
                sqlx::query("INSERT INTO payments(order_id,kind,amount,method,reference,wallet,created) VALUES(?,'payment',?,'wallet',?,?,?)").bind(id).bind(total).bind(&account).bind(wallet).bind(now()).execute(&mut *tx).await.map_err(err)?;
                paid = true;
            }
        }
        if !is_direct {
            sqlx::query("UPDATE sessions SET cart='[]',pending=NULL WHERE token=?")
                .bind(&s.token)
                .execute(&mut *tx)
                .await
                .map_err(err)?;
        }
        tx.commit().await.map_err(err)?;
        let items = cart
            .iter()
            .map(|i| format!("{} × {}", i["quantity"], i["name"].as_str().unwrap_or("")))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(
            json!({"reply":format!("Order #{id} is with our kitchen.\n{items}\nHK${:.2}{}",total as f64/100.,if paid{" · Paid from your wallet / 지갑 자동 결제 완료"}else{""}),"order_id":id,"paid":paid}),
        )
    }
}
pub fn s<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v[k].as_str().ok_or_else(|| format!("Missing {k}"))
}
pub fn require(s: &Session, role: &str) -> Result<()> {
    if s.role == "owner" || (role == "staff" && s.role == "staff") {
        Ok(())
    } else {
        Err("This action needs an authorized café wallet.".into())
    }
}
pub fn valid_transition(a: &str, b: &str) -> bool {
    matches!(
        (a, b),
        ("new", "preparing")
            | ("preparing", "ready")
            | ("ready", "served")
            | ("new", "cancelled")
            | ("preparing", "cancelled")
            | ("ready", "cancelled")
            | ("served", "cancelled")
    )
}

#[cfg(test)]
mod order_transition_tests {
    use super::*;
    #[test]
    fn terminal_orders_cannot_reenter_the_kitchen() {
        for from in ["completed", "cancelled", "refunded"] {
            for to in [
                "new",
                "preparing",
                "ready",
                "served",
                "cancelled",
                "completed",
                "refunded",
            ] {
                assert!(!valid_transition(from, to));
            }
        }
        assert!(valid_transition("new", "preparing"));
        assert!(valid_transition("preparing", "ready"));
        assert!(valid_transition("ready", "served"));
        assert!(!valid_transition("new", "served"));
        assert!(!valid_transition("ready", "preparing"));
    }
}
