use crate::*;
fn account(ses: &Session) -> String {
    match &ses.wallet {
        Some(wallet) => format!("wallet:{wallet}"),
        None => format!("guest:{}", ses.token),
    }
}
impl App {
    pub async fn faucet_state(&self, ses: &Session) -> Result<Value> {
        if !self.faucet_enabled {
            return Ok(json!({"enabled":false}));
        }
        let balance: Option<i64> =
            sqlx::query_scalar("SELECT balance FROM test_faucet_credits WHERE account=?")
                .bind(account(ses))
                .fetch_optional(&self.db)
                .await
                .map_err(err)?;
        Ok(
            json!({"enabled":true,"test_only":true,"currency":"HKD","grant":100000,"balance":balance.unwrap_or(0),"claimed":balance.is_some(),"login_required":ses.wallet.is_none(),"scope":if ses.wallet.is_some(){"wallet"}else{"guest_session"}}),
        )
    }
    pub async fn claim_faucet(&self, ses: &Session) -> Result<Value> {
        if !self.faucet_enabled {
            return Err("Test faucet is disabled".into());
        }
        if ses.wallet.is_none() {
            return Err(
                "Please log in to receive wallet credits. / 로그인 후 받을 수 있습니다.".into(),
            );
        }
        // The account and amount are server-defined. A unique account prevents double grants,
        // including simultaneous requests from multiple sessions of the same wallet.
        let credited=sqlx::query("INSERT OR IGNORE INTO test_faucet_credits(account,balance,claimed_at) VALUES(?,100000,?)")
            .bind(account(ses)).bind(now()).execute(&self.db).await.map_err(err)?.rows_affected()==1;
        let state = self.faucet_state(ses).await?;
        if credited {
            self.notify_change();
        }
        Ok(
            json!({"reply":if credited{"HK$1000 test credit added. No real money was transferred."}else{"Your free HK$1000 test credit has already been claimed."},"credited":credited,"faucet":state,"ui":"faucet"}),
        )
    }
}
