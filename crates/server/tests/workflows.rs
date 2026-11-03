use cafe_server::*;
use cafe_shared::{identity::Identity, Rpc};
use serde_json::{json, Value};
const PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
async fn call(app: &App, token: &str, method: &str, p: Value) -> Result<Value> {
    app.call(Rpc {
        token: token.into(),
        method: method.into(),
        payload: p,
    })
    .await
}
async fn guest(app: &App, table: i64) -> String {
    call(app, "", "guest", json!({"table":table}))
        .await
        .unwrap()["token"]
        .as_str()
        .unwrap()
        .into()
}
async fn login(app: &App, phrase: &str) -> String {
    let id = Identity::from_mnemonic(phrase).unwrap();
    let c = call(app, "", "challenge", json!({"wallet":id.address()}))
        .await
        .unwrap();
    let sig = id.sign(c["message"].as_str().unwrap()).unwrap();
    call(
        app,
        "",
        "login",
        json!({"nonce":c["nonce"],"signature":sig}),
    )
    .await
    .unwrap()["token"]
        .as_str()
        .unwrap()
        .into()
}
#[tokio::test]
async fn ordering_is_priced_by_server_private_and_idempotent() {
    let app = init(":memory:", "", true).await.unwrap();
    let a = guest(&app, 4).await;
    let b = guest(&app, 5).await;
    assert!(
        call(&app, &a, "cart_add", json!({"id":"milk-tea","quantity":-1}))
            .await
            .is_err()
    );
    call(
        &app,
        &a,
        "cart_add",
        json!({"id":"milk-tea","quantity":2,"price":1}),
    )
    .await
    .unwrap();
    let first = call(&app, &a, "checkout", json!({"request_id":"one"}))
        .await
        .unwrap();
    let second = call(&app, &a, "checkout", json!({"request_id":"one"}))
        .await
        .unwrap();
    assert_eq!(first["order_id"], second["order_id"]);
    let state = call(&app, &a, "state", json!({})).await.unwrap();
    assert_eq!(state["orders"][0]["total"], 5600);
    assert_eq!(state["orders"][0]["table"], 4);
    assert_eq!(state["cart"], json!([]));
    assert_eq!(
        call(&app, &b, "state", json!({})).await.unwrap()["orders"],
        json!([])
    );
    assert!(call(
        &app,
        &a,
        "action",
        json!({"action":"staff_add","wallet":"0x0000000000000000000000000000000000000000"})
    )
    .await
    .is_err());
    assert!(call(
        &app,
        &a,
        "action",
        json!({"action":"order_status","id":1,"status":"served"})
    )
    .await
    .is_err());
}
#[tokio::test]
async fn owner_and_staff_permissions_confirmation_revocation_and_transitions() {
    let identity = Identity::from_mnemonic(PHRASE).unwrap();
    let app = init(":memory:", &identity.address(), true).await.unwrap();
    let owner = login(&app, PHRASE).await;
    let staff_id = Identity::from_mnemonic_at(PHRASE, 1).unwrap();
    // Sign for indexed staff identity without transmitting a mnemonic.
    let c = call(&app, "", "challenge", json!({"wallet":staff_id.address()}))
        .await
        .unwrap();
    let sig = staff_id.sign(c["message"].as_str().unwrap()).unwrap();
    let staff = call(
        &app,
        "",
        "login",
        json!({"nonce":c["nonce"],"signature":sig}),
    )
    .await
    .unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &app,
        &owner,
        "action",
        json!({"action":"staff_add","wallet":staff_id.address()}),
    )
    .await
    .unwrap();
    assert_eq!(app.session(&staff).await.unwrap().role, "guest");
    call(&app, &owner, "confirm", json!({})).await.unwrap();
    assert_eq!(app.session(&staff).await.unwrap().role, "staff");
    assert!(call(
        &app,
        &staff,
        "action",
        json!({"action":"menu_update","id":"milk-tea","price":1,"available":true})
    )
    .await
    .is_err());
    call(
        &app,
        &staff,
        "cart_add",
        json!({"id":"egg-tart","quantity":1}),
    )
    .await
    .unwrap();
    call(&app, &staff, "checkout", json!({"request_id":"order"}))
        .await
        .unwrap();
    call(
        &app,
        &staff,
        "action",
        json!({"action":"order_status","id":1,"status":"served"}),
    )
    .await
    .unwrap();
    assert!(call(&app, &staff, "confirm", json!({})).await.is_err());
    for status in ["preparing", "ready", "served"] {
        call(
            &app,
            &staff,
            "action",
            json!({"action":"order_status","id":1,"status":status}),
        )
        .await
        .unwrap();
        call(&app, &staff, "confirm", json!({})).await.unwrap();
    }
    assert_eq!(
        call(&app, &owner, "state", json!({})).await.unwrap()["dashboard"]["sales"],
        0
    );
    call(
        &app,
        &staff,
        "action",
        json!({"action":"order_pay","id":1,"method":"cash"}),
    )
    .await
    .unwrap();
    call(&app, &staff, "confirm", json!({})).await.unwrap();
    assert_eq!(
        call(&app, &owner, "state", json!({})).await.unwrap()["orders"][0]["status"],
        "completed"
    );
    call(
        &app,
        &owner,
        "action",
        json!({"action":"staff_remove","wallet":staff_id.address()}),
    )
    .await
    .unwrap();
    call(&app, &owner, "confirm", json!({})).await.unwrap();
    assert_eq!(app.session(&staff).await.unwrap().role, "guest");
    assert_eq!(
        call(&app, &owner, "state", json!({})).await.unwrap()["dashboard"]["sales"],
        1800
    );
}
#[tokio::test]
async fn signature_replay_and_forgery_rejected() {
    let id = Identity::from_mnemonic(PHRASE).unwrap();
    let app = init(":memory:", &id.address(), true).await.unwrap();
    let c = call(&app, "", "challenge", json!({"wallet":id.address()}))
        .await
        .unwrap();
    let p =
        json!({"nonce":c["nonce"],"signature":id.sign(c["message"].as_str().unwrap()).unwrap()});
    assert!(call(&app, "", "login", p.clone()).await.is_ok());
    assert!(call(&app, "", "login", p).await.is_err());
    let c = call(&app, "", "challenge", json!({"wallet":id.address()}))
        .await
        .unwrap();
    assert!(call(
        &app,
        "",
        "login",
        json!({"nonce":c["nonce"],"signature":id.sign("forged").unwrap()})
    )
    .await
    .is_err());
    assert!(call(&app, "invented-token", "state", json!({}))
        .await
        .is_err());
}
#[tokio::test]
async fn price_changes_require_new_review_and_migrations_survive_restart() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("cafe.sqlite");
    let app = init(path.to_str().unwrap(), "", true).await.unwrap();
    let token = guest(&app, 2).await;
    call(
        &app,
        &token,
        "cart_add",
        json!({"id":"milk-tea","quantity":1}),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE menu SET price=3000 WHERE id='milk-tea'")
        .execute(&app.db)
        .await
        .unwrap();
    assert!(call(&app, &token, "checkout", json!({"request_id":"a"}))
        .await
        .is_err());
    app.db.close().await;
    let app = init(path.to_str().unwrap(), "", true).await.unwrap();
    assert_eq!(
        call(&app, &token, "state", json!({})).await.unwrap()["cart"][0]["price"],
        2800
    );
}
#[tokio::test]
async fn simultaneous_confirmations_create_only_one_order() {
    let app = init(":memory:", "", true).await.unwrap();
    let token = guest(&app, 1).await;
    call(
        &app,
        &token,
        "cart_add",
        json!({"id":"egg-tart","quantity":1}),
    )
    .await
    .unwrap();
    let (a, b) = tokio::join!(
        call(&app, &token, "checkout", json!({"request_id":"a"})),
        call(&app, &token, "checkout", json!({"request_id":"b"}))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
}
#[tokio::test]
#[ignore = "uses the configured Meta API key and network"]
async fn live_meta_chat() {
    dotenvy::from_path("../../.env").ok();
    dotenvy::dotenv().ok();
    assert!(
        std::env::var("MUSE_API_KEY").is_ok(),
        "MUSE_API_KEY missing"
    );
    let app = init(":memory:", "", false).await.unwrap();
    assert_eq!(app.shop().await.unwrap()["agent_running"], false);
    let token = guest(&app, 1).await;
    let result = call(
        &app,
        &token,
        "chat",
        json!({"text":"Add exactly two Hong Kong milk teas to my basket, with no special instructions. Do not check out yet.","language":"en"}),
    )
    .await
    .unwrap();
    assert!(!result["reply"].as_str().unwrap_or("").is_empty());
    let state = call(&app, &token, "state", json!({})).await.unwrap();
    assert_eq!(state["cart"][0]["id"], "milk-tea");
    assert_eq!(state["cart"][0]["quantity"], 2);
    call(
        &app,
        &token,
        "chat",
        json!({"text":"Confirm order.","language":"en"}),
    )
    .await
    .unwrap();
    assert_eq!(
        call(&app, &token, "state", json!({})).await.unwrap()["orders"][0]["total"],
        5600
    );
}

#[tokio::test]
async fn voice_confirmation_accepts_transcription_punctuation() {
    let app = init(":memory:", "", true).await.unwrap();
    let token = guest(&app, 3).await;
    call(
        &app,
        &token,
        "cart_add",
        json!({"id":"milk-tea","quantity":1}),
    )
    .await
    .unwrap();
    call(
        &app,
        &token,
        "chat",
        json!({"text":"確認訂單。","language":"yue"}),
    )
    .await
    .unwrap();
    assert_eq!(
        call(&app, &token, "state", json!({})).await.unwrap()["orders"][0]["total"],
        2800
    );
}

async fn apply(app: &App, token: &str, p: Value) -> Result<Value> {
    call(app, token, "action", p).await?;
    call(app, token, "confirm", json!({})).await
}
#[tokio::test]
async fn stale_orders_and_duplicate_payments_cannot_be_applied() {
    let identity = Identity::from_mnemonic(PHRASE).unwrap();
    let app = init(":memory:", &identity.address(), true).await.unwrap();
    let a = login(&app, PHRASE).await;
    let b = login(&app, PHRASE).await;
    call(&app, &a, "cart_add", json!({"id":"milk-tea","quantity":1}))
        .await
        .unwrap();
    call(&app, &a, "checkout", json!({"request_id":"x"}))
        .await
        .unwrap();
    call(
        &app,
        &a,
        "action",
        json!({"action":"order_pay","id":1,"method":"cash"}),
    )
    .await
    .unwrap();
    apply(&app,&b,json!({"action":"order_edit","id":1,"items":[{"id":"egg-tart","quantity":2}],"note":"updated"})).await.unwrap();
    assert!(call(&app, &a, "confirm", json!({})).await.is_err());
    apply(
        &app,
        &a,
        json!({"action":"order_pay","id":1,"method":"card"}),
    )
    .await
    .unwrap();
    assert!(apply(
        &app,
        &b,
        json!({"action":"order_pay","id":1,"method":"cash"})
    )
    .await
    .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payments")
            .fetch_one(&app.db)
            .await
            .unwrap(),
        1
    );
    let s = call(&app, &a, "state", json!({})).await.unwrap();
    assert_eq!(s["orders"][0]["paid_amount"], 3600);
    assert_eq!(s["dashboard"]["sales"], 3600);
    apply(
        &app,
        &a,
        json!({"action":"order_refund","id":1,"reason":"Cash returned to customer"}),
    )
    .await
    .unwrap();
    assert_eq!(
        call(&app, &a, "state", json!({})).await.unwrap()["dashboard"]["sales"],
        0
    );
}
#[tokio::test]
async fn management_batches_are_atomic_and_qr_uses_table_codes() {
    let identity = Identity::from_mnemonic(PHRASE).unwrap();
    let app = init(":memory:", &identity.address(), true).await.unwrap();
    let owner = login(&app, PHRASE).await;
    let before = app.tables().await.unwrap().as_array().unwrap().len();
    assert!(apply(&app,&owner,json!({"action":"batch","actions":[{"action":"table_save","code":"P1","kind":"patio","seats":4},{"action":"table_save","code":"P1","kind":"patio","seats":4}]})).await.is_err());
    assert_eq!(
        app.tables().await.unwrap().as_array().unwrap().len(),
        before
    );
    apply(
        &app,
        &owner,
        json!({"action":"table_batch","prefix":"P","count":2,"kind":"patio","seats":4}),
    )
    .await
    .unwrap();
    let token = call(&app, "", "guest", json!({"table":"P1"}))
        .await
        .unwrap();
    assert!(!token["token"].is_null());
    assert!(app.qr_svg("P1").await.unwrap().contains("<svg"));
    let guid = "21a33f60-8e8d-47a1-89e6-a2d8049e79ab";
    apply(
        &app,
        &owner,
        json!({"action":"shop_update","guid":guid,"is_open":false}),
    )
    .await
    .unwrap();
    assert!(
        call(&app, "", "guest", json!({"table":"P1","shop_guid":"wrong"}))
            .await
            .is_err()
    );
    let guest = token["token"].as_str().unwrap();
    call(
        &app,
        guest,
        "cart_add",
        json!({"id":"milk-tea","quantity":1}),
    )
    .await
    .unwrap();
    assert!(
        call(&app, guest, "checkout", json!({"request_id":"closed"}))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn run_gate_and_token_reservations_enforce_shared_budgets() {
    let identity = Identity::from_mnemonic(PHRASE).unwrap();
    let app = init(":memory:", &identity.address(), false).await.unwrap();
    let owner = login(&app, PHRASE).await;
    let g = guest(&app, 1).await;
    let ses = app.session(&owner).await.unwrap();
    assert!(app.reserve_ai(&ses, "chat", "test", 100).await.is_err());
    assert!(call(&app, &g, "agent_run", json!({"running":true}))
        .await
        .is_err());
    call(&app, &owner, "agent_run", json!({"running":true}))
        .await
        .unwrap();
    apply(&app,&owner,json!({"action":"ai_limits","daily_token_limit":1000,"hourly_token_limit":500,"max_output_tokens":128})).await.unwrap();
    let (a, b) = tokio::join!(
        app.reserve_ai(&ses, "chat", "test", 300),
        app.reserve_ai(&ses, "chat", "test", 300)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let id = a.or(b).unwrap();
    app.finish_ai(
        &id,
        Some(&json!({"usage":{"prompt_tokens":100,"completion_tokens":50}})),
        "complete",
    )
    .await
    .unwrap();
    let usage = app.usage(&ses).await.unwrap();
    assert_eq!(usage["hour"]["input"], 100);
    assert_eq!(usage["day"]["output"], 50);
    assert_eq!(usage["hour"]["reserved"], 0);
    sqlx::query("UPDATE ai_usage SET created=created-3700")
        .execute(&app.db)
        .await
        .unwrap();
    assert_eq!(app.usage(&ses).await.unwrap()["hour"]["requests"], 0);
    call(&app, &owner, "agent_run", json!({"running":false}))
        .await
        .unwrap();
    assert!(app.reserve_ai(&ses, "image", "test", 100).await.is_err());
}
#[tokio::test]
async fn draft_prices_require_review_and_publish_checks_version() {
    let identity = Identity::from_mnemonic(PHRASE).unwrap();
    let app = init(":memory:", &identity.address(), true).await.unwrap();
    let owner = login(&app, PHRASE).await;
    let items = json!([{"id":"photo-tea","name":"Photo tea","price":null,"category":"Drinks","allergens":"Check with kitchen"}]);
    sqlx::query(
        "INSERT INTO menu_drafts(id,author,prompt,items,created) VALUES('draft',?,'photo',?,0)",
    )
    .bind(identity.address())
    .bind(items.to_string())
    .execute(&app.db)
    .await
    .unwrap();
    assert!(call(&app, &owner, "draft_publish", json!({"id":"draft"}))
        .await
        .is_err());
    let mut items = items;
    items[0]["price"] = json!(2800);
    call(
        &app,
        &owner,
        "draft_save",
        json!({"id":"draft","version":1,"items":items}),
    )
    .await
    .unwrap();
    call(&app, &owner, "draft_publish", json!({"id":"draft"}))
        .await
        .unwrap();
    items[0]["price"] = json!(3000);
    call(
        &app,
        &owner,
        "draft_save",
        json!({"id":"draft","version":2,"items":items}),
    )
    .await
    .unwrap();
    assert!(call(&app, &owner, "confirm", json!({})).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM menu WHERE id='photo-tea'")
            .fetch_one(&app.db)
            .await
            .unwrap(),
        0
    );
    call(&app, &owner, "draft_publish", json!({"id":"draft"}))
        .await
        .unwrap();
    call(&app, &owner, "confirm", json!({})).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT price FROM menu WHERE id='photo-tea'")
            .fetch_one(&app.db)
            .await
            .unwrap(),
        3000
    );
}

#[tokio::test]
#[ignore = "uses Meta vision and one image generation request"]
async fn live_photo_menu_and_image() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    dotenvy::from_path("../../.env").ok();
    dotenvy::dotenv().ok();
    let identity = Identity::from_mnemonic(PHRASE).unwrap();
    let app = init(":memory:", &identity.address(), false).await.unwrap();
    let owner = login(&app, PHRASE).await;
    call(&app, &owner, "agent_run", json!({"running":true}))
        .await
        .unwrap();
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/menu-photo.jpg"
    ))
    .unwrap();
    let image = format!("data:image/jpeg;base64,{}", STANDARD.encode(bytes));
    let result=call(&app,&owner,"menu_draft",json!({"prompt":"Read all three menu items and exact printed HKD prices. Missing prices must be null.","image":image})).await.unwrap();
    let state = call(&app, &owner, "state", json!({})).await.unwrap();
    let items = state["drafts"][0]["items"].as_array().unwrap();
    assert!(items.iter().any(|i| i["price"] == 2800));
    assert!(items.iter().any(|i| i["price"] == 1800));
    assert!(items.iter().any(|i| i["price"].is_null()));
    app.image_tick().await.unwrap();
    let jobs: Vec<(String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT status,image_path,error FROM image_jobs WHERE draft_id=?")
            .bind(result["draft_id"].as_str().unwrap())
            .fetch_all(&app.db)
            .await
            .unwrap();
    assert!(
        jobs.iter().any(|j| j.0 == "complete"),
        "Image did not complete: {jobs:?}"
    );
    let usage = app
        .usage(&app.session(&owner).await.unwrap())
        .await
        .unwrap();
    println!("Live photo + image usage: {}", usage["hour"]);
    for (_, path, _) in jobs {
        if let Some(path) = path {
            let _ = std::fs::remove_file(format!("data{}", path));
        }
    }
}

#[tokio::test]
async fn concurrent_guests_checkout_on_file_database_without_lock_errors() {
    let temp = tempfile::tempdir().unwrap();
    let app = init(
        temp.path().join("concurrent.sqlite").to_str().unwrap(),
        "",
        true,
    )
    .await
    .unwrap();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(16));
    let mut tasks = Vec::new();
    for n in 0..16 {
        let token = guest(&app, n % 8 + 1).await;
        call(
            &app,
            &token,
            "cart_add",
            json!({"id":"egg-tart","quantity":1}),
        )
        .await
        .unwrap();
        let app = app.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            call(
                &app,
                &token,
                "checkout",
                json!({"request_id":format!("parallel-{n}")}),
            )
            .await
        }));
    }
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM orders")
            .fetch_one(&app.db)
            .await
            .unwrap(),
        16
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT SUM(total) FROM orders")
            .fetch_one(&app.db)
            .await
            .unwrap(),
        16 * 1800
    );
}

#[tokio::test]
async fn guest_messages_activate_on_demand_without_enabling_background_ai() {
    let app = init(":memory:", "", false).await.unwrap();
    let token = guest(&app, 1).await;
    let ses = app.session(&token).await.unwrap();
    assert_eq!(app.shop().await.unwrap()["agent_running"], false);
    let id = app.reserve_ai(&ses, "chat", "test", 100).await.unwrap();
    app.finish_ai(
        &id,
        Some(&json!({"usage":{"input_tokens":25,"output_tokens":10}})),
        "complete",
    )
    .await
    .unwrap();
    let image = app.reserve_ai(&ses, "image", "test", 100).await.unwrap();
    app.finish_ai(
        &image,
        Some(&json!({"usage":{"input_tokens":50,"output_tokens":20}})),
        "complete",
    )
    .await
    .unwrap();
    for kind in ["voice", "menu_draft", "photo_menu"] {
        assert!(app.reserve_ai(&ses, kind, "test", 100).await.is_err());
    }
    sqlx::query("UPDATE shop SET simulation=1")
        .execute(&app.db)
        .await
        .unwrap();
    call(&app, &token, "chat", json!({"text":"2 milk tea"}))
        .await
        .unwrap();
    assert_eq!(
        call(&app, &token, "state", json!({})).await.unwrap()["cart"][0]["quantity"],
        2
    );
    call(&app, &token, "chat", json!({"text":"confirm order"}))
        .await
        .unwrap();
    assert_eq!(
        call(&app, &token, "state", json!({})).await.unwrap()["orders"][0]["total"],
        5600
    );
    assert_eq!(app.shop().await.unwrap()["agent_running"], false);
    sqlx::query("UPDATE shop SET hourly_token_limit=0")
        .execute(&app.db)
        .await
        .unwrap();
    assert!(app.reserve_ai(&ses, "chat", "test", 100).await.is_err());
}

#[tokio::test]
async fn default_customer_functions_are_authoritative_and_session_scoped() {
    let app = init(":memory:", "", true).await.unwrap();
    let a = guest(&app, 1).await;
    let b = guest(&app, 2).await;
    let menu = call(&app, &a, "menu_list", json!({})).await.unwrap();
    assert!(menu["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["id"] == "milk-tea" && i["price"] == 2800));
    call(
        &app,
        &a,
        "choose_menu",
        json!({"id":"milk-tea","quantity":2,"price":1}),
    )
    .await
    .unwrap();
    call(
        &app,
        &a,
        "choose_menu",
        json!({"id":"milk-tea","quantity":1,"note":"iced"}),
    )
    .await
    .unwrap();
    assert!(call(
        &app,
        &a,
        "cancel_menu",
        json!({"id":"milk-tea","quantity":0})
    )
    .await
    .is_err());
    assert!(call(&app, &b, "cancel_menu", json!({"id":"milk-tea"}))
        .await
        .is_err());
    let r = call(
        &app,
        &a,
        "cancel_menu",
        json!({"id":"milk-tea","quantity":2}),
    )
    .await
    .unwrap();
    assert_eq!(r["cart"][0]["quantity"], 1);
    assert_eq!(r["cart"][0]["note"], "iced");
    call(&app, &a, "chat", json!({"text":"current progress"}))
        .await
        .unwrap();
    assert_eq!(
        call(&app, &a, "current_progress", json!({})).await.unwrap()["orders"],
        json!([])
    );
    call(&app, &a, "chat", json!({"text":"confirm order"}))
        .await
        .unwrap();
    assert_eq!(
        call(&app, &a, "current_progress", json!({})).await.unwrap()["orders"][0]["total"],
        2800
    );
    assert_eq!(
        call(&app, &b, "current_progress", json!({"id":1}))
            .await
            .unwrap()["orders"],
        json!([])
    );
    assert!(call(&app, &a, "cancel_menu", json!({"id":"milk-tea"}))
        .await
        .is_err());
    assert_eq!(app.shop().await.unwrap()["agent_running"], false);
    for command in ["menu list", "choose menu", "can menu"] {
        assert!(call(&app, &a, "chat", json!({"text":command}))
            .await
            .is_ok());
    }
}

#[tokio::test]
async fn authenticated_change_stream_waits_for_mutations_not_state_reads() {
    use futures_util::StreamExt;
    use tower::ServiceExt;
    let app = init(":memory:", "", true).await.unwrap();
    let token = guest(&app, 1).await;
    let router = router(app.clone());
    let unauthorized = router
        .clone()
        .oneshot(
            http::Request::post("/api/events")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), 401);
    let response = router
        .oneshot(
            http::Request::post("/api/events")
                .header("Authorization", format!("Bearer {token}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let mut stream = response.into_body().into_data_stream();
    let initial = stream.next().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&initial).contains("data: refresh"));
    call(&app, &token, "state", json!({})).await.unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), stream.next())
            .await
            .is_err()
    );
    call(&app, &token, "choose_menu", json!({"id":"egg-tart"}))
        .await
        .unwrap();
    call(
        &app,
        &token,
        "checkout",
        json!({"request_id":"event-check"}),
    )
    .await
    .unwrap();
    let event = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&event).contains("data: refresh"));
    assert!(!String::from_utf8_lossy(&event).contains(&token));
}

#[tokio::test]
async fn faucet_is_fixed_isolated_idempotent_and_persistent_for_wallets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("faucet.sqlite");
    let app = init(path.to_str().unwrap(), "", true).await.unwrap();
    let a = login(&app, PHRASE).await;
    let b = login(&app, PHRASE).await;
    let stranger = guest(&app, 1).await;
    let (first, second) = tokio::join!(
        call(
            &app,
            &a,
            "faucet_claim",
            json!({"amount":999999,"wallet":"other"})
        ),
        call(&app, &b, "faucet_claim", json!({}))
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(
        usize::from(first["credited"] == true) + usize::from(second["credited"] == true),
        1
    );
    assert_eq!(first["faucet"]["balance"], 100000);
    assert_eq!(second["faucet"]["balance"], 100000);
    assert_eq!(
        call(&app, &stranger, "state", json!({})).await.unwrap()["faucet"]["balance"],
        0
    );
    assert!(call(&app, "invalid", "faucet_claim", json!({}))
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payments")
            .fetch_one(&app.db)
            .await
            .unwrap(),
        0
    );
    app.db.close().await;
    let mut app = init(path.to_str().unwrap(), "", true).await.unwrap();
    assert_eq!(
        call(&app, &a, "state", json!({})).await.unwrap()["faucet"]["balance"],
        100000
    );
    assert_eq!(
        call(&app, &a, "faucet_claim", json!({})).await.unwrap()["credited"],
        false
    );
    app.faucet_enabled = false;
    assert!(call(&app, &a, "faucet_claim", json!({})).await.is_err());
    assert_eq!(
        call(&app, &a, "state", json!({})).await.unwrap()["faucet"]["enabled"],
        false
    );
}

#[tokio::test]
async fn simulation_mode_is_owner_only_persistent_and_blocks_paid_ai() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mode.sqlite");
    let identity = Identity::from_mnemonic(PHRASE).unwrap();
    let app = init(path.to_str().unwrap(), &identity.address(), true)
        .await
        .unwrap();
    let owner = login(&app, PHRASE).await;
    let g = guest(&app, 1).await;
    assert_eq!(app.shop().await.unwrap()["simulation"], true);
    let session = app.session(&g).await.unwrap();
    assert!(app
        .reserve_ai(&session, "chat", "test", 1)
        .await
        .unwrap_err()
        .contains("Simulation"));
    assert!(apply(
        &app,
        &g,
        json!({"action":"simulation_mode","enabled":false})
    )
    .await
    .is_err());
    let staff_phrase = "test test test test test test test test test test test junk";
    let staff_id = Identity::from_mnemonic(staff_phrase).unwrap();
    apply(
        &app,
        &owner,
        json!({"action":"staff_add","wallet":staff_id.address(),"name":"Staff"}),
    )
    .await
    .unwrap();
    let staff = login(&app, staff_phrase).await;
    assert!(apply(
        &app,
        &staff,
        json!({"action":"simulation_mode","enabled":false})
    )
    .await
    .is_err());
    call(&app, &owner, "agent_run", json!({"running":true}))
        .await
        .unwrap();
    apply(
        &app,
        &owner,
        json!({"action":"simulation_mode","enabled":false}),
    )
    .await
    .unwrap();
    assert_eq!(app.shop().await.unwrap()["simulation"], false);
    assert_eq!(app.shop().await.unwrap()["agent_running"], false);
    app.db.close().await;
    let app = init(path.to_str().unwrap(), &identity.address(), true)
        .await
        .unwrap();
    assert_eq!(app.shop().await.unwrap()["simulation"], false);
    call(&app, &owner, "chat", json!({"text":"simulation mode on"}))
        .await
        .unwrap();
    call(&app, &owner, "confirm", json!({})).await.unwrap();
    assert_eq!(app.shop().await.unwrap()["simulation"], true);
}

#[tokio::test]
async fn guest_simulation_controls_are_owned_gated_and_idempotent() {
    let app = init(":memory:", "", true).await.unwrap();
    let guest = guest(&app, 1).await;
    let other = call(&app, "", "guest", json!({})).await.unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    call(
        &app,
        &guest,
        "cart_add",
        json!({"id":"egg-tart","quantity":1}),
    )
    .await
    .unwrap();
    let id = call(&app, &guest, "checkout", json!({"request_id":"practice"}))
        .await
        .unwrap()["order_id"]
        .clone();
    assert!(call(
        &app,
        &guest,
        "simulation_order",
        json!({"id":id,"action":"pay"})
    )
    .await
    .is_err());
    assert!(call(
        &app,
        &other,
        "simulation_order",
        json!({"id":id,"action":"ready"})
    )
    .await
    .is_err());
    sqlx::query("UPDATE shop SET simulation=0")
        .execute(&app.db)
        .await
        .unwrap();
    assert!(call(
        &app,
        &guest,
        "simulation_order",
        json!({"id":id,"action":"ready"})
    )
    .await
    .is_err());
    sqlx::query("UPDATE shop SET simulation=1")
        .execute(&app.db)
        .await
        .unwrap();
    for _ in 0..2 {
        call(
            &app,
            &guest,
            "simulation_order",
            json!({"id":id,"action":"ready"}),
        )
        .await
        .unwrap();
    }
    let (a, b) = tokio::join!(
        call(
            &app,
            &guest,
            "simulation_order",
            json!({"id":id,"action":"pay"})
        ),
        call(
            &app,
            &guest,
            "simulation_order",
            json!({"id":id,"action":"pay"})
        )
    );
    a.unwrap();
    b.unwrap();
    let state = call(&app, &guest, "state", json!({})).await.unwrap();
    assert_eq!(state["orders"][0]["status"], "completed");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM payments")
            .fetch_one(&app.db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ai_usage")
            .fetch_one(&app.db)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn group_chat_suggests_exact_counts_without_submitting_or_overwriting() {
    let app = init(":memory:", "", true).await.unwrap();
    let token = guest(&app, 1).await;
    let reply = call(
        &app,
        &token,
        "chat",
        json!({"text":"3명 메뉴 3개, 음료수 3개","language":"ko"}),
    )
    .await
    .unwrap();
    assert_eq!(reply["function"], "recommend_group");
    assert_eq!(reply["total"], 23800);
    let state = call(&app, &token, "state", json!({})).await.unwrap();
    let cart = state["cart"].as_array().unwrap();
    assert_eq!(
        cart.iter()
            .map(|i| i["quantity"].as_i64().unwrap())
            .sum::<i64>(),
        6
    );
    assert_eq!(state["orders"].as_array().unwrap().len(), 0);
    call(
        &app,
        &token,
        "chat",
        json!({"text":"3명 메뉴 3개, 음료수 3개"}),
    )
    .await
    .unwrap();
    assert_eq!(
        call(&app, &token, "state", json!({})).await.unwrap()["cart"],
        state["cart"]
    );
    call(&app, &token, "chat", json!({"text":"주문 확인"}))
        .await
        .unwrap();
    assert_eq!(
        call(&app, &token, "state", json!({})).await.unwrap()["orders"][0]["total"],
        23800
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ai_usage")
            .fetch_one(&app.db)
            .await
            .unwrap(),
        0
    );
    assert!(group_order::parse_group("3명 메뉴 3개 음료수 3개 땅콩 알레르기").is_none());
    assert!(group_order::parse_group("3명 메뉴 3개 음료수 3개 100달러 이하").is_none());
    let other = guest(&app, 2).await;
    assert!(call(
        &app,
        &other,
        "chat",
        json!({"text":"3명 메뉴 999개 음료수 3개"})
    )
    .await
    .is_err());
    sqlx::query("UPDATE menu SET available=0 WHERE category='Drinks'")
        .execute(&app.db)
        .await
        .unwrap();
    call(
        &app,
        &other,
        "chat",
        json!({"text":"3명 메뉴 3개 음료수 3잔"}),
    )
    .await
    .unwrap();
    assert_eq!(
        call(&app, &other, "state", json!({})).await.unwrap()["cart"],
        json!([])
    );
}

#[tokio::test]
async fn recommendations_use_recent_demand_and_growth_without_cross_mode_pollution() {
    async fn seed(app: &App, id: &str, qty: i64, age: i64, status: &str, mode: Option<bool>) {
        sqlx::query("INSERT INTO orders(session_token,table_no,items,total,status,created,request_id,simulation) VALUES('trend-fixture',1,?,100,?,?,?,?)")
            .bind(json!([{"id":id,"quantity":qty}]).to_string()).bind(status).bind(now()-age).bind(uid()).bind(mode).execute(&app.db).await.unwrap();
    }
    let app = init(":memory:", "", true).await.unwrap();
    seed(&app, "rice", 6, 60, "ready", Some(true)).await;
    seed(&app, "macaroni", 10, 2 * 86400, "completed", Some(true)).await;
    seed(&app, "lemon-tea", 4, 60, "served", Some(true)).await;
    seed(&app, "milk-tea", 10, 2 * 86400, "completed", Some(true)).await;
    seed(&app, "milk-tea", 100, 8 * 86400, "completed", Some(true)).await;
    for status in ["cancelled", "refunded"] {
        seed(&app, "yuenyeung", 999, 60, status, Some(true)).await;
    }
    seed(
        &app,
        "french-toast",
        999,
        15 * 86400,
        "completed",
        Some(true),
    )
    .await;
    seed(&app, "french-toast", 999, 60, "completed", None).await;
    seed(&app, "macaroni", 999, 60, "completed", Some(false)).await;
    let token = guest(&app, 1).await;
    let result = call(
        &app,
        &token,
        "chat",
        json!({"text":"1명 메뉴 1개 음료수 1개"}),
    )
    .await
    .unwrap();
    assert_eq!(result["recommendation_basis"], "shop_order_trends");
    assert_eq!(result["trends"][0]["id"], "rice");
    assert_eq!(result["trends"][0]["last_hour"], 6);
    assert_eq!(result["trends"][1]["id"], "lemon-tea");
    assert_eq!(result["total"], 8800);
    call(&app, &token, "cart_clear", json!({})).await.unwrap();
    sqlx::query("UPDATE menu SET available=0 WHERE id='rice'")
        .execute(&app.db)
        .await
        .unwrap();
    let result = call(
        &app,
        &token,
        "chat",
        json!({"text":"1명 메뉴 1개 음료수 1개"}),
    )
    .await
    .unwrap();
    assert_eq!(result["trends"][0]["id"], "macaroni");
    assert_eq!(result["trends"][0]["last_7_days"], 10);
    call(&app, &token, "cart_clear", json!({})).await.unwrap();
    sqlx::query("UPDATE shop SET simulation=0")
        .execute(&app.db)
        .await
        .unwrap();
    let result = call(
        &app,
        &token,
        "recommend_group",
        json!({"people":1,"food":1,"drinks":1,"language":"ko"}),
    )
    .await
    .unwrap();
    assert_eq!(result["trends"][0]["last_7_days"], 999);
    assert_eq!(result["trends"][1]["id"], "milk-tea");
    assert_eq!(result["trends"][1]["last_7_days"], 0);
    call(
        &app,
        &token,
        "checkout",
        json!({"request_id":"trend-mode-check"}),
    )
    .await
    .unwrap();
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT simulation FROM orders WHERE session_token=?")
            .bind(&token)
            .fetch_one(&app.db)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn harness_enforces_order_ownership_and_service_permissions() {
    let app = init(":memory:", "", true).await.unwrap();
    let a = guest(&app, 1).await;
    let b = guest(&app, 2).await;
    call(&app, &a, "cart_add", json!({"id":"milk-tea","quantity":1}))
        .await
        .unwrap();
    let id = call(&app, &a, "checkout", json!({"request_id":"harness"}))
        .await
        .unwrap()["order_id"]
        .clone();
    assert!(call(&app, &b, "order_cancel", json!({"id":id}))
        .await
        .is_err());
    sqlx::query("UPDATE orders SET status='preparing'")
        .execute(&app.db)
        .await
        .unwrap();
    call(&app, &a, "order_cancel", json!({"id":id}))
        .await
        .unwrap();
    assert_eq!(
        call(&app, &a, "state", json!({})).await.unwrap()["orders"][0]["status"],
        "preparing"
    );
    for _ in 0..2 {
        call(&app, &a, "staff_call", json!({"note":"Water please"}))
            .await
            .unwrap();
    }
    let state = call(&app, &a, "state", json!({})).await.unwrap();
    assert_eq!(state["service_calls"].as_array().unwrap().len(), 1);
    assert_eq!(
        call(&app, &b, "state", json!({})).await.unwrap()["service_calls"],
        json!([])
    );
    assert!(call(
        &app,
        &a,
        "service_resolve",
        json!({"id":state["service_calls"][0]["id"]})
    )
    .await
    .is_err());
}

#[tokio::test]
async fn token_dashboard_windows_roles_and_restart_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usage.sqlite");
    let identity = Identity::from_mnemonic(PHRASE).unwrap();
    let app = init(path.to_str().unwrap(), &identity.address(), false)
        .await
        .unwrap();
    let owner = login(&app, PHRASE).await;
    let guest = guest(&app, 1).await;
    let staff_words = "test test test test test test test test test test test junk";
    let staff_identity = Identity::from_mnemonic(staff_words).unwrap();
    apply(
        &app,
        &owner,
        json!({"action":"staff_add","wallet":staff_identity.address(),"name":"Usage staff"}),
    )
    .await
    .unwrap();
    let staff = login(&app, staff_words).await;
    // A fixed instant 2 hours after midnight KST: distinguish rolling-hour and calendar-day data.
    let t = 86400 * 20000 + 54000 + 7200;
    for (created, input, output, reserved, estimated, kind) in [
        (t, 100, 20, 0, 0, "chat"),
        (t - 3600, 30, 10, 0, 0, "image"),
        (t - 3601, 40, 0, 0, 1, "voice"),
        (t - 7201, 999, 0, 0, 0, "chat"),
        (t + 1, 999, 0, 0, 0, "chat"),
        (t - 10, 0, 0, 50, 0, "chat"),
    ] {
        sqlx::query("INSERT INTO ai_usage(id,session_token,kind,model,input_tokens,output_tokens,reserved,estimated,created) VALUES(?,?,?,'test',?,?,?,?,?)").bind(uid()).bind(&guest).bind(kind).bind(input).bind(output).bind(reserved).bind(estimated).bind(created).execute(&app.db).await.unwrap();
    }
    let owner_session = app.session(&owner).await.unwrap();
    let staff_session = app.session(&staff).await.unwrap();
    let u = app.usage_at(&owner_session, t).await.unwrap();
    assert_eq!(u["hour"]["input"], 130);
    assert_eq!(u["hour"]["output"], 30);
    assert_eq!(u["hour"]["reserved"], 50);
    assert_eq!(u["hour"]["requests"], 3);
    assert_eq!(u["day"]["input"], 170);
    assert_eq!(u["day"]["requests"], 4);
    assert_eq!(u["day"]["estimated_requests"], 1);
    assert_eq!(u["day"]["images"], 1);
    assert_eq!(u["day"]["voice"], 1);
    assert_eq!(app.usage_at(&staff_session, t).await.unwrap(), u);
    assert!(app
        .usage_at(&app.session(&guest).await.unwrap(), t)
        .await
        .unwrap()
        .is_null());
    app.db.close().await;
    let reopened = init(path.to_str().unwrap(), &identity.address(), false)
        .await
        .unwrap();
    assert_eq!(
        reopened
            .usage_at(&reopened.session(&owner).await.unwrap(), t)
            .await
            .unwrap(),
        u
    );
    apply(
        &reopened,
        &owner,
        json!({"action":"staff_remove","wallet":staff_identity.address()}),
    )
    .await
    .unwrap();
    assert!(reopened
        .usage_at(&reopened.session(&staff).await.unwrap(), t)
        .await
        .unwrap()
        .is_null());
}

#[tokio::test]
async fn guest_conversational_harness() {
    let app = init(":memory:", "", true).await.unwrap();
    let a = guest(&app, 1).await;
    let b = guest(&app, 2).await;
    for text in ["what's menu?", "현재 메뉴는?", "show menu", "explain menu"] {
        let r = call(&app, &a, "chat", json!({"text":text})).await.unwrap();
        assert_eq!(r["function"], "menu_list");
    }
    call(&app, &a, "chat", json!({"text":"2 milk tea"}))
        .await
        .unwrap();
    let r = call(&app, &a, "chat", json!({"text":"what's my order?"}))
        .await
        .unwrap();
    assert!(r["reply"]
        .as_str()
        .unwrap()
        .contains("2 × Hong Kong milk tea"));
    call(&app, &a, "chat", json!({"text":"explain milk tea"}))
        .await
        .unwrap();
    assert_eq!(
        call(&app, &a, "state", json!({})).await.unwrap()["cart"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    call(&app, &a, "chat", json!({"text":"confirm order"}))
        .await
        .unwrap();
    let r = call(&app, &a, "chat", json!({"text":"현재 주문은?"}))
        .await
        .unwrap();
    assert_eq!(r["orders"].as_array().unwrap().len(), 1);
    assert!(r["reply"].as_str().unwrap().contains("Hong Kong milk tea"));
    assert_eq!(
        call(&app, &b, "chat", json!({"text":"my order"}))
            .await
            .unwrap()["orders"],
        json!([])
    );
    let r = call(&app, &a, "chat", json!({"text":"call a waiter"}))
        .await
        .unwrap();
    assert!(!r["reply"].as_str().unwrap().contains("Simulation"));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM service_calls")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn direct_coffee_wallet_payment_is_atomic_and_login_required_for_faucet() {
    let owner_phrase = "test test test test test test test test test test test junk";
    let owner_id = Identity::from_mnemonic(owner_phrase).unwrap();
    let app = init(":memory:", &owner_id.address(), true).await.unwrap();
    let anonymous = guest(&app, 1).await;
    assert!(call(&app, &anonymous, "faucet_claim", json!({}))
        .await
        .is_err());
    let token = login(&app, PHRASE).await;
    sqlx::query("INSERT INTO menu(id,name,description,category,price,available,allergens) VALUES('coffee','Coffee','Black coffee','Drinks',3000,1,'')").execute(&app.db).await.unwrap();
    assert!(call(&app, &token, "chat", json!({"text":"커피 3잔"}))
        .await
        .is_err());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(count, 0);
    call(&app, &token, "faucet_claim", json!({})).await.unwrap();
    call(
        &app,
        &token,
        "cart_add",
        json!({"id":"milk-tea","quantity":1}),
    )
    .await
    .unwrap();
    let result = call(&app, &token, "chat", json!({"text":"커피 3잔"}))
        .await
        .unwrap();
    assert_eq!(result["paid"], true);
    let state = call(&app, &token, "state", json!({})).await.unwrap();
    assert_eq!(state["faucet"]["balance"], 91000);
    assert_eq!(state["cart"].as_array().unwrap().len(), 1);
    assert_eq!(state["orders"][0]["items"].as_array().unwrap().len(), 1);
    assert_eq!(state["orders"][0]["items"][0]["quantity"], 3);
    assert_eq!(state["orders"][0]["payment_status"], "paid");
    assert_eq!(state["orders"][0]["status"], "new");
    let payments: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM payments")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(payments, 1);
    let owner_token = login(&app, owner_phrase).await;
    let owner = app.session(&owner_token).await.unwrap();
    for status in ["preparing", "ready", "served"] {
        app.propose(
            &owner,
            json!({"action":"order_status","id":result["order_id"],"status":status}),
        )
        .await
        .unwrap();
        app.confirm(&owner).await.unwrap();
    }
    assert_eq!(
        call(&app, &token, "state", json!({})).await.unwrap()["orders"][0]["status"],
        "completed"
    );
    app.propose(
        &owner,
        json!({"action":"order_refund","id":result["order_id"],"reason":"Requested refund"}),
    )
    .await
    .unwrap();
    app.confirm(&owner).await.unwrap();
    assert_eq!(
        call(&app, &token, "state", json!({})).await.unwrap()["faucet"]["balance"],
        100000
    );
    assert!(app.confirm(&owner).await.is_err());
}

#[tokio::test]
async fn korean_guest_tool_results_are_not_bilingual() {
    let app = init(":memory:", "", true).await.unwrap();
    let token = guest(&app, 1).await;
    for text in [
        "현재 메뉴는?",
        "밀크티 2개",
        "현재 주문은?",
        "직원 불러주세요",
        "주문 확인",
    ] {
        let result = call(&app, &token, "chat", json!({"text":text,"language":"ko"}))
            .await
            .unwrap();
        let reply = result["reply"].as_str().unwrap();
        assert!(reply.chars().any(|c| ('가'..='힣').contains(&c)), "{reply}");
        for unwanted in ["Added", "Your", "Staff", "Order #", " / ", "Hong Kong"] {
            assert!(!reply.contains(unwanted), "{reply}");
        }
    }
}

#[tokio::test]
#[ignore = "uses live Meta API and token budget"]
async fn live_meta_korean_delegation_uses_history_and_menu() {
    dotenvy::from_path("../../.env").ok();
    dotenvy::dotenv().ok();
    std::env::set_var("CAFE_GUEST_AI", "1");
    let app = init(":memory:", "", true).await.unwrap();
    let token = guest(&app, 1).await;
    for (role, content) in [
        (
            "user",
            "3명이야. 음식 3개와 음료 3잔을 주문해줘. 메뉴 선택은 네가 해줘. 알레르기는 없어.",
        ),
        ("assistant", "세 분의 음식과 음료를 골라 드릴게요."),
    ] {
        sqlx::query("INSERT INTO messages(session_token,role,content) VALUES(?,?,?)")
            .bind(&token)
            .bind(role)
            .bind(content)
            .execute(&app.db)
            .await
            .unwrap();
    }
    let result = app
        .chat(
            &app.session(&token).await.unwrap(),
            &json!({"text":"너가 알아서 해","language":"ko"}),
        )
        .await
        .unwrap();
    let reply = result["reply"].as_str().unwrap();
    println!("Korean delegation reply: {reply}");
    assert!(reply.chars().any(|c| ('가'..='힣').contains(&c)));
    assert!(!reply.contains("What would you like") && !reply.contains(" / "));
    let state = call(&app, &token, "state", json!({})).await.unwrap();
    assert_eq!(state["orders"].as_array().unwrap().len(), 1, "{reply}");
    let items = state["orders"][0]["items"].as_array().unwrap();
    let total: i64 = items.iter().map(|i| i["quantity"].as_i64().unwrap()).sum();
    assert_eq!(total, 6);
    let mut drinks = 0;
    for i in items {
        let menu = state["menu"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == i["id"])
            .unwrap();
        assert_eq!(menu["available"], true);
        if menu["category"] == "Drinks" {
            drinks += i["quantity"].as_i64().unwrap();
        }
    }
    assert_eq!(drinks, 3);
    assert_eq!(state["shop"]["agent_running"], false);
    let explain = app
        .chat(
            &app.session(&token).await.unwrap(),
            &json!({"text":"방금 주문한 위엔양은 어떤 음료야? 설명만 해줘.","language":"ko"}),
        )
        .await
        .unwrap();
    let reply = explain["reply"].as_str().unwrap();
    println!("Korean explanation: {reply}");
    assert!(reply.contains("커피"));
    assert!(!reply.contains(" / "));
    assert_eq!(
        call(&app, &token, "state", json!({})).await.unwrap()["orders"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
#[ignore = "uses live Meta API for a natural multi-turn conversation"]
async fn live_meta_natural_guest_conversation() {
    dotenvy::from_path("../../.env").ok();
    dotenvy::dotenv().ok();
    std::env::set_var("CAFE_GUEST_AI", "1");
    let app = init(":memory:", "", true).await.unwrap();
    let token = guest(&app, 1).await;
    let ses = app.session(&token).await.unwrap();
    for text in [
        "안녕, 어떤 카페야?",
        "3명, 음료수와 빵 주문해",
        "너가 알아서해",
    ] {
        let result = app
            .chat(&ses, &json!({"text":text,"language":"ko"}))
            .await
            .unwrap();
        println!(
            "Guest: {text}\nPanda: {}",
            result["reply"].as_str().unwrap()
        );
        assert!(!result["reply"]
            .as_str()
            .unwrap()
            .contains("What would you like"));
    }
    let state = app.snapshot(&ses).await.unwrap();
    assert_eq!(
        state["orders"].as_array().unwrap().len(),
        1,
        "Must not create a duplicate order for vague follow-up"
    );
    let items = state["orders"][0]["items"].as_array().unwrap();
    assert_eq!(
        items
            .iter()
            .map(|i| i["quantity"].as_i64().unwrap())
            .sum::<i64>(),
        6
    );
    let mut drinks = 0;
    for item in items {
        let m = state["menu"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == item["id"])
            .unwrap();
        if m["category"] == "Drinks" {
            drinks += item["quantity"].as_i64().unwrap();
        } else {
            assert!(!matches!(m["id"].as_str(), Some("rice" | "macaroni")));
        }
    }
    assert_eq!(drinks, 3);
}

#[tokio::test]
async fn failed_order_keeps_guest_intent_for_followup() {
    let app = init(":memory:", "", true).await.unwrap();
    let token = login(&app, PHRASE).await;
    sqlx::query("INSERT INTO menu(id,name,description,category,price,available,allergens) VALUES('coffee','Coffee','Black coffee','Drinks',3000,1,'')").execute(&app.db).await.unwrap();
    assert!(call(
        &app,
        &token,
        "chat",
        json!({"text":"커피 3잔","language":"ko"})
    )
    .await
    .is_err());
    let state = call(&app, &token, "state", json!({})).await.unwrap();
    assert_eq!(state["messages"].as_array().unwrap().len(), 1);
    assert_eq!(state["messages"][0]["content"], "커피 3잔");
    assert_eq!(state["orders"], json!([]));
}

#[tokio::test]
async fn order_visual_is_private_and_tracks_cart_and_last_order() {
    let app = init(":memory:", "", true).await.unwrap();
    let a = guest(&app, 1).await;
    let b = guest(&app, 1).await;
    assert!(call(&app, &a, "state", json!({})).await.unwrap()["order_visual"].is_null());
    call(&app, &a, "cart_add", json!({"id":"milk-tea","quantity":2}))
        .await
        .unwrap();
    let raw: String = sqlx::query_scalar("SELECT cart FROM sessions WHERE token=?")
        .bind(&a)
        .fetch_one(&app.db)
        .await
        .unwrap();
    let cart: Vec<Value> = serde_json::from_str(&raw).unwrap();
    let basket = json!(cart
        .iter()
        .map(|v| json!({"id":v["id"],"name":v["name"],"quantity":v["quantity"]}))
        .collect::<Vec<_>>())
    .to_string();
    let key = json!([a, basket]).to_string();
    sqlx::query("INSERT INTO order_visuals(id,basket_key,session_token,prompt,created,status,image_path) VALUES('fixture',?,?, 'fixture',0,'complete','/media/fixture.png')").bind(key).bind(&a).execute(&app.db).await.unwrap();
    assert_eq!(
        call(&app, &a, "state", json!({})).await.unwrap()["order_visual"]["status"],
        "complete"
    );
    call(&app, &b, "cart_add", json!({"id":"milk-tea","quantity":2}))
        .await
        .unwrap();
    assert_eq!(
        call(&app, &b, "state", json!({})).await.unwrap()["order_visual"]["status"],
        "available"
    );
    call(&app, &a, "checkout", json!({"request_id":"visual-order"}))
        .await
        .unwrap();
    assert_eq!(
        call(&app, &a, "state", json!({})).await.unwrap()["order_visual"]["status"],
        "complete"
    );
    call(&app, &a, "cart_add", json!({"id":"egg-tart","quantity":1}))
        .await
        .unwrap();
    assert_eq!(
        call(&app, &a, "state", json!({})).await.unwrap()["order_visual"]["status"],
        "available"
    );
}

#[tokio::test]
#[ignore = "Calls paid Meta image generation"]
async fn live_guest_order_visual() {
    dotenvy::from_path("../../.env").ok();
    dotenvy::dotenv().ok();
    let mut app = init(":memory:", "", false).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    app.media_dir = dir.path().into();
    let token = guest(&app, 1).await;
    call(
        &app,
        &token,
        "cart_add",
        json!({"id":"milk-tea","quantity":2}),
    )
    .await
    .unwrap();
    call(
        &app,
        &token,
        "cart_add",
        json!({"id":"egg-tart","quantity":1}),
    )
    .await
    .unwrap();
    call(&app, &token, "order_visual", json!({})).await.unwrap();
    call(&app, &token, "order_visual", json!({})).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(200), async {
        loop {
            let v = call(&app, &token, "state", json!({})).await.unwrap()["order_visual"].clone();
            assert_ne!(v["status"], "paused", "Image failed");
            if v["status"] == "complete" {
                assert!(app
                    .media_dir
                    .join(
                        v["image_path"]
                            .as_str()
                            .unwrap()
                            .trim_start_matches("/media/")
                    )
                    .exists());
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    })
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM order_visuals")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn accelerated_service_is_owner_only_and_never_changes_real_orders() {
    let app = init(
        ":memory:",
        &Identity::from_mnemonic(PHRASE).unwrap().address(),
        true,
    )
    .await
    .unwrap();
    let owner = login(&app, PHRASE).await;
    let guest = guest(&app, 1).await;
    assert!(call(
        &app,
        &guest,
        "simulation_control",
        json!({"running":true,"speed":100})
    )
    .await
    .is_err());
    call(
        &app,
        &guest,
        "cart_add",
        json!({"id":"milk-tea","quantity":1}),
    )
    .await
    .unwrap();
    let real = call(
        &app,
        &guest,
        "checkout",
        json!({"request_id":"human-order"}),
    )
    .await
    .unwrap()["order_id"]
        .as_i64()
        .unwrap();
    call(
        &app,
        &owner,
        "simulation_control",
        json!({"running":true,"speed":100}),
    )
    .await
    .unwrap();
    for _ in 0..15 {
        app.service_tick().await.unwrap();
    }
    let state = call(&app, &owner, "state", json!({})).await.unwrap();
    assert_eq!(state["service_simulation"]["elapsed"], 1500);
    let completed:i64=sqlx::query_scalar("SELECT COUNT(*) FROM orders o JOIN simulated_orders s ON o.id=s.order_id WHERE o.status='completed'").fetch_one(&app.db).await.unwrap();
    assert!(completed > 0);
    let status: String = sqlx::query_scalar("SELECT status FROM orders WHERE id=?")
        .bind(real)
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(status, "new");
    let ai: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_usage")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(ai, 0);
    call(
        &app,
        &owner,
        "simulation_control",
        json!({"running":false,"speed":100}),
    )
    .await
    .unwrap();
    app.service_tick().await.unwrap();
    assert_eq!(
        call(&app, &owner, "state", json!({})).await.unwrap()["service_simulation"]["elapsed"],
        1500
    );
    sqlx::query("UPDATE shop SET simulation=0")
        .execute(&app.db)
        .await
        .unwrap();
    assert!(call(
        &app,
        &owner,
        "simulation_control",
        json!({"running":true,"speed":100})
    )
    .await
    .is_err());
}
#[tokio::test]
async fn floorplan_requires_owner_review_and_valid_coordinates() {
    let app = init(
        ":memory:",
        &Identity::from_mnemonic(PHRASE).unwrap().address(),
        true,
    )
    .await
    .unwrap();
    let owner = login(&app, PHRASE).await;
    let guest = guest(&app, 1).await;
    let plan = json!({"action":"floorplan_apply","tables":[{"code":"P1","seats":4,"x":30,"y":40}]});
    assert!(call(&app, &guest, "action", plan.clone()).await.is_err());
    call(&app, &owner, "action", plan).await.unwrap();
    assert!(!app
        .tables()
        .await
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["code"] == "P1"));
    call(&app, &owner, "confirm", json!({})).await.unwrap();
    let tables = app.tables().await.unwrap();
    let t = tables
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["code"] == "P1")
        .unwrap();
    assert_eq!(t["seats"], 4);
    assert_eq!(t["x"], 30.0);
    assert!(cafe_server::floorplan::validate_tables(
        &json!([{"code":"P2","seats":4,"x":999,"y":40}])
    )
    .is_err());
}
