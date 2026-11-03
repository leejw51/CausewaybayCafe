use crate::*;
fn day_start(t: i64) -> i64 {
    (t + 32400).div_euclid(86400) * 86400 - 32400
}
impl App {
    pub async fn usage(&self, ses: &Session) -> Result<Value> {
        self.usage_at(ses, now()).await
    }
    pub async fn usage_at(&self, ses: &Session, t: i64) -> Result<Value> {
        if ses.role == "guest" {
            return Ok(Value::Null);
        }
        let day = day_start(t);
        let mut result = json!({"day_timezone":"Asia/Seoul","day_start":day});
        for (label, start) in [("day", day), ("hour", t - 3600)] {
            let r=sqlx::query("SELECT COALESCE(SUM(input_tokens),0),COALESCE(SUM(output_tokens),0),COALESCE(SUM(reserved),0),COUNT(*),COALESCE(SUM(CASE WHEN kind='image' THEN 1 ELSE 0 END),0),COALESCE(SUM(CASE WHEN kind='voice' THEN 1 ELSE 0 END),0),COALESCE(SUM(estimated),0) FROM ai_usage WHERE created>=? AND created<=?").bind(start).bind(t).fetch_one(&self.db).await.map_err(err)?;
            result[label] = json!({"input":r.get::<i64,_>(0),"output":r.get::<i64,_>(1),"reserved":r.get::<i64,_>(2),"requests":r.get::<i64,_>(3),"images":r.get::<i64,_>(4),"voice":r.get::<i64,_>(5),"estimated_requests":r.get::<i64,_>(6)});
        }
        Ok(result)
    }
    pub async fn reserve_ai(
        &self,
        ses: &Session,
        kind: &str,
        model: &str,
        estimate: i64,
    ) -> Result<String> {
        let _lock = self.budget.lock().await;
        let shop = self.shop().await?;
        if shop["simulation"] == true
            && !(ses.role == "guest"
                && matches!(kind, "chat" | "image")
                && std::env::var("CAFE_GUEST_AI").as_deref() == Ok("1"))
            && !(ses.role != "guest"
                && kind == "chat"
                && std::env::var("CAFE_MANAGEMENT_AI").as_deref() == Ok("1"))
        {
            return Err("Simulation mode: paid AI services are disabled. Owner can enable Live mode in Shop management.".into());
        }
        // Guest messages and explicitly requested order pictures are on-demand work.
        // Background staff/menu jobs still require Run.
        let guest_message = ses.role == "guest" && matches!(kind, "chat" | "image");
        if shop["agent_running"] != true && !guest_message {
            return Err("AI is stopped. STAFF or OWNER must press Run.".into());
        }
        let t = now();
        let day = (t + 32400) / 86400 * 86400 - 32400;
        for (start, key) in [(day, "daily_token_limit"), (t - 3600, "hourly_token_limit")] {
            let used:i64=sqlx::query_scalar("SELECT COALESCE(SUM(input_tokens+output_tokens+reserved),0) FROM ai_usage WHERE created>=?").bind(start).fetch_one(&self.db).await.map_err(err)?;
            let limit = shop[key].as_i64().unwrap_or(0);
            if limit == 0 || used >= limit || used + estimate > limit {
                return Err(format!(
                    "AI token budget reached ({key}). Manual controls remain available."
                ));
            }
        }
        if kind == "image" {
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM ai_usage WHERE kind='image' AND created>=?",
            )
            .bind(t - 3600)
            .fetch_one(&self.db)
            .await
            .map_err(err)?;
            if count >= 6 {
                return Err("Image limit reached: 6 requests per hour.".into());
            }
        }
        let id = uid();
        sqlx::query("INSERT INTO ai_usage(id,session_token,kind,model,reserved,created) VALUES(?,?,?,?,?,?)").bind(&id).bind(&ses.token).bind(kind).bind(model).bind(estimate).bind(t).execute(&self.db).await.map_err(err)?;
        self.notify_change();
        Ok(id)
    }
    pub async fn finish_ai(&self, id: &str, body: Option<&Value>, status: &str) -> Result<()> {
        let usage = body.and_then(|v| v.get("usage"));
        let input = usage.and_then(|v| {
            v["prompt_tokens"]
                .as_i64()
                .or_else(|| v["input_tokens"].as_i64())
        });
        let output = usage.and_then(|v| {
            v["completion_tokens"]
                .as_i64()
                .or_else(|| v["output_tokens"].as_i64())
        });
        if let (Some(i), Some(o)) = (input, output) {
            sqlx::query(
                "UPDATE ai_usage SET input_tokens=?,output_tokens=?,reserved=0,status=? WHERE id=?",
            )
            .bind(i.max(0))
            .bind(o.max(0))
            .bind(status)
            .bind(id)
            .execute(&self.db)
            .await
            .map_err(err)?;
        } else {
            sqlx::query("UPDATE ai_usage SET input_tokens=reserved,output_tokens=0,reserved=0,estimated=1,status=? WHERE id=?").bind(status).bind(id).execute(&self.db).await.map_err(err)?;
        }
        self.notify_change();
        Ok(())
    }
    pub async fn completion(
        &self,
        ses: &Session,
        messages: Value,
        kind: &str,
        image: bool,
    ) -> Result<Value> {
        let message = self.model_message(ses, messages, kind, image, None).await?;
        serde_json::from_str(
            message["content"]
                .as_str()
                .ok_or("AI response empty or output limit reached")?,
        )
        .map_err(|_| "AI result incomplete. Increase the output cap or ask for fewer items.".into())
    }

    /// Native assistant/tool messages for the guest agent; None retains structured JSON for staff.
    pub(crate) async fn model_message(
        &self,
        ses: &Session,
        messages: Value,
        kind: &str,
        image: bool,
        tools: Option<Value>,
    ) -> Result<Value> {
        let provider = std::env::var("CAFE_AI_PROVIDER").unwrap_or("meta".into());
        let (url, key_env, default) = match provider.as_str() {
            "meta" => ("https://api.meta.ai/v1", "MUSE_API_KEY", "muse-spark-1.3"),
            "openai" => (
                "https://api.openai.com/v1",
                "OPENAI_API_KEY",
                "gpt-4.1-mini",
            ),
            "grok" => ("https://api.x.ai/v1", "XAI_API_KEY", "grok-3-mini"),
            _ => return Err("Unsupported provider".into()),
        };
        let key = if provider == "meta" {
            self.muse_key().await?
        } else {
            std::env::var(key_env)
                .ok()
                .filter(|v| !v.is_empty())
                .ok_or(format!("{key_env} is not configured"))?
        };
        let model = std::env::var("CAFE_AI_MODEL").unwrap_or(default.into());
        let max = self.shop().await?["max_output_tokens"]
            .as_i64()
            .unwrap_or(1200);
        // Count text conservatively by UTF-8 bytes; base64 image data is not text tokens.
        let mut compact = messages.clone();
        if let Some(a) = compact.as_array_mut() {
            for m in a {
                if let Some(parts) = m["content"].as_array_mut() {
                    parts.retain(|p| p["type"] != "image_url");
                }
            }
        }
        let reserve = self
            .reserve_ai(
                ses,
                kind,
                &model,
                compact.to_string().len() as i64
                    + tools
                        .as_ref()
                        .map(|v| v.to_string().len() as i64)
                        .unwrap_or(0)
                    + max
                    + 256
                    + if image { 12000 } else { 0 },
            )
            .await?;
        let mut request = json!({"model":model,"messages":messages,"max_completion_tokens":max});
        match tools {
            None => {
                request["response_format"] = json!({"type":"json_object"});
            }
            Some(tools) if tools.as_array().is_some_and(|a| !a.is_empty()) => {
                request["tools"] = tools;
                request["tool_choice"] = json!("auto");
            }
            Some(_) => {}
        }
        // Keep simple guest interactions within the configured output cap, including reasoning.
        if provider == "meta" && ses.role == "guest" && matches!(kind, "chat" | "image") {
            request["reasoning_effort"] = json!("minimal");
        }
        let res = self
            .client
            .post(format!("{url}/chat/completions"))
            .bearer_auth(key)
            .json(&request)
            .send()
            .await;
        let response = match res {
            Ok(r) => r,
            Err(_) => {
                self.finish_ai(&reserve, None, "network_error").await?;
                return Err(
                    "AI connection failed; request reserved usage was counted conservatively."
                        .into(),
                );
            }
        };
        let status = response.status();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        self.finish_ai(
            &reserve,
            Some(&body),
            if status.is_success() {
                "complete"
            } else {
                "error"
            },
        )
        .await?;
        if !status.is_success() {
            return Err(format!(
                "AI provider returned {status}. No action executed."
            ));
        }
        let choice = &body["choices"][0];
        if choice["finish_reason"] == "length" {
            return Err("AI output limit reached before a complete response".into());
        }
        let message = choice["message"].clone();
        if message["content"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty())
            && !message["tool_calls"]
                .as_array()
                .is_some_and(|a| !a.is_empty())
        {
            return Err("AI response empty or output limit reached".into());
        }
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kst_day_boundary() {
        assert_eq!(day_start(53999), -32400);
        assert_eq!(day_start(54000), 54000);
        assert_eq!(day_start(54001), 54000);
    }
}
