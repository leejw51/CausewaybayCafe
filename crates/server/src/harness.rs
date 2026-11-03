use crate::*;
impl App {
    pub async fn service_calls(&self, ses: &Session) -> Result<Value> {
        let rows=sqlx::query("SELECT id,table_no,note,status,created FROM service_calls WHERE status='open' AND (?!='guest' OR session_token=?) ORDER BY id").bind(&ses.role).bind(&ses.token).fetch_all(&self.db).await.map_err(err)?;
        Ok(json!(rows.iter().map(|r|json!({"id":r.get::<i64,_>(0),"table":r.get::<i64,_>(1),"note":r.get::<String,_>(2),"status":r.get::<String,_>(3),"created":r.get::<i64,_>(4)})).collect::<Vec<_>>()))
    }
    pub async fn call_staff(&self, ses: &Session, p: &Value) -> Result<Value> {
        let note = p["note"].as_str().unwrap_or("Guest requests assistance");
        if note.len() > 500 {
            return Err("Please keep the request under 500 bytes".into());
        }
        sqlx::query("INSERT OR IGNORE INTO service_calls(session_token,table_no,note,created) VALUES(?,?,?,?)").bind(&ses.token).bind(ses.table).bind(note).bind(now()).execute(&self.db).await.map_err(err)?;
        self.notify_change();
        Ok(
            json!({"reply":"직원을 호출했습니다. / Staff notified.","service_calls":self.service_calls(ses).await?}),
        )
    }
    pub async fn resolve_call(&self, ses: &Session, p: &Value) -> Result<Value> {
        require(ses, "staff")?;
        sqlx::query("UPDATE service_calls SET status='resolved' WHERE id=? AND status='open'")
            .bind(p["id"].as_i64().ok_or("Call ID required")?)
            .execute(&self.db)
            .await
            .map_err(err)?;
        self.notify_change();
        Ok(json!({"reply":"Service request resolved."}))
    }
    pub async fn cancel_order(&self, ses: &Session, p: &Value) -> Result<Value> {
        let mut tx = self.db.begin_with("BEGIN IMMEDIATE").await.map_err(err)?;
        let id = if let Some(id) = p["id"].as_i64() {
            id
        } else {
            let ids=sqlx::query_scalar::<_,i64>("SELECT id FROM orders WHERE session_token=? AND status NOT IN ('cancelled','completed','refunded') ORDER BY id").bind(&ses.token).fetch_all(&mut *tx).await.map_err(err)?;
            if ids.len() != 1 {
                return Ok(
                    json!({"reply":"취소할 주문 번호를 알려주세요. / Specify the order number to cancel.","order_ids":ids}),
                );
            }
            ids[0]
        };
        let row =
            sqlx::query("SELECT status,payment_status FROM orders WHERE id=? AND session_token=?")
                .bind(id)
                .bind(&ses.token)
                .fetch_optional(&mut *tx)
                .await
                .map_err(err)?
                .ok_or("Order not found")?;
        let status: String = row.get(0);
        if status == "cancelled" {
            return Ok(json!({"reply":"Already cancelled."}));
        }
        if status != "new" || row.get::<String, _>(1) != "unpaid" {
            return Ok(
                json!({"reply":"이미 준비 중이거나 결제된 주문입니다. 직원 호출로 취소를 요청해주세요. / Ask staff to cancel a preparing or paid order."}),
            );
        }
        sqlx::query("UPDATE orders SET status='cancelled',version=version+1 WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(err)?;
        sqlx::query(
            "INSERT INTO audit(wallet,action,detail,created) VALUES(?,'guest_order_cancel',?,?)",
        )
        .bind(&ses.wallet)
        .bind(json!({"id":id}).to_string())
        .bind(now())
        .execute(&mut *tx)
        .await
        .map_err(err)?;
        tx.commit().await.map_err(err)?;
        self.notify_change();
        Ok(json!({"reply":format!("주문 #{id} 취소 완료 / Order cancelled."),"ui":"orders"}))
    }
}
