//! Default customer functions. Results come from cafe data, never model-generated prices/status.
use crate::*;
impl App {
    pub async fn menu_list(&self) -> Result<Value> {
        let items = self.menu().await?;
        let lines = items
            .as_array()
            .ok_or("Invalid menu")?
            .iter()
            .map(|m| {
                format!(
                    "{} · HK${:.2}{}\n{}",
                    m["name"].as_str().unwrap_or(""),
                    m["price"].as_i64().unwrap_or(0) as f64 / 100.,
                    if m["available"] == true {
                        ""
                    } else {
                        " · Sold out"
                    },
                    m["description"].as_str().unwrap_or("")
                )
            })
            .collect::<Vec<_>>();
        Ok(
            json!({"reply":if lines.is_empty(){"No menu items yet.".into()}else{lines.join("\n")},"items":items,"ui":"menu","function":"menu_list"}),
        )
    }
    pub async fn choose_menu(&self, ses: &Session, p: &Value) -> Result<Value> {
        if p["id"].as_str().is_none_or(|id| id.trim().is_empty()) {
            let mut result = self.menu_list().await?;
            result["reply"] = json!(format!(
                "Choose an item and quantity, for example: 2 milk teas.\n{}",
                result["reply"].as_str().unwrap_or("")
            ));
            result["function"] = json!("choose_menu");
            return Ok(result);
        }
        let mut args = p.clone();
        if args["quantity"].is_null() {
            args["quantity"] = json!(1);
        }
        let mut result = self.cart_add(ses, &args).await?;
        result["function"] = json!("choose_menu");
        Ok(result)
    }
    pub async fn cancel_menu(&self, ses: &Session, p: &Value) -> Result<Value> {
        let raw: String = sqlx::query_scalar("SELECT cart FROM sessions WHERE token=?")
            .bind(&ses.token)
            .fetch_one(&self.db)
            .await
            .map_err(err)?;
        let mut cart: Vec<Value> = serde_json::from_str(&raw).map_err(err)?;
        let Some(id) = p["id"].as_str().filter(|id| !id.trim().is_empty()) else {
            let list = cart
                .iter()
                .map(|i| format!("{} × {}", i["quantity"], i["name"].as_str().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(
                json!({"reply":if cart.is_empty(){"Your basket is empty. Ask staff to cancel a submitted order.".into()}else{format!("Which item should I remove from your basket? Tell me its name and optionally a quantity.\n{list}")},"cart":cart,"function":"cancel_menu"}),
            );
        };
        let total: i64 = cart
            .iter()
            .filter(|i| i["id"] == id)
            .map(|i| i["quantity"].as_i64().unwrap_or(0))
            .sum();
        if total == 0 {
            return Err(
                "That item is not in your basket. Ask staff to change a submitted order.".into(),
            );
        }
        let quantity = match p.get("quantity") {
            None => total,
            Some(v) => v
                .as_i64()
                .filter(|n| *n > 0 && *n <= total)
                .ok_or("Choose a positive quantity no greater than the selected quantity")?,
        };
        let mut remaining = quantity;
        for item in cart.iter_mut().filter(|i| i["id"] == id) {
            let old = item["quantity"].as_i64().unwrap_or(0);
            let removed = remaining.min(old);
            item["quantity"] = json!(old - removed);
            remaining -= removed;
        }
        cart.retain(|i| i["quantity"].as_i64().unwrap_or(0) > 0);
        sqlx::query("UPDATE sessions SET cart=?,pending=NULL WHERE token=?")
            .bind(json!(cart).to_string())
            .bind(&ses.token)
            .execute(&self.db)
            .await
            .map_err(err)?;
        Ok(
            json!({"reply":format!("Removed {quantity} × {id} from your basket. Submitted orders are unchanged."),"cart":cart,"function":"cancel_menu"}),
        )
    }
    pub async fn current_progress(&self, ses: &Session) -> Result<Value> {
        // snapshot enforces guest ownership; never accept another session/order owner from arguments.
        let snapshot = self.snapshot(ses).await?;
        let orders = snapshot["orders"].clone();
        let mut lines = orders
            .as_array()
            .ok_or("Invalid orders")?
            .iter()
            .map(|o| {
                format!(
                    "#{} · {} · {} · HK${:.2}\n{}",
                    o["id"],
                    o["status"].as_str().unwrap_or(""),
                    o["payment_status"].as_str().unwrap_or(""),
                    o["total"].as_i64().unwrap_or(0) as f64 / 100.,
                    o["items"]
                        .as_array()
                        .map(|items| items
                            .iter()
                            .map(|i| format!(
                                "{} × {}",
                                i["quantity"],
                                i["name"].as_str().unwrap_or("")
                            ))
                            .collect::<Vec<_>>()
                            .join("\n"))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>();
        if let Some(cart) = snapshot["cart"].as_array().filter(|cart| !cart.is_empty()) {
            lines.push(format!(
                "Basket · not yet ordered / 아직 주문 전\n{}",
                cart.iter()
                    .map(|i| format!("{} × {}", i["quantity"], i["name"].as_str().unwrap_or("")))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
        Ok(
            json!({"reply":if lines.is_empty(){"No submitted orders yet. Choose food, then confirm your basket.".into()}else{lines.join("\n")},"orders":orders,"ui":"orders","function":"current_progress"}),
        )
    }
}

impl App {
    /// A direct order contains only requested lines, never unrelated basket contents.
    pub async fn order_now(&self, ses: &Session, args: &Value) -> Result<Value> {
        self.order_now_keyed(ses, args, &uid()).await
    }
    pub(crate) async fn order_now_keyed(
        &self,
        ses: &Session,
        args: &Value,
        request_id: &str,
    ) -> Result<Value> {
        if ses.role != "guest" {
            return Err("Direct ordering is for guests".into());
        }
        let lines = args["items"].as_array().ok_or("Order items required")?;
        if lines.is_empty() || lines.len() > 20 {
            return Err("Choose 1–20 items".into());
        }
        let menu = self.menu().await?;
        let mut items = Vec::new();
        for line in lines {
            let quantity = line["quantity"].as_i64().ok_or("Quantity required")?;
            if !(1..=20).contains(&quantity) {
                return Err("Choose 1–20 of each item".into());
            }
            let item = menu
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["id"] == line["id"] && m["available"] == true)
                .ok_or("This item is not available")?;
            let note = line["note"].as_str().unwrap_or("");
            if note.len() > 300 {
                return Err("Note is too long".into());
            }
            items.push(json!({"id":item["id"],"name":item["name"],"price":item["price"],"quantity":quantity,"note":note}));
        }
        self.checkout_items(ses, request_id, Some(items)).await
    }
    /// Resolve a plain coffee request to the lowest-priced available coffee, with a stable tie break.
    pub(crate) async fn quick_coffee(&self, ses: &Session, text: &str) -> Result<Option<Value>> {
        if ses.role != "guest" {
            return Ok(None);
        }
        let compact = text.to_lowercase().replace(' ', "");
        let quantity = compact
            .split(|c: char| !c.is_ascii_digit())
            .find_map(|s| s.parse::<i64>().ok());
        let Some(quantity) = quantity else {
            return Ok(None);
        };
        let words = compact
            .chars()
            .filter(|c| !c.is_ascii_digit())
            .collect::<String>();
        if !matches!(
            words.as_str(),
            "커피잔" | "커피개" | "커피" | "coffee" | "coffees"
        ) {
            return Ok(None);
        }
        let menu = self.menu().await?;
        let mut coffees = menu
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| {
                let name = m["name"].as_str().unwrap_or("").to_lowercase();
                m["available"] == true
                    && (name.contains("coffee")
                        || name.contains("커피")
                        || name.contains("americano")
                        || name.contains("아메리카노"))
            })
            .collect::<Vec<_>>();
        coffees.sort_by_key(|m| {
            (
                m["price"].as_i64().unwrap_or(i64::MAX),
                m["id"].as_str().unwrap_or(""),
            )
        });
        match coffees.first() {
            Some(item) => Ok(Some(
                self.order_now(
                    ses,
                    &json!({"items":[{"id":item["id"],"quantity":quantity}]}),
                )
                .await?,
            )),
            None => Ok(Some(
                json!({"reply":"현재 판매 가능한 커피가 없습니다. 메뉴에서 다른 음료를 골라주세요. / Coffee is not currently available. Please choose another drink from the menu."}),
            )),
        }
    }
}

fn ko_name(item: &Value) -> String {
    match item["id"].as_str().unwrap_or("") {
        "milk-tea" => "홍콩 밀크티",
        "egg-tart" => "에그타르트",
        "lemon-tea" => "레몬차",
        "yuenyeung" => "위엔양",
        "pineapple-bun" => "파인애플 번",
        "french-toast" => "프렌치토스트",
        "macaroni" => "마카로니",
        "rice" => "토마토 구운 밥",
        _ => return item["name"].as_str().unwrap_or("메뉴").into(),
    }
    .into()
}
fn ko_lines(items: &Value) -> String {
    items
        .as_array()
        .map(|a| {
            a.iter()
                .map(|i| {
                    format!(
                        "{} {}개 · HK${:.2}",
                        ko_name(i),
                        i["quantity"].as_i64().unwrap_or(1),
                        i["price"].as_i64().unwrap_or(0) as f64
                            * i["quantity"].as_i64().unwrap_or(1) as f64
                            / 100.
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}
impl App {
    pub(crate) async fn korean_guest_reply(&self, ses: &Session, r: &mut Value) -> Result<()> {
        if ses.role != "guest" {
            return Ok(());
        }
        let previous = r["reply"].as_str().unwrap_or("").to_string();
        let reply = match r["function"].as_str().unwrap_or("") {
            "menu_list" => {
                let menu = self.menu().await?;
                format!(
                    "오늘 메뉴예요!\n{}",
                    menu.as_array()
                        .unwrap()
                        .iter()
                        .map(|i| format!(
                            "{} · HK${:.2}{}",
                            ko_name(i),
                            i["price"].as_i64().unwrap_or(0) as f64 / 100.,
                            if i["available"] == true {
                                ""
                            } else {
                                " · 품절"
                            }
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
            "current_progress" => {
                let state = self.snapshot(ses).await?;
                let mut lines = Vec::new();
                for o in state["orders"].as_array().unwrap() {
                    let status = match o["status"].as_str().unwrap_or("") {
                        "new" => "주문 접수",
                        "preparing" => "준비 중",
                        "ready" => "음식 준비 완료",
                        "served" => "서빙 완료",
                        "completed" => "완료",
                        "cancelled" => "취소",
                        "refunded" => "환불 완료",
                        _ => "확인 중",
                    };
                    lines.push(format!(
                        "주문 #{} · {status} · {}\n{}",
                        o["id"],
                        if o["payment_status"] == "paid" {
                            "결제 완료"
                        } else {
                            "미결제"
                        },
                        ko_lines(&o["items"])
                    ));
                }
                if state["cart"].as_array().is_some_and(|a| !a.is_empty()) {
                    lines.push(format!(
                        "장바구니 · 아직 주문 전\n{}",
                        ko_lines(&state["cart"])
                    ));
                }
                if lines.is_empty() {
                    "아직 주문한 메뉴가 없어요. 먹고 싶은 걸 알려주세요!".into()
                } else {
                    lines.join("\n\n")
                }
            }
            "choose_menu" | "cancel_menu" => {
                let state = self.snapshot(ses).await?;
                if state["cart"].as_array().unwrap().is_empty() {
                    "장바구니가 비었어요. 먹고 싶은 걸 알려주세요!".into()
                } else {
                    format!(
                        "장바구니를 바꿨어요.\n{}\n주문하려면 ‘주문 확인’이라고 말해주세요.",
                        ko_lines(&state["cart"])
                    )
                }
            }
            _ => {
                if r.get("service_calls").is_some() {
                    "직원을 불렀어요. 조금만 기다려주세요!".into()
                } else if let Some(id) = r["order_id"].as_i64() {
                    let state = self.snapshot(ses).await?;
                    let order = state["orders"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|o| o["id"] == id);
                    if let Some(o) = order {
                        format!(
                            "주문 #{id} 접수했어요!\n{}\n총 HK${:.2} · {}",
                            ko_lines(&o["items"]),
                            o["total"].as_i64().unwrap_or(0) as f64 / 100.,
                            if o["payment_status"] == "paid" {
                                "지갑 결제 완료"
                            } else {
                                "미결제"
                            }
                        )
                    } else {
                        format!("주문 #{id}를 확인했어요.")
                    }
                } else if previous.contains(" / ") {
                    previous
                        .split(" / ")
                        .find(|s| s.chars().any(|c| ('가'..='힣').contains(&c)))
                        .unwrap_or("요청을 확인했어요.")
                        .to_string()
                } else if previous.chars().any(|c| ('가'..='힣').contains(&c)) {
                    previous
                } else if previous.contains("cleared") {
                    "장바구니를 비웠어요.".into()
                } else {
                    "먹고 싶은 음식이나 원하는 도움을 알려주세요!".into()
                }
            }
        };
        r["reply"] = json!(reply);
        Ok(())
    }
}
