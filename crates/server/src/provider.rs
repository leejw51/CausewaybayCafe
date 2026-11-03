use crate::*;
impl App {
    pub async fn muse_key(&self) -> Result<String> {
        let key: Option<String> =
            sqlx::query_scalar("SELECT value FROM provider_secrets WHERE name='muse'")
                .fetch_optional(&self.db)
                .await
                .map_err(err)?;
        key.or_else(|| std::env::var("MUSE_API_KEY").ok())
            .filter(|v| !v.is_empty())
            .ok_or("MUSE_API_KEY is not configured".into())
    }
    pub async fn provider_config(&self, s: &Session) -> Result<Value> {
        if s.role != "owner" {
            return Ok(Value::Null);
        }
        let overridden: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM provider_secrets WHERE name='muse')")
                .fetch_one(&self.db)
                .await
                .map_err(err)?;
        let configured = self.muse_key().await.is_ok();
        Ok(
            json!({"configured":configured,"source":if overridden{"owner"}else if configured{"environment"}else{"missing"}}),
        )
    }
    pub async fn set_provider_key(&self, s: &Session, p: &Value) -> Result<Value> {
        require(s, "owner")?;
        let key = p["key"].as_str().ok_or("Key required")?.trim();
        if !key.is_empty()
            && (!(8..=512).contains(&key.len()) || key.chars().any(char::is_whitespace))
        {
            return Err("Invalid API key format".into());
        }
        if key.is_empty() {
            sqlx::query("DELETE FROM provider_secrets WHERE name='muse'")
                .execute(&self.db)
                .await
                .map_err(err)?;
        } else {
            sqlx::query("INSERT INTO provider_secrets(name,value) VALUES('muse',?) ON CONFLICT(name) DO UPDATE SET value=excluded.value").bind(key).execute(&self.db).await.map_err(err)?;
        }
        self.notify_change();
        Ok(json!({"reply":"Meta API settings saved. / Meta API 설정을 저장했습니다."}))
    }
}
