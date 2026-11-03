use crate::*;
use sqlx::{Sqlite, Transaction};
const OWNER_ACTIONS: &[&str] = &[
    "menu_create",
    "menu_update",
    "menu_delete",
    "staff_add",
    "staff_remove",
    "table_save",
    "floorplan_apply",
    "table_batch",
    "table_disable",
    "shop_update",
    "simulation_mode",
    "ai_limits",
    "order_refund",
    "draft_mark_published",
];
const STAFF_ACTIONS: &[&str] = &["order_status", "order_edit", "order_pay"];
pub fn is_action(action: &str) -> bool {
    OWNER_ACTIONS.contains(&action) || STAFF_ACTIONS.contains(&action) || action == "batch"
}
fn permission(ses: &Session, action: &str) -> Result<()> {
    if OWNER_ACTIONS.contains(&action) {
        require(ses, "owner")
    } else if STAFF_ACTIONS.contains(&action) {
        require(ses, "staff")
    } else {
        Err("Unsupported management action".into())
    }
}
fn text(p: &Value, key: &str, max: usize) -> Result<String> {
    let v = s(p, key)?.trim();
    if v.is_empty() || v.len() > max {
        return Err(format!("{key}: enter 1–{max} characters"));
    }
    Ok(v.into())
}
fn integer(p: &Value, key: &str, min: i64, max: i64) -> Result<i64> {
    p[key]
        .as_i64()
        .filter(|n| *n >= min && *n <= max)
        .ok_or(format!("{key}: expected a whole number {min}–{max}"))
}
fn identifier(p: &Value, key: &str) -> Result<String> {
    let v = text(p, key, 40)?;
    if !v
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("{key}: use letters, numbers, - or _"));
    }
    Ok(v)
}
impl App {
    pub async fn shop(&self) -> Result<Value> {
        let r = sqlx::query("SELECT * FROM shop WHERE id=1")
            .fetch_one(&self.db)
            .await
            .map_err(err)?;
        Ok(
            json!({"simulation":r.get::<bool,_>("simulation"),"guid":r.get::<Option<String>,_>("guid"),"name":r.get::<String,_>("name"),"currency":r.get::<String,_>("currency"),"is_open":r.get::<bool,_>("is_open"),"agent_running":r.get::<bool,_>("agent_running"),"daily_token_limit":r.get::<i64,_>("daily_token_limit"),"hourly_token_limit":r.get::<i64,_>("hourly_token_limit"),"max_output_tokens":r.get::<i64,_>("max_output_tokens")}),
        )
    }
    pub async fn tables(&self) -> Result<Value> {
        let rows=sqlx::query("SELECT t.*, (SELECT COUNT(*) FROM orders o WHERE o.table_no=t.id AND o.status NOT IN ('completed','cancelled','refunded')) AS open_orders FROM cafe_tables t ORDER BY t.id").fetch_all(&self.db).await.map_err(err)?;
        Ok(json!(rows.iter().map(|r|json!({"id":r.get::<i64,_>("id"),"code":r.get::<String,_>("code"),"kind":r.get::<String,_>("kind"),"seats":r.get::<i64,_>("seats"),"active":r.get::<bool,_>("active"),"x":r.get::<Option<f64>,_>("x"),"y":r.get::<Option<f64>,_>("y"),"open_orders":r.get::<i64,_>("open_orders")})).collect::<Vec<_>>()))
    }
    pub async fn team(&self, ses: &Session) -> Result<Value> {
        if ses.role != "owner" {
            return Ok(json!([]));
        }
        let r = sqlx::query("SELECT wallet,name FROM staff ORDER BY name,wallet")
            .fetch_all(&self.db)
            .await
            .map_err(err)?;
        Ok(json!(r.iter().map(|r|json!({"wallet":r.get::<String,_>(0),"name":r.get::<String,_>(1),"role":"staff"})).collect::<Vec<_>>()))
    }
    pub async fn audit(&self, ses: &Session) -> Result<Value> {
        if ses.role != "owner" {
            return Ok(json!([]));
        }
        let r =
            sqlx::query("SELECT action,detail,created,wallet FROM audit ORDER BY id DESC LIMIT 30")
                .fetch_all(&self.db)
                .await
                .map_err(err)?;
        Ok(json!(r.iter().map(|r|json!({"action":r.get::<String,_>(0),"detail":r.get::<String,_>(1),"created":r.get::<i64,_>(2),"wallet":r.get::<Option<String>,_>(3)})).collect::<Vec<_>>()))
    }
    pub async fn resolve_guest_table(&self, p: &Value) -> Result<i64> {
        let shop = self.shop().await?;
        if !p["shop_guid"].is_null() && p["shop_guid"] != shop["guid"] {
            return Err("This QR code belongs to another shop".into());
        }
        if p["table"].is_null() {
            return sqlx::query_scalar(
                "SELECT id FROM cafe_tables WHERE active=1 ORDER BY id LIMIT 1",
            )
            .fetch_optional(&self.db)
            .await
            .map_err(err)?
            .ok_or("No active tables. Ask the owner to configure tables.".into());
        }
        let key = p["table"]
            .as_str()
            .map(str::to_string)
            .or_else(|| p["table"].as_i64().map(|n| n.to_string()))
            .unwrap_or("1".into());
        sqlx::query_scalar("SELECT id FROM cafe_tables WHERE code=? AND active=1")
            .bind(key)
            .fetch_optional(&self.db)
            .await
            .map_err(err)?
            .ok_or("Table is unavailable. Ask staff for its QR code.".into())
    }
    pub async fn qr_svg(&self, key: &str) -> Result<String> {
        let code: String =
            sqlx::query_scalar("SELECT code FROM cafe_tables WHERE code=? AND active=1")
                .bind(key)
                .fetch_optional(&self.db)
                .await
                .map_err(err)?
                .ok_or("Table not found")?;
        let shop = self.shop().await?;
        let base = std::env::var("CAFE_PUBLIC_URL").unwrap_or("http://localhost:8791".into());
        let mut url = reqwest::Url::parse(&base).map_err(err)?;
        url.set_path("/");
        url.set_query(None);
        url.query_pairs_mut().append_pair("table", &code);
        if let Some(guid) = shop["guid"].as_str() {
            url.query_pairs_mut().append_pair("shop_guid", guid);
        }
        let qr = qrcode::QrCode::new(url.to_string()).map_err(err)?;
        Ok(qr
            .render::<qrcode::render::svg::Color>()
            .min_dimensions(240, 240)
            .build())
    }
    pub async fn agent_run(&self, ses: &Session, running: bool) -> Result<Value> {
        require(ses, "staff")?;
        sqlx::query("UPDATE shop SET agent_running=? WHERE id=1")
            .bind(running)
            .execute(&self.db)
            .await
            .map_err(err)?;
        sqlx::query("INSERT INTO audit(wallet,action,detail,created) VALUES(?,?,?,?)")
            .bind(&ses.wallet)
            .bind(if running { "agent_run" } else { "agent_stop" })
            .bind("{}")
            .bind(now())
            .execute(&self.db)
            .await
            .map_err(err)?;
        Ok(
            json!({"reply":if running{"AI is running. Requests are processed only when someone speaks or sends a message."}else{"Background and staff AI stopped. Guest messages still activate AI on demand. Manual café controls remain available."}}),
        )
    }
    pub async fn propose(&self, ses: &Session, mut p: Value) -> Result<Value> {
        let actions = if p["action"] == "batch" {
            p["actions"]
                .as_array_mut()
                .ok_or("Batch requires actions")?
        } else {
            p = json!({"action":"batch","actions":[p]});
            p["actions"].as_array_mut().unwrap()
        };
        if actions.is_empty() || actions.len() > 20 {
            return Err("Choose 1–20 actions per request".into());
        }
        let mut order_ids = std::collections::HashSet::new();
        for action in actions.iter_mut() {
            let kind = s(action, "action")?.to_string();
            permission(ses, &kind)?;
            if kind.starts_with("order_") {
                let id = integer(action, "id", 1, i64::MAX)?;
                if !order_ids.insert(id) {
                    return Err("Review one change per order at a time".into());
                }
                let row = sqlx::query("SELECT version,total,status FROM orders WHERE id=?")
                    .bind(id)
                    .fetch_optional(&self.db)
                    .await
                    .map_err(err)?
                    .ok_or("Order not found")?;
                let version: i64 = row.get(0);
                if let Some(expected) = action["version"].as_i64() {
                    if expected != version {
                        return Err("Order changed. Refresh and review it again.".into());
                    }
                }
                action["version"] = json!(version);
                action["current_total"] = json!(row.get::<i64, _>(1));
                if kind == "order_pay" {
                    action["amount"] = json!(row.get::<i64, _>(1));
                }
                if kind == "order_edit" {
                    let mut tx = self.db.begin().await.map_err(err)?;
                    let (items, total) = quote_items(&mut tx, &action["items"]).await?;
                    action["items"] = json!(items);
                    action["quoted_total"] = json!(total);
                    tx.rollback().await.map_err(err)?;
                }
            }
        }
        let p = if actions.len() == 1 {
            actions[0].clone()
        } else {
            p
        };
        sqlx::query("UPDATE sessions SET pending=? WHERE token=?")
            .bind(p.to_string())
            .bind(&ses.token)
            .execute(&self.db)
            .await
            .map_err(err)?;
        Ok(
            json!({"reply":"작업 내용을 확인한 뒤 ‘확인’이라고 말하거나 Confirm을 눌러주세요. / Review, then confirm.","pending":p}),
        )
    }
    pub async fn confirm(&self, ses: &Session) -> Result<Value> {
        // Recheck role at execution time; an AI response cannot preserve revoked access.
        let ses = self.session(&ses.token).await?;
        let mut tx = self.db.begin_with("BEGIN IMMEDIATE").await.map_err(err)?;
        let raw: Option<String> = sqlx::query_scalar("SELECT pending FROM sessions WHERE token=?")
            .bind(&ses.token)
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?;
        let p: Value =
            serde_json::from_str(&raw.ok_or("No change waiting for confirmation")?).map_err(err)?;
        let actions = if p["action"] == "batch" {
            p["actions"].as_array().ok_or("Invalid batch")?.clone()
        } else {
            vec![p]
        };
        for p in &actions {
            permission(&ses, s(p, "action")?)?;
            execute(&mut tx, &ses, p).await?;
            sqlx::query("INSERT INTO audit(wallet,action,detail,created) VALUES(?,?,?,?)")
                .bind(&ses.wallet)
                .bind(s(p, "action")?)
                .bind(p.to_string())
                .bind(now())
                .execute(&mut *tx)
                .await
                .map_err(err)?;
        }
        sqlx::query("UPDATE sessions SET pending=NULL WHERE token=?")
            .bind(&ses.token)
            .execute(&mut *tx)
            .await
            .map_err(err)?;
        tx.commit().await.map_err(err)?;
        Ok(
            json!({"reply":format!("완료 · {}개 작업을 적용했습니다. / Changes applied.",actions.len())}),
        )
    }
}
async fn quote_items(tx: &mut Transaction<'_, Sqlite>, value: &Value) -> Result<(Vec<Value>, i64)> {
    let lines = value
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= 30)
        .ok_or("Order requires 1–30 lines")?;
    let mut total = 0;
    let mut out = vec![];
    for line in lines {
        let qty = integer(line, "quantity", 1, 20)?;
        let id = text(line, "id", 40)?;
        let row =
            sqlx::query("SELECT name,price FROM menu WHERE id=? AND available=1 AND archived=0")
                .bind(&id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(err)?
                .ok_or("Menu item unavailable")?;
        let price: i64 = row.get(1);
        total += price * qty;
        let note = line["note"].as_str().unwrap_or("");
        if note.len() > 300 {
            return Err("Item note is too long".into());
        }
        out.push(
            json!({"id":id,"name":row.get::<String,_>(0),"quantity":qty,"price":price,"note":note}),
        );
    }
    Ok((out, total))
}
async fn execute(tx: &mut Transaction<'_, Sqlite>, ses: &Session, p: &Value) -> Result<()> {
    let action = s(p, "action")?;
    if action.starts_with("order_") {
        let id = integer(p, "id", 1, i64::MAX)?;
        let row = sqlx::query("SELECT * FROM orders WHERE id=?")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(err)?
            .ok_or("Order missing")?;
        let version: i64 = row.get("version");
        if p["version"].as_i64() != Some(version) {
            return Err("Order changed since review. Ask again before confirming.".into());
        }
        let status: String = row.get("status");
        let payment: String = row.get("payment_status");
        let total: i64 = row.get("total");
        match action {
            "order_status" => {
                let next = s(p, "status")?;
                if !matches!(payment.as_str(), "unpaid" | "paid")
                    || (payment == "paid" && next == "cancelled")
                    || !valid_transition(&status, next)
                {
                    return Err("Invalid order transition".into());
                }
                sqlx::query("UPDATE orders SET status=?,version=version+1 WHERE id=?")
                    .bind(if payment == "paid" && next == "served" {
                        "completed"
                    } else {
                        next
                    })
                    .bind(id)
                    .execute(&mut **tx)
                    .await
                    .map_err(err)?;
            }
            "order_edit" => {
                if payment != "unpaid"
                    || matches!(status.as_str(), "cancelled" | "completed" | "refunded")
                {
                    return Err("Paid or closed orders cannot be edited".into());
                }
                let (items, total) = quote_items(tx, &p["items"]).await?;
                if p["quoted_total"].as_i64() != Some(total) {
                    return Err("Menu prices changed. Review the revised order again.".into());
                }
                let table = p["table"].as_i64().unwrap_or(row.get("table_no"));
                let active: bool = sqlx::query_scalar("SELECT active FROM cafe_tables WHERE id=?")
                    .bind(table)
                    .fetch_optional(&mut **tx)
                    .await
                    .map_err(err)?
                    .unwrap_or(false);
                if !active {
                    return Err("Table unavailable".into());
                }
                let note = p["note"].as_str().unwrap_or("");
                if note.len() > 500 {
                    return Err("Note too long".into());
                }
                sqlx::query("UPDATE orders SET items=?,total=?,table_no=?,note=?,version=version+1 WHERE id=?").bind(json!(items).to_string()).bind(total).bind(table).bind(note).bind(id).execute(&mut **tx).await.map_err(err)?;
            }
            "order_pay" => {
                if payment != "unpaid"
                    || matches!(status.as_str(), "cancelled" | "completed" | "refunded")
                {
                    return Err("Order is already paid or closed".into());
                }
                if p["amount"].as_i64() != Some(total) {
                    return Err("Payment amount changed. Review again.".into());
                }
                let method = s(p, "method")?;
                if !["cash", "card", "other"].contains(&method) {
                    return Err("Choose cash, card or other".into());
                }
                let reference = p["reference"].as_str().unwrap_or("");
                if reference.len() > 200 {
                    return Err("Reference too long".into());
                }
                sqlx::query("INSERT INTO payments(order_id,kind,amount,method,reference,wallet,created) VALUES(?,'payment',?,?,?,?,?)").bind(id).bind(total).bind(method).bind(reference).bind(&ses.wallet).bind(now()).execute(&mut **tx).await.map_err(err)?;
                sqlx::query("UPDATE orders SET fulfillment_status=status,status='completed',payment_status='paid',paid_amount=?,paid_at=?,payment_method=?,payment_reference=?,version=version+1 WHERE id=?").bind(total).bind(now()).bind(method).bind(reference).bind(id).execute(&mut **tx).await.map_err(err)?;
            }
            "order_refund" => {
                if payment != "paid" {
                    return Err("Only paid orders can be refunded".into());
                }
                let reason = text(p, "reason", 300)?;
                if row.get::<Option<String>, _>("payment_method").as_deref() == Some("wallet") {
                    let account: String = row.get("payment_reference");
                    if sqlx::query(
                        "UPDATE test_faucet_credits SET balance=balance+? WHERE account=?",
                    )
                    .bind(row.get::<i64, _>("paid_amount"))
                    .bind(account)
                    .execute(&mut **tx)
                    .await
                    .map_err(err)?
                    .rows_affected()
                        != 1
                    {
                        return Err("Wallet refund account missing".into());
                    }
                }
                sqlx::query("INSERT INTO payments(order_id,kind,amount,method,reference,wallet,created) VALUES(?,'refund',?,?,?,?,?)").bind(id).bind(row.get::<i64,_>("paid_amount")).bind(row.get::<Option<String>,_>("payment_method").unwrap_or("other".into())).bind(reason).bind(&ses.wallet).bind(now()).execute(&mut **tx).await.map_err(err)?;
                sqlx::query("UPDATE orders SET status='refunded',payment_status='refunded',version=version+1 WHERE id=?").bind(id).execute(&mut **tx).await.map_err(err)?;
            }
            _ => return Err("Unknown order operation".into()),
        }
        return Ok(());
    }
    match action{
  "draft_mark_published"=>{if sqlx::query("UPDATE menu_drafts SET status='published',version=version+1 WHERE id=? AND version=? AND status='draft'").bind(s(p,"id")?).bind(p["version"].as_i64().ok_or("Draft version required")?).execute(&mut **tx).await.map_err(err)?.rows_affected()!=1{return Err("Draft changed since review".into());}},
  "staff_add"=>{let wallet=identity::normalize_address(s(p,"wallet")?)?;let name=p["name"].as_str().unwrap_or("");if name.len()>80{return Err("Staff name too long".into());}sqlx::query("INSERT INTO staff(wallet,name) VALUES(?,?) ON CONFLICT(wallet) DO UPDATE SET name=excluded.name").bind(wallet).bind(name).execute(&mut **tx).await.map_err(err)?;},
  "staff_remove"=>{let w=identity::normalize_address(s(p,"wallet")?)?;sqlx::query("DELETE FROM staff WHERE wallet=?").bind(w).execute(&mut **tx).await.map_err(err)?;},
  "menu_create"|"menu_update"=>{let id=identifier(p,"id")?;let old=sqlx::query("SELECT * FROM menu WHERE id=? AND archived=0").bind(&id).fetch_optional(&mut **tx).await.map_err(err)?;if action=="menu_create"&&old.is_some(){return Err("Menu ID already exists".into());}if action=="menu_update"&&old.is_none(){return Err("Menu not found".into());}
   let field=|k:&str,default:&str|p[k].as_str().map(str::to_string).or_else(||old.as_ref().map(|r|r.get::<String,_>(k))).unwrap_or(default.into());let name=field("name","");let description=field("description","");let category=field("category","Kitchen");let allergens=field("allergens","");if name.trim().is_empty()||name.len()>100||description.len()>1000||category.trim().is_empty()||category.len()>50||allergens.len()>300{return Err("Invalid menu fields".into());}let price=if p.get("price").is_some(){integer(p,"price",0,1_000_000)?}else{old.as_ref().map(|r|r.get("price")).ok_or("Price required")?};let available=p["available"].as_bool().unwrap_or_else(||old.as_ref().map(|r|r.get("available")).unwrap_or(true));
   if action=="menu_create"{sqlx::query("INSERT INTO menu(id,name,description,category,price,available,allergens) VALUES(?,?,?,?,?,?,?)").bind(id).bind(name).bind(description).bind(category).bind(price).bind(available).bind(allergens).execute(&mut **tx).await.map_err(err)?;}else{sqlx::query("UPDATE menu SET name=?,description=?,category=?,price=?,available=?,allergens=? WHERE id=?").bind(name).bind(description).bind(category).bind(price).bind(available).bind(allergens).bind(id).execute(&mut **tx).await.map_err(err)?;}},
  "menu_delete"=>{if sqlx::query("UPDATE menu SET archived=1,available=0 WHERE id=? AND archived=0").bind(s(p,"id")?).execute(&mut **tx).await.map_err(err)?.rows_affected()==0{return Err("Menu not found".into());}},
  "floorplan_apply"=>{let tables=crate::floorplan::validate_tables(&p["tables"])?;for t in tables{sqlx::query("INSERT INTO cafe_tables(code,kind,seats,x,y) VALUES(?,'standard',?,?,?) ON CONFLICT(code) DO UPDATE SET seats=excluded.seats,x=excluded.x,y=excluded.y").bind(t["code"].as_str().unwrap()).bind(t["seats"].as_i64().unwrap()).bind(t["x"].as_f64().unwrap()).bind(t["y"].as_f64().unwrap()).execute(&mut **tx).await.map_err(err)?;}},
  "table_save"=>{let code=identifier(p,"code")?;let kind=text(p,"kind",60)?;let seats=integer(p,"seats",1,30)?;let active=p["active"].as_bool().unwrap_or(true);if !active{return Err("Use table_disable to deactivate a table".into());}if let Some(id)=p["id"].as_i64(){if sqlx::query("UPDATE cafe_tables SET code=?,kind=?,seats=?,active=1 WHERE id=?").bind(code).bind(kind).bind(seats).bind(id).execute(&mut **tx).await.map_err(err)?.rows_affected()!=1{return Err("Table not found".into());}}else{sqlx::query("INSERT INTO cafe_tables(code,kind,seats) VALUES(?,?,?)").bind(code).bind(kind).bind(seats).execute(&mut **tx).await.map_err(err)?;}},
  "table_batch"=>{let prefix=identifier(p,"prefix")?;let count=integer(p,"count",1,100)?;let start=p["start"].as_i64().unwrap_or(1);if !(1..=10000).contains(&start){return Err("Invalid start ID".into());}let kind=text(p,"kind",60)?;let seats=integer(p,"seats",1,30)?;for n in start..start+count{sqlx::query("INSERT INTO cafe_tables(code,kind,seats) VALUES(?,?,?)").bind(format!("{prefix}{n}")).bind(&kind).bind(seats).execute(&mut **tx).await.map_err(err)?;}},
  "table_disable"=>{let id=integer(p,"id",1,i64::MAX)?;let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE table_no=? AND status NOT IN ('completed','cancelled','refunded')").bind(id).fetch_one(&mut **tx).await.map_err(err)?;if count>0{return Err("Finish or move the open orders before disabling this table".into());}if sqlx::query("UPDATE cafe_tables SET active=0 WHERE id=?").bind(id).execute(&mut **tx).await.map_err(err)?.rows_affected()!=1{return Err("Table not found".into());}},
  "simulation_mode"=>{let enabled=p["enabled"].as_bool().ok_or("Simulation mode requires enabled: true or false")?;sqlx::query("UPDATE shop SET simulation=?,agent_running=0 WHERE id=1").bind(enabled).execute(&mut **tx).await.map_err(err)?;},
  "shop_update"=>{if let Some(v)=p.get("guid"){let guid=if v.is_null(){None}else{Some(uuid::Uuid::parse_str(v.as_str().ok_or("GUID must be a UUID or null")?).map_err(err)?.to_string())};sqlx::query("UPDATE shop SET guid=? WHERE id=1").bind(guid).execute(&mut **tx).await.map_err(err)?;}if p.get("name").is_some(){let name=text(p,"name",100)?;sqlx::query("UPDATE shop SET name=? WHERE id=1").bind(name).execute(&mut **tx).await.map_err(err)?;}if let Some(open)=p["is_open"].as_bool(){sqlx::query("UPDATE shop SET is_open=? WHERE id=1").bind(open).execute(&mut **tx).await.map_err(err)?;}},
  "ai_limits"=>{let daily=integer(p,"daily_token_limit",0,10_000_000)?;let hourly=integer(p,"hourly_token_limit",0,10_000_000)?;let output=integer(p,"max_output_tokens",128,8000)?;sqlx::query("UPDATE shop SET daily_token_limit=?,hourly_token_limit=?,max_output_tokens=? WHERE id=1").bind(daily).bind(hourly).bind(output).execute(&mut **tx).await.map_err(err)?;},
  _=>return Err("Unknown operation".into())
 }
    if matches!(action, "menu_create" | "menu_update") {
        if let Some(path) = p["image_path"].as_str() {
            if !path.starts_with("/media/") || path.contains("..") {
                return Err("Invalid menu image path".into());
            }
            sqlx::query("UPDATE menu SET image_path=? WHERE id=?")
                .bind(path)
                .bind(s(p, "id")?)
                .execute(&mut **tx)
                .await
                .map_err(err)?;
        }
    }
    Ok(())
}
