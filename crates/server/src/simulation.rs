use crate::*;
impl App {
    /// Guest practice controls; never grant staff permissions or accept another guest's order.
    pub async fn simulation_order(&self, ses: &Session, p: &Value) -> Result<Value> {
        if ses.role != "guest" {
            return Err("Use staff order controls".into());
        }
        let id = p["id"].as_i64().ok_or("Order ID required")?;
        let action = s(p, "action")?;
        if !matches!(action, "ready" | "pay") {
            return Err("Choose ready or pay".into());
        }
        let mut tx = self.db.begin_with("BEGIN IMMEDIATE").await.map_err(err)?;
        let simulation: bool = sqlx::query_scalar("SELECT simulation FROM shop WHERE id=1")
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?;
        if !simulation {
            return Err("Guest practice controls require Simulation mode".into());
        }
        let row = sqlx::query(
            "SELECT status,payment_status,total FROM orders WHERE id=? AND session_token=?",
        )
        .bind(id)
        .bind(&ses.token)
        .fetch_optional(&mut *tx)
        .await
        .map_err(err)?
        .ok_or("Order not found")?;
        let status: String = row.get("status");
        let payment: String = row.get("payment_status");
        if (action == "ready" && status == "ready")
            || (action == "pay" && status == "completed" && payment == "paid")
        {
            return Ok(json!({"reply":"Already applied.","order_id":id}));
        }
        if payment != "unpaid" {
            return Err("Order is already paid or closed".into());
        }
        if action == "ready" {
            if !matches!(status.as_str(), "new" | "preparing") {
                return Err("Order cannot be marked ready".into());
            }
            sqlx::query("UPDATE orders SET status='ready',version=version+1 WHERE id=?")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(err)?;
        } else {
            if !matches!(status.as_str(), "ready" | "served") {
                return Err("Mark food ready before paying".into());
            }
            let total: i64 = row.get("total");
            sqlx::query("INSERT INTO payments(order_id,kind,amount,method,reference,wallet,created) VALUES(?,'payment',?,'other','SIMULATION ONLY','simulation-guest',?)")
                .bind(id).bind(total).bind(now()).execute(&mut *tx).await.map_err(err)?;
            sqlx::query("UPDATE orders SET fulfillment_status='served',status='completed',payment_status='paid',paid_amount=?,paid_at=?,payment_method='other',payment_reference='SIMULATION ONLY',version=version+1 WHERE id=?")
                .bind(total).bind(now()).bind(id).execute(&mut *tx).await.map_err(err)?;
        }
        sqlx::query("INSERT INTO audit(wallet,action,detail,created) VALUES(?,?,?,?)")
            .bind(&ses.wallet)
            .bind(format!("simulation_guest_{action}"))
            .bind(json!({"order_id":id}).to_string())
            .bind(now())
            .execute(&mut *tx)
            .await
            .map_err(err)?;
        tx.commit().await.map_err(err)?;
        self.notify_change();
        Ok(
            json!({"reply":if action=="ready"{"Food is ready."}else{"Simulation payment complete. Finished eating. No real money was charged."},"order_id":id}),
        )
    }
}

impl App {
    pub async fn simulation_state(&self, ses: &Session) -> Result<Value> {
        if ses.role == "guest" {
            return Ok(Value::Null);
        }
        let r = sqlx::query("SELECT * FROM service_simulation WHERE id=1")
            .fetch_one(&self.db)
            .await
            .map_err(err)?;
        Ok(
            json!({"running":r.get::<bool,_>("running"),"speed":r.get::<i64,_>("speed"),"elapsed":r.get::<i64,_>("elapsed"),"generated":r.get::<i64,_>("generated")}),
        )
    }
    pub async fn simulation_control(&self, ses: &Session, p: &Value) -> Result<Value> {
        require(ses, "owner")?;
        let running = p["running"].as_bool().ok_or("running is required")?;
        let speed = p["speed"]
            .as_i64()
            .filter(|v| [1, 10, 100].contains(v))
            .ok_or("Choose 1, 10 or 100")?;
        let mut tx = self.db.begin_with("BEGIN IMMEDIATE").await.map_err(err)?;
        let enabled: bool = sqlx::query_scalar("SELECT simulation FROM shop WHERE id=1")
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?;
        if !enabled {
            return Err("Use a demo shop for service simulation".into());
        }
        sqlx::query("UPDATE service_simulation SET running=?,speed=?,run_id=CASE WHEN run_id='' THEN ? ELSE run_id END WHERE id=1").bind(running).bind(speed).bind(uid()).execute(&mut *tx).await.map_err(err)?;
        tx.commit().await.map_err(err)?;
        self.notify_change();
        Ok(
            json!({"reply":format!("Service simulation {} · {speed}×. No AI calls or real payments.",if running{"running"}else{"paused"})}),
        )
    }
    /// Advance virtual service time once per wall-clock second. Never touches non-simulator orders.
    pub async fn service_tick(&self) -> Result<()> {
        let mut tx = self.db.begin_with("BEGIN IMMEDIATE").await.map_err(err)?;
        let row=sqlx::query("SELECT s.*,shop.simulation FROM service_simulation s JOIN shop ON shop.id=1 WHERE s.id=1").fetch_one(&mut *tx).await.map_err(err)?;
        if !row.get::<bool, _>("simulation") {
            sqlx::query("UPDATE service_simulation SET running=0 WHERE id=1")
                .execute(&mut *tx)
                .await
                .map_err(err)?;
            tx.commit().await.map_err(err)?;
            return Ok(());
        }
        if !row.get::<bool, _>("running") {
            return Ok(());
        }
        let elapsed = row.get::<i64, _>("elapsed") + row.get::<i64, _>("speed");
        let run: String = row.get("run_id");
        let generated: i64 = row.get("generated");
        if elapsed >= row.get::<i64, _>("next_arrival") && generated < 300 {
            let table: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM cafe_tables WHERE active=1 ORDER BY id LIMIT 1 OFFSET ?",
            )
            .bind(
                generated
                    % sqlx::query_scalar::<_, i64>(
                        "SELECT COUNT(*) FROM cafe_tables WHERE active=1",
                    )
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(err)?
                    .max(1),
            )
            .fetch_optional(&mut *tx)
            .await
            .map_err(err)?;
            let menu=sqlx::query("SELECT id,name,price FROM menu WHERE available=1 AND archived=0 ORDER BY id LIMIT 1").fetch_optional(&mut *tx).await.map_err(err)?;
            if let (Some(table), Some(menu)) = (table, menu) {
                let token = format!("sim-{}", uid());
                let items = json!([{"id":menu.get::<String,_>(0),"name":menu.get::<String,_>(1),"price":menu.get::<i64,_>(2),"quantity":2,"note":"SIMULATION ONLY"}]);
                sqlx::query("INSERT INTO sessions(token,table_no,expires) VALUES(?,?,?)")
                    .bind(&token)
                    .bind(table)
                    .bind(now() + 86400)
                    .execute(&mut *tx)
                    .await
                    .map_err(err)?;
                let order=sqlx::query("INSERT INTO orders(session_token,table_no,items,total,created,request_id,simulation) VALUES(?,?,?,?,?,?,1)").bind(token).bind(table).bind(items.to_string()).bind(menu.get::<i64,_>(2)*2).bind(now()).bind(uid()).execute(&mut *tx).await.map_err(err)?.last_insert_rowid();
                sqlx::query("INSERT INTO simulated_orders(order_id,run_id,born) VALUES(?,?,?)")
                    .bind(order)
                    .bind(&run)
                    .bind(elapsed)
                    .execute(&mut *tx)
                    .await
                    .map_err(err)?;
                sqlx::query(
                    "UPDATE service_simulation SET generated=generated+1,next_arrival=? WHERE id=1",
                )
                .bind(elapsed + 180)
                .execute(&mut *tx)
                .await
                .map_err(err)?;
            }
        }
        let orders=sqlx::query("SELECT o.id,o.status,o.total,s.born FROM orders o JOIN simulated_orders s ON s.order_id=o.id WHERE s.run_id=? AND o.status NOT IN ('completed','cancelled','refunded')").bind(run).fetch_all(&mut *tx).await.map_err(err)?;
        for order in orders {
            let age = elapsed - order.get::<i64, _>("born");
            let status = if age >= 600 {
                "completed"
            } else if age >= 480 {
                "served"
            } else if age >= 360 {
                "ready"
            } else if age >= 60 {
                "preparing"
            } else {
                "new"
            };
            if status == order.get::<String, _>("status") {
                continue;
            }
            let id: i64 = order.get("id");
            if status == "completed" {
                sqlx::query("INSERT INTO payments(order_id,kind,amount,method,reference,wallet,created) VALUES(?,'payment',?,'other','ACCELERATED SIMULATION ONLY','simulation',?)").bind(id).bind(order.get::<i64,_>("total")).bind(now()).execute(&mut *tx).await.map_err(err)?;
                sqlx::query("UPDATE orders SET status='completed',payment_status='paid',fulfillment_status='served',paid_amount=total,paid_at=?,version=version+1 WHERE id=?").bind(now()).bind(id).execute(&mut *tx).await.map_err(err)?;
            } else {
                sqlx::query(
                    "UPDATE orders SET status=?,fulfillment_status=?,version=version+1 WHERE id=?",
                )
                .bind(status)
                .bind(status)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(err)?;
            }
        }
        sqlx::query("UPDATE service_simulation SET elapsed=?,running=CASE WHEN generated>=300 AND NOT EXISTS(SELECT 1 FROM orders o JOIN simulated_orders s ON s.order_id=o.id WHERE o.status NOT IN ('completed','cancelled','refunded')) THEN 0 ELSE running END WHERE id=1").bind(elapsed).execute(&mut *tx).await.map_err(err)?;
        tx.commit().await.map_err(err)?;
        self.flush_journal().await?;
        self.notify_change();
        Ok(())
    }
}
