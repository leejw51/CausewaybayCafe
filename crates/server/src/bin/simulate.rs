use cafe_shared::Rpc;
use serde_json::{json, Value};
async fn rpc(
    c: &reqwest::Client,
    url: &str,
    token: &str,
    method: &str,
    payload: Value,
) -> Result<Value, Box<dyn std::error::Error>> {
    let body = cafe_shared::encode(&Rpc {
        token: token.into(),
        method: method.into(),
        payload,
    })?;
    let res = c
        .post(format!("{url}/api/rpc"))
        .header("Content-Type", "application/x-capnp")
        .body(body)
        .send()
        .await?
        .bytes()
        .await?;
    let r = cafe_shared::decode(&res)?;
    if r.payload["ok"] != true {
        return Err(r.payload.to_string().into());
    }
    Ok(r.payload["data"].clone())
}
const OWNER: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const STAFF: &str = "test test test test test test test test test test test junk";
type Error = Box<dyn std::error::Error>;
async fn login(c: &reqwest::Client, url: &str, words: &str) -> Result<String, Error> {
    let identity = cafe_shared::identity::Identity::from_mnemonic(words)?;
    let challenge = rpc(
        c,
        url,
        "",
        "challenge",
        json!({"wallet":identity.address()}),
    )
    .await?;
    let signature = identity.sign(challenge["message"].as_str().ok_or("Missing challenge")?)?;
    let result = rpc(
        c,
        url,
        "",
        "login",
        json!({"nonce":challenge["nonce"],"signature":signature}),
    )
    .await?;
    Ok(result["token"].as_str().ok_or("Missing session")?.into())
}
async fn apply(c: &reqwest::Client, url: &str, token: &str, action: Value) -> Result<(), Error> {
    let state = rpc(c, url, token, "state", json!({})).await?;
    if state["simulation"] != true {
        return Err("Simulation client refuses a live cafe".into());
    }
    rpc(c, url, token, "action", action).await?;
    rpc(c, url, token, "confirm", json!({})).await?;
    Ok(())
}
async fn order(c: reqwest::Client, url: String, i: usize, delay: u64) -> Result<(), Error> {
    // Separate staff sessions prevent concurrent review/confirmation from sharing pending actions.
    let staff = login(&c, &url, STAFF).await?;
    let guest = rpc(&c, &url, "", "guest", json!({"table":i%8+1})).await?;
    let token = guest["token"].as_str().ok_or("Missing guest token")?;
    let state = rpc(&c, &url, token, "state", json!({})).await?;
    if state["simulation"] != true {
        return Err("Simulation client refuses a live cafe".into());
    }
    let item = ["milk-tea", "egg-tart", "pineapple-bun"][i % 3];
    let quantity = i % 3 + 1;
    let price = state["menu"]
        .as_array()
        .ok_or("Missing menu")?
        .iter()
        .find(|m| m["id"] == item)
        .ok_or("Missing menu item")?["price"]
        .as_i64()
        .ok_or("Missing price")?;
    rpc(
        &c,
        &url,
        token,
        "cart_add",
        json!({"id":item,"quantity":quantity}),
    )
    .await?;
    let request_id = format!("simulation-{}-{i}", std::process::id());
    let placed = rpc(
        &c,
        &url,
        token,
        "checkout",
        json!({"request_id":request_id}),
    )
    .await?;
    let id = placed["order_id"].as_i64().ok_or("Missing order")?;
    let repeat = rpc(
        &c,
        &url,
        token,
        "checkout",
        json!({"request_id":request_id}),
    )
    .await?;
    if repeat["order_id"] != id {
        return Err("Duplicate checkout created another order".into());
    }
    for status in ["preparing", "ready", "served"] {
        apply(
            &c,
            &url,
            &staff,
            json!({"action":"order_status","id":id,"status":status}),
        )
        .await?;
        if delay > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
        }
    }
    apply(
        &c,
        &url,
        &staff,
        json!({"action":"order_pay","id":id,"method":"cash","reference":"SIMULATION ONLY"}),
    )
    .await?;
    let state = rpc(&c, &url, token, "state", json!({})).await?;
    let orders = state["orders"].as_array().ok_or("Missing orders")?;
    if orders.len() != 1
        || orders[0]["id"] != id
        || orders[0]["status"] != "completed"
        || orders[0]["total"] != price * quantity as i64
    {
        return Err(format!("Order {id} verification failed").into());
    }
    println!("Order #{id}: placed → preparing → ready → served → paid/completed ✓");
    Ok(())
}
#[tokio::main]
async fn main() -> Result<(), Error> {
    use futures_util::{stream, StreamExt, TryStreamExt};
    let c = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let url = std::env::var("CAFE_URL").unwrap_or("http://127.0.0.1:8788".into());
    let parsed = reqwest::Url::parse(&url)?;
    if !matches!(parsed.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")) {
        return Err("Simulation requires a loopback test server".into());
    }
    let owner = login(&c, &url, OWNER).await?;
    let staff = cafe_shared::identity::Identity::from_mnemonic(STAFF)?;
    apply(
        &c,
        &url,
        &owner,
        json!({"action":"staff_add","wallet":staff.address(),"name":"Automatic test service"}),
    )
    .await?;
    let count = std::env::var("CAFE_TEST_ORDERS")
        .unwrap_or("12".into())
        .parse::<usize>()?
        .clamp(1, 1000);
    let concurrency = std::env::var("CAFE_TEST_CONCURRENCY")
        .unwrap_or("1".into())
        .parse::<usize>()?
        .clamp(1, 32);
    let delay = std::env::var("CAFE_TEST_DELAY_MS")
        .unwrap_or("250".into())
        .parse::<u64>()?
        .min(2000);
    stream::iter((0..count).map(|i| order(c.clone(), url.clone(), i, delay)))
        .buffer_unordered(concurrency)
        .try_collect::<Vec<_>>()
        .await?;
    println!("PASS: {count} complete order/service/payment flows; concurrency={concurrency}");
    Ok(())
}
