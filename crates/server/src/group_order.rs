use crate::*;

/// Recognize only the simple quantity-only pattern. Extra preferences must not be ignored.
pub fn parse_group(text: &str) -> Option<Value> {
    fn number(s: &str) -> Option<(i64, &str)> {
        let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        if end == 0 {
            return None;
        }
        Some((s[..end].parse().ok()?, &s[end..]))
    }
    fn prefix<'a>(s: &'a str, choices: &[&str]) -> Option<&'a str> {
        choices.iter().find_map(|p| s.strip_prefix(p))
    }
    let compact: String = text
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, ',' | '，' | '.' | '!' | '。'))
        .collect();
    let (people, rest) = number(&compact)?;
    let rest = prefix(rest, &["명", "인"])?;
    let rest = prefix(rest, &["메뉴", "음식", "식사"])?;
    let (food, rest) = number(rest)?;
    let rest = prefix(rest, &["개"])?;
    let rest = prefix(rest, &["음료수", "음료"])?;
    let (drinks, rest) = number(rest)?;
    let rest = prefix(rest, &["개", "잔"])?;
    if !matches!(
        rest,
        "" | "주세요" | "부탁해요" | "추천해줘" | "추천해주세요"
    ) {
        return None;
    }
    Some(json!({"people":people,"food":food,"drinks":drinks,"language":"ko"}))
}
impl App {
    pub async fn recommend_group(&self, ses: &Session, p: &Value) -> Result<Value> {
        let people = p["people"]
            .as_i64()
            .filter(|v| (1..=20).contains(v))
            .ok_or("인원은 1–20명으로 입력해주세요. / Choose 1–20 people.")?;
        let food = p["food"]
            .as_i64()
            .filter(|v| (1..=20).contains(v))
            .ok_or("음식은 1–20개로 입력해주세요. / Choose 1–20 food items.")?;
        let drinks = p["drinks"]
            .as_i64()
            .filter(|v| (1..=20).contains(v))
            .ok_or("음료는 1–20개로 입력해주세요. / Choose 1–20 drinks.")?;
        let korean = p["language"] == "ko";
        let mut tx = self.db.begin_with("BEGIN IMMEDIATE").await.map_err(err)?;
        let raw: String = sqlx::query_scalar("SELECT cart FROM sessions WHERE token=?")
            .bind(&ses.token)
            .fetch_one(&mut *tx)
            .await
            .map_err(err)?;
        if !serde_json::from_str::<Vec<Value>>(&raw)
            .map_err(err)?
            .is_empty()
        {
            return Ok(
                json!({"reply":if korean{"장바구니에 이미 메뉴가 있어요. 기존 메뉴를 확인하거나 비운 뒤 단체 추천을 다시 요청해주세요."}else{"Your basket already has items. Review or clear it before requesting a group recommendation."}}),
            );
        }
        // Rank from this shop's own order quantities, calculated only on a request.
        // Separate simulation and live history; exclude unknown legacy mode and closed failures.
        let timestamp = now();
        let menu = sqlx::query(r#"
            WITH demand AS (
                SELECT json_extract(j.value,'$.id') AS item_id,
                  SUM(CASE WHEN o.created>=? THEN CAST(json_extract(j.value,'$.quantity') AS INTEGER) ELSE 0 END) AS recent,
                  SUM(CASE WHEN o.created<? THEN CAST(json_extract(j.value,'$.quantity') AS INTEGER) ELSE 0 END) AS previous,
                  SUM(CASE WHEN o.created>=? THEN CAST(json_extract(j.value,'$.quantity') AS INTEGER) ELSE 0 END) AS hour
                FROM orders o, json_each(o.items) j
                WHERE o.created>=? AND o.created<=?
                  AND o.status IN ('new','preparing','ready','served','completed')
                  AND o.simulation=(SELECT simulation FROM shop WHERE id=1)
                GROUP BY item_id
            ), ranked AS (
                SELECT m.id,m.name,m.price,m.category,m.allergens,m.rowid AS position,
                  COALESCE(d.recent,0) AS recent, COALESCE(d.previous,0) AS previous,
                  COALESCE(d.hour,0) AS hour
                FROM menu m LEFT JOIN demand d ON d.item_id=m.id
                WHERE m.available=1 AND m.archived=0
            )
            SELECT *, recent+3*hour+2*MAX(recent-previous,0) AS score
            FROM ranked ORDER BY score DESC,position ASC
        "#).bind(timestamp-7*86400).bind(timestamp-7*86400).bind(timestamp-3600)
          .bind(timestamp-14*86400).bind(timestamp).fetch_all(&mut *tx).await.map_err(err)?;
        let has_history = menu
            .iter()
            .any(|r| r.get::<i64, _>("recent") > 0 || r.get::<i64, _>("previous") > 0);
        let category = |r: &&sqlx::sqlite::SqliteRow| r.get::<String, _>("category").to_lowercase();
        let is_drink = |r: &&sqlx::sqlite::SqliteRow| {
            matches!(
                category(r).as_str(),
                "drinks"
                    | "drink"
                    | "beverages"
                    | "beverage"
                    | "음료"
                    | "음료수"
                    | "飲品"
                    | "饮品"
                    | "ドリンク"
            )
        };
        let drinks_menu: Vec<_> = menu.iter().filter(is_drink).collect();
        let mut food_menu: Vec<_> = menu
            .iter()
            .filter(|r| {
                matches!(
                    category(r).as_str(),
                    "kitchen" | "food" | "meals" | "main" | "mains" | "식사" | "음식" | "主食"
                )
            })
            .collect();
        if food_menu.is_empty() {
            food_menu = menu
                .iter()
                .filter(|r| {
                    matches!(
                        category(r).as_str(),
                        "bakery" | "dessert" | "desserts" | "베이커리" | "디저트"
                    )
                })
                .collect();
        }
        if food_menu.is_empty() || drinks_menu.is_empty() {
            return Ok(
                json!({"reply":if korean{"현재 판매 가능한 음식과 음료 구성을 찾지 못했어요. 메뉴에서 직접 선택해주세요."}else{"There are not enough available food/drink categories. Please choose from the menu."}}),
            );
        }
        let mut cart = Vec::new();
        let mut descriptions = Vec::new();
        let mut total = 0i64;
        let mut selection_trends = Vec::new();
        for (items, count) in [(&food_menu, food), (&drinks_menu, drinks)] {
            for (index, item) in items.iter().take(count as usize).enumerate() {
                let quantity = count / items.len() as i64
                    + if (index as i64) < count % items.len() as i64 {
                        1
                    } else {
                        0
                    };
                let id: String = item.get("id");
                let name: String = item.get("name");
                let price: i64 = item.get("price");
                let allergens: String = item.get("allergens");
                let recent: i64 = item.get("recent");
                let previous: i64 = item.get("previous");
                let hour: i64 = item.get("hour");
                selection_trends.push(json!({"id":id,"last_7_days":recent,"previous_7_days":previous,"last_hour":hour,"score":item.get::<i64,_>("score")}));
                total += price * quantity;
                descriptions.push(format!(
                    "{quantity} × {name} · HK${:.2}{}",
                    price as f64 * quantity as f64 / 100.,
                    if allergens.is_empty() {
                        String::new()
                    } else {
                        format!(" ({allergens})")
                    }
                ));
                if has_history {
                    descriptions.push(if korean {format!("  최근 7일 {recent}개 · 최근 1시간 {hour}개 · 이전 7일 대비 {:+}개",recent-previous)}else{format!("  Last 7 days: {recent} · Last hour: {hour} · Change vs previous 7 days: {:+}",recent-previous)});
                }
                cart.push(json!({"id":id,"name":name,"price":price,"quantity":quantity,"note":""}));
            }
        }
        sqlx::query("UPDATE sessions SET cart=?,pending=NULL WHERE token=?")
            .bind(json!(cart).to_string())
            .bind(&ses.token)
            .execute(&mut *tx)
            .await
            .map_err(err)?;
        tx.commit().await.map_err(err)?;
        let basis = if has_history {
            if korean {
                "매장 내 최근 주문량과 증가 추세를 반영했습니다. 외부 유행 데이터는 사용하지 않았습니다."
            } else {
                "Based on this cafe's recent order quantities and growth, not external trends."
            }
        } else if korean {
            "아직 주문 이력이 부족해 판매 가능한 메뉴로 기본 구성했습니다."
        } else {
            "Not enough order history yet; using available menu items as a default selection."
        };
        let reply = if korean {
            format!("{people}명이 드실 음식 {food}개와 음료 {drinks}개를 추천해서 담았어요. 아직 주문하지 않았습니다.\n{}\n합계 HK${:.2}\n알레르기와 취향에 맞는지 확인해주세요. 변경할 수 있으며, ‘주문 확인’이라고 보내면 주문합니다.",descriptions.join("\n"),total as f64/100.)
        } else {
            format!("Suggested {food} food items and {drinks} drinks for {people} people. Added to your basket, not ordered yet.\n{}\nTotal HK${:.2}\nReview preferences/allergens, change any selections, then say ‘confirm order’.",descriptions.join("\n"),total as f64/100.)
        };
        Ok(
            json!({"reply":format!("{basis}\n{reply}"),"function":"recommend_group","people":people,"food":food,"drinks":drinks,"total":total,"recommendation_basis":if has_history{"shop_order_trends"}else{"available_menu_fallback"},"trends":selection_trends}),
        )
    }
}
