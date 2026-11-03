mod i18n;
mod management;
use cafe_shared::{identity::Identity, Rpc};
use i18n::{food, t, LANGUAGES};
use leptos::prelude::*;
use management::*;
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
#[wasm_bindgen(raw_module = "/media.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn listen_once() -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn speech_text(language: &str) -> Result<JsValue, JsValue>;
    fn scroll_chat();
    fn new_request_id() -> String;
    fn stop_listening();
    fn subscribe_updates(token: &str, callback: &js_sys::Function);
    fn unsubscribe_updates();
    #[wasm_bindgen(catch)]
    async fn camera_start() -> Result<JsValue, JsValue>;
    fn camera_stop();
    #[wasm_bindgen(catch)]
    async fn read_menu_photo() -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch)]
    async fn read_floor_photo() -> Result<JsValue, JsValue>;
    fn print_page();
    fn camera_frame() -> String;
}
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.session_storage().ok()?
}
fn pref(key: &str, default: &str) -> String {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(key).ok().flatten())
        .unwrap_or(default.into())
}
fn save(key: &str, value: &str) {
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = s.set_item(key, value);
    }
}
fn money(v: i64) -> String {
    if v % 100 == 0 {
        format!("HK${}", v / 100)
    } else {
        format!("HK${:.2}", v as f64 / 100.)
    }
}
fn val(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").into()
}
fn arr(v: &Value, k: &str) -> Vec<Value> {
    v[k].as_array().cloned().unwrap_or_default()
}
fn error_js(e: JsValue) -> String {
    e.as_string().unwrap_or_else(|| {
        js_sys::JSON::stringify(&e)
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or("Device unavailable".into())
    })
}
async fn rpc(token: &str, method: &str, payload: Value) -> Result<Value, String> {
    let bytes = cafe_shared::encode(&Rpc {
        token: token.into(),
        method: method.into(),
        payload,
    })?;
    let response = gloo_net::http::Request::post("/api/rpc")
        .header("Content-Type", "application/x-capnp")
        .body(bytes)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|_| "Cannot reach the café. Please try again.".to_string())?;
    if !response.ok() {
        return Err(format!("Café returned {}", response.status()));
    }
    let bytes = response.binary().await.map_err(|e| e.to_string())?;
    let r = cafe_shared::decode(&bytes)?;
    if r.payload["ok"] == true {
        Ok(r.payload["data"].clone())
    } else {
        Err(r.payload["error"]
            .as_str()
            .unwrap_or("Something went wrong")
            .into())
    }
}

fn table_code(state: &Value, id: i64) -> String {
    arr(state, "tables")
        .iter()
        .find(|t| t["id"].as_i64() == Some(id))
        .map(|t| val(t, "code"))
        .unwrap_or(id.to_string())
}
fn pending_summary(p: &Value) -> String {
    management::describe_action(p)
}

#[derive(Clone, Copy)]
struct Ui {
    token: RwSignal<String>,
    state: RwSignal<Value>,
    state_revision: RwSignal<u64>,
    lang: RwSignal<String>,
    busy: RwSignal<bool>,
    notice: RwSignal<String>,
    input: RwSignal<String>,
    listening: RwSignal<bool>,
    handsfree: RwSignal<bool>,
    camera: RwSignal<bool>,
    tab: RwSignal<String>,
    dark: RwSignal<bool>,
    vertical: RwSignal<bool>,
    font: RwSignal<i32>,
}
impl Ui {
    fn handle_result(self, v: &Value) {
        if self.state.get_untracked()["role"] == "guest" && self.tab.get_untracked() == "chat" {
            return;
        }
        match v["ui"].as_str() {
            Some("drafts") => self.tab.set("drafts".into()),
            Some("menu") => self.tab.set("menu".into()),
            Some("orders") => self.tab.set("orders".into()),
            Some("faucet") => self.tab.set("faucet".into()),
            Some("print_qr") => self.tab.set("print".into()),
            _ => {}
        }
    }
    fn tr(self, key: &str) -> String {
        t(&self.lang.get(), key)
    }
    async fn refresh(self) -> Result<(), String> {
        let v = rpc(&self.token.get_untracked(), "state", json!({})).await?;
        if v["role"] != "guest"
            && v["shop"]["agent_running"] != true
            && self.listening.get_untracked()
        {
            self.listening.set(false);
            stop_listening();
        }
        self.state.set(v);
        self.state_revision.update(|r| *r += 1);
        Ok(())
    }
    fn run(self, method: &'static str, p: Value) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        spawn_local(async move {
            match rpc(&self.token.get_untracked(), method, p).await {
                Ok(v) => {
                    self.notice.set(val(&v, "reply"));
                    self.handle_result(&v);
                    if let Err(e) = self.refresh().await {
                        self.notice.set(e);
                    }
                }
                Err(e) => self.notice.set(e),
            }
            self.busy.set(false);
        });
    }
    async fn send(self, text: String) -> Result<(), String> {
        if text.trim().is_empty() {
            return Ok(());
        }
        let command = text
            .trim()
            .trim_end_matches(['.', '!', '?', '。', '！', '？', ',', '，'])
            .trim()
            .to_lowercase();
        let guest_cafe_command = self.state.get_untracked()["role"] == "guest"
            && matches!(
                command.as_str(),
                "show menu"
                    | "menu"
                    | "餐牌"
                    | "菜单"
                    | "メニュー"
                    | "메뉴"
                    | "menú"
                    | "show orders"
                    | "orders"
                    | "訂單"
                    | "订单"
                    | "注文"
                    | "주문"
                    | "objednávky"
                    | "pedidos"
            );
        let handled = match if guest_cafe_command {
            ""
        } else {
            command.as_str()
        } {
            "run" | "ai 시작" | "가동" => {
                let v = rpc(
                    &self.token.get_untracked(),
                    "agent_run",
                    json!({"running":true}),
                )
                .await?;
                self.notice.set(val(&v, "reply"));
                self.refresh().await?;
                return Ok(());
            }
            "stop" | "ai 중지" => {
                let v = rpc(
                    &self.token.get_untracked(),
                    "agent_run",
                    json!({"running":false}),
                )
                .await?;
                self.notice.set(val(&v, "reply"));
                self.refresh().await?;
                return Ok(());
            }
            "메뉴 사진 입력" | "photo menu" => {
                self.tab.set("photo".into());
                true
            }
            "메뉴 생성" | "menu templates" => {
                self.tab.set("templates".into());
                true
            }
            "메뉴 초안" | "drafts" => {
                self.tab.set("drafts".into());
                true
            }
            "직원 관리" | "매장 관리" | "management" => {
                self.tab.set("management".into());
                true
            }
            "qr 인쇄" | "print qr" => {
                let v = rpc(&self.token.get_untracked(), "print_qr", json!({})).await?;
                self.handle_result(&v);
                return Ok(());
            }
            "show menu" | "menu" | "餐牌" | "菜单" | "メニュー" | "메뉴" | "menú" => {
                self.tab.set("menu".into());
                true
            }
            "show orders" | "orders" | "訂單" | "订单" | "注文" | "주문" | "objednávky"
            | "pedidos" => {
                self.tab.set("orders".into());
                true
            }
            "dashboard" | "show dashboard" | "대시보드" | "概覽" | "概览" | "přehled"
            | "resumen" => {
                if self.state.get_untracked()["role"] != "guest" {
                    self.tab.set("dashboard".into());
                }
                true
            }
            "dark mode" | "深色模式" | "다크 모드" | "ダークモード" | "modo oscuro"
            | "tmavý režim" => {
                self.dark.set(true);
                true
            }
            "light mode" | "淺色模式" | "浅色模式" | "라이트 모드" | "ライトモード"
            | "modo claro" | "světlý režim" => {
                self.dark.set(false);
                true
            }
            "vertical mode" | "vertical layout" => {
                self.vertical.set(true);
                true
            }
            "horizontal mode" | "horizontal layout" => {
                self.vertical.set(false);
                true
            }
            "bigger text" | "increase font" => {
                self.font.update(|n| *n = (*n + 1).min(24));
                true
            }
            "smaller text" | "decrease font" => {
                self.font.update(|n| *n = (*n - 1).max(14));
                true
            }
            "hands-free on" | "hands free on" => {
                self.handsfree.set(true);
                if !self.listening.get_untracked() {
                    self.voice();
                }
                true
            }
            "hands-free off" | "hands free off" => {
                self.handsfree.set(false);
                true
            }
            "stop listening" => {
                self.listening.set(false);
                stop_listening();
                true
            }
            "camera off" | "stop camera" => {
                camera_stop();
                self.camera.set(false);
                true
            }
            "camera on" | "start camera" => {
                self.camera.set(true);
                if let Err(e) = camera_start().await {
                    self.camera.set(false);
                    return Err(error_js(e));
                }
                true
            }
            _ => false,
        };
        if handled {
            self.notice.set("Done.".into());
            return Ok(());
        }
        for (code, name) in LANGUAGES {
            if command == format!("language {}", name.to_lowercase())
                || command == format!("language {code}")
            {
                self.lang.set(code.into());
                self.notice.set("Language updated.".into());
                return Ok(());
            }
        }
        let image = if self.camera.get_untracked() {
            camera_frame()
        } else {
            String::new()
        };
        let token = self.token.get_untracked();
        let language = self.lang.get_untracked();
        let pending = storage()
            .and_then(|s| s.get_item("cafe-pending-chat").ok().flatten())
            .and_then(|s| serde_json::from_str::<Value>(&s).ok());
        let request_id = pending
            .filter(|p| p["token"] == token && p["text"] == text && p["language"] == language)
            .and_then(|p| p["id"].as_str().map(str::to_owned))
            .unwrap_or_else(new_request_id);
        if let Some(s) = storage() {
            let _ = s.set_item(
                "cafe-pending-chat",
                &json!({"id":request_id,"token":token,"text":text,"language":language}).to_string(),
            );
        }
        let v = rpc(
            &token,
            "chat",
            json!({"text":text,"language":language,"image":image,"request_id":request_id}),
        )
        .await?;
        self.notice.set(String::new());
        self.handle_result(&v);
        self.refresh().await?;
        if let Some(s) = storage() {
            let _ = s.remove_item("cafe-pending-chat");
        }
        Ok(())
    }
    fn chat(self) {
        if self.busy.get_untracked() {
            return;
        }
        let text = self.input.get_untracked();
        if text.trim().is_empty() {
            return;
        }
        self.input.set(String::new());
        self.busy.set(true);
        spawn_local(async move {
            if let Err(e) = self.send(text.clone()).await {
                self.notice.set(e);
                self.input.set(text);
                let _ = self.refresh().await;
            }
            self.busy.set(false);
        });
    }
    fn voice(self) {
        if self.listening.get_untracked() {
            self.listening.set(false);
            stop_listening();
            return;
        }
        if self.state.get_untracked()["role"] == "guest" {
            self.listening.set(true);
            self.notice
                .set(label(self, "말씀해 주세요…", "Listening…").into());
            spawn_local(async move {
                let result = speech_text(&self.lang.get_untracked()).await;
                self.listening.set(false);
                match result {
                    Ok(text) => {
                        let text = text.as_string().unwrap_or_default();
                        if !text.is_empty() {
                            self.input.set(text);
                            self.chat();
                        }
                    }
                    Err(error) => self.notice.set(error_js(error)),
                }
            });
            return;
        }
        if self.state.get_untracked()["shop"]["agent_running"] != true {
            self.notice
                .set("STAFF / OWNER: press Run to activate AI.".into());
            return;
        }
        self.listening.set(true);
        spawn_local(async move {
            loop {
                match listen_once().await {
                    Ok(audio) => {
                        if !self.listening.get_untracked() {
                            break;
                        }
                        let audio = audio.as_string().unwrap_or_default();
                        if !audio.is_empty() {
                            self.busy.set(true);
                            self.notice.set("Transcribing…".into());
                            let result = rpc(
                                &self.token.get_untracked(),
                                "transcribe",
                                json!({"audio":audio,"language":self.lang.get_untracked()}),
                            )
                            .await;
                            match result {
                                Ok(v) => {
                                    let text = val(&v, "text");
                                    self.notice.set(text.clone());
                                    if let Err(e) = self.send(text).await {
                                        self.notice.set(e);
                                        self.listening.set(false);
                                    }
                                }
                                Err(e) => {
                                    self.notice.set(e);
                                    self.listening.set(false);
                                }
                            }
                            self.busy.set(false);
                        }
                    }
                    Err(e) => {
                        self.notice.set(error_js(e));
                        break;
                    }
                }
                if !self.listening.get_untracked() || !self.handsfree.get_untracked() {
                    break;
                }
            }
            self.listening.set(false);
            stop_listening();
        });
    }
}
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
#[component]
fn App() -> impl IntoView {
    let ui = Ui {
        token: RwSignal::new(String::new()),
        state: RwSignal::new(json!({})),
        state_revision: RwSignal::new(0),
        lang: RwSignal::new(pref("cafe-language", "en")),
        busy: RwSignal::new(false),
        notice: RwSignal::new(String::new()),
        input: RwSignal::new(String::new()),
        listening: RwSignal::new(false),
        handsfree: RwSignal::new(false),
        camera: RwSignal::new(false),
        tab: RwSignal::new("chat".into()),
        dark: RwSignal::new(pref("cafe-theme", "light") == "dark"),
        vertical: RwSignal::new(pref("cafe-layout", "horizontal") == "vertical"),
        font: RwSignal::new(
            pref("cafe-font", "16")
                .parse::<i32>()
                .unwrap_or(16)
                .clamp(14, 24),
        ),
    };
    let dark = ui.dark;
    let vertical = ui.vertical;
    let font = ui.font;
    let settings = RwSignal::new(false);
    let more = RwSignal::new(false);
    let login = RwSignal::new(false);
    let category = RwSignal::new("all".to_string());
    Effect::new(move |_| {
        save("cafe-language", &ui.lang.get());
        save("cafe-theme", if dark.get() { "dark" } else { "light" });
        save(
            "cafe-layout",
            if vertical.get() {
                "vertical"
            } else {
                "horizontal"
            },
        );
        save("cafe-font", &font.get().to_string());
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            if let Some(el) = doc.document_element() {
                let _ = el.set_attribute("lang", &ui.lang.get());
            }
        }
    });
    spawn_local(async move {
        let query = web_sys::window()
            .unwrap()
            .location()
            .search()
            .unwrap_or_default();
        let table = query
            .trim_start_matches('?')
            .split('&')
            .find_map(|s| s.strip_prefix("table="))
            .map(str::to_string)
            .unwrap_or("1".into());
        let guid = query
            .trim_start_matches('?')
            .split('&')
            .find_map(|s| s.strip_prefix("shop_guid="));
        let saved = storage().and_then(|s| s.get_item("cafe-token").ok().flatten());
        if let Some(token) = saved {
            ui.token.set(token);
            if ui.refresh().await.is_ok()
                && guid.is_none_or(|g| ui.state.get_untracked()["shop"]["guid"].as_str() == Some(g))
                && (ui.state.get_untracked()["role"] != "guest"
                    || !query.contains("table=")
                    || arr(&ui.state.get_untracked(), "tables").iter().any(|t| {
                        t["id"] == ui.state.get_untracked()["table"] && t["code"] == table
                    }))
            {
                if ui.state.get_untracked()["role"] != "guest" {
                    ui.tab.set("orders".into());
                }
                return;
            }
        }
        let guid = query
            .trim_start_matches('?')
            .split('&')
            .find_map(|s| s.strip_prefix("shop_guid="));
        match rpc(
            "",
            "guest",
            json!({"table":if query.contains("table="){Some(table)}else{None},"shop_guid":guid}),
        )
        .await
        {
            Ok(v) => {
                ui.token.set(val(&v, "token"));
                if let Some(s) = storage() {
                    let _ = s.set_item("cafe-token", &ui.token.get_untracked());
                }
                if let Err(e) = ui.refresh().await {
                    ui.notice.set(e);
                }
            }
            Err(e) => ui.notice.set(e),
        }
    });
    // No order polling: fetch a fresh snapshot only after a server change event.
    let updates = RwSignal::new(0u64);
    let handled = RwSignal::new(0u64);
    let refreshing = RwSignal::new(false);
    let callback = StoredValue::new_local(Closure::<dyn Fn()>::new(move || {
        updates.update(|n| *n += 1)
    }));
    Effect::new(move |_| {
        let token = ui.token.get();
        callback.with_value(|cb| subscribe_updates(&token, cb.as_ref().unchecked_ref()));
    });
    on_cleanup(unsubscribe_updates);
    Effect::new(move |_| {
        let requested = updates.get();
        let token = ui.token.get();
        let busy = ui.busy.get();
        if requested > handled.get() && !token.is_empty() && !busy && !refreshing.get() {
            refreshing.set(true);
            let revision = ui.state_revision.get_untracked();
            spawn_local(async move {
                let result = rpc(&token, "state", json!({})).await;
                if token == ui.token.get_untracked()
                    && !ui.busy.get_untracked()
                    && revision == ui.state_revision.get_untracked()
                {
                    match result {
                        Ok(v) => {
                            if v["role"] != "guest"
                                && v["shop"]["agent_running"] != true
                                && ui.listening.get_untracked()
                            {
                                ui.listening.set(false);
                                stop_listening();
                            }
                            ui.state.set(v);
                            ui.state_revision.update(|r| *r += 1);
                        }
                        Err(e) => ui.notice.set(e),
                    }
                    handled.set(requested);
                }
                refreshing.set(false);
            });
        }
    });
    Effect::new(move |_| {
        let state = ui.state.get();
        let notice = ui.notice.get();
        if !arr(&state, "messages").is_empty() || !notice.is_empty() {
            scroll_chat();
        }
    });
    view! {
    <div data-state-version=move||ui.state_revision.get().to_string() class=move||format!("app{}{}",if dark.get(){" dark"}else{""},if ui.state.get()["role"].as_str().unwrap_or("guest")=="guest"&&ui.tab.get()=="chat"{" guest-chat-app"}else{""}) style=move||format!("font-size:{}px",font.get())>
    <header class="topbar"><a class="brand" href="/"><span class="brand-mark">"茶"</span><span>"CAUSEWAYBAY"<small>"CAFE · 銅鑼灣"</small></span></a>
    <div class="top-actions">
    <div class="quick-preferences" role="group" aria-label="Quick preferences">
        <select class="top-language" aria-label="Language" title="Language" prop:value=move||ui.lang.get() on:change=move|e|ui.lang.set(event_target_value(&e))>
            {LANGUAGES.into_iter().map(|(code,name)|view!{<option value=code>{name}</option>}).collect_view()}
        </select>
        <button class="top-theme" aria-label="Toggle theme" title="Light / dark theme" aria-pressed=move||dark.get().to_string() on:click=move |_|dark.update(|v|*v=!*v)>{move||if dark.get(){"☾"}else{"☀"}}</button>
        <button class="layout-toggle" aria-label=move||if vertical.get(){"Switch to horizontal layout"}else{"Switch to vertical layout"} title=move||label(ui,"가로·세로 화면 전환","Switch horizontal / vertical layout") aria-pressed=move||vertical.get().to_string() on:click=move |_|vertical.update(|v|*v=!*v)><span aria-hidden="true">{move||if vertical.get(){"↕"}else{"↔"}}</span><span class="layout-label">{move||if vertical.get(){label(ui,"세로","Vertical")}else{label(ui,"가로","Horizontal")}}</span></button>
        <div class="font-controls" role="group" aria-label="Font size">
            <button aria-label="Decrease font" title="Smaller text" disabled=move||{font.get()<=14} on:click=move |_|font.update(|v|*v=(*v-1).max(14))>"A−"</button>
            <output aria-label="Current font size">{move||font.get()}</output>
            <button aria-label="Increase font" title="Larger text" disabled=move||{font.get()>=24} on:click=move |_|font.update(|v|*v=(*v+1).min(24))>"A+"</button>
        </div>
    </div><Show when=move||ui.state.get()["role"].as_str().unwrap_or("guest")!="guest"><button class="wallet-login" on:click=move |_|login.set(true)>{move||if ui.state.get()["wallet"].is_null(){label(ui,"게스트 로그인","Guest login")}else{label(ui,"내 지갑","My wallet")}}</button></Show><span class="table-tag">{move||format!("{} · Table {}",ui.tr(ui.state.get()["role"].as_str().unwrap_or("guest")),table_code(&ui.state.get(),ui.state.get()["table"].as_i64().unwrap_or(1)))}</span>
    <Show when=move||ui.camera.get()><button class="camera-indicator" on:click=move |_|{camera_stop();ui.camera.set(false);}>"◉ Camera on · Stop"</button></Show><button class="icon-button more-toggle" aria-label="More options" aria-expanded=move||more.get().to_string() aria-controls="topbar-menu" on:click=move |_|more.update(|v|*v=!*v)>"☰"</button></div><Show when=move||more.get()><button class="navigation-dismiss" aria-label="Close menu" on:click=move |_|more.set(false)></button></Show><div id="topbar-menu" class="cafe-options" hidden=move||!more.get() on:keydown=move|e|{if e.key()=="Escape"{more.set(false);}} on:click=move |_|more.set(false)><nav class="tabs"><Show when=move||true><button data-testid="guest-chat-tab" class:active=move||ui.tab.get()=="chat" on:click=move |_|ui.tab.set("chat".into())>"☏ "{move||ui.tr("chat")}</button></Show><Show when=move||ui.state.get()["faucet"]["enabled"]==true><button class:active=move||ui.tab.get()=="faucet" on:click=move |_|ui.tab.set("faucet".into())>"🎁 Faucet"</button></Show><button class:active=move||ui.tab.get()=="menu" on:click=move |_|ui.tab.set("menu".into())>"⌑ "{move||ui.tr("menu")}</button><button class:active=move||ui.tab.get()=="orders" on:click=move |_|ui.tab.set("orders".into())>"▤ "{move||ui.tr("orders")}<span class="count">{move||arr(&ui.state.get(),"orders").len()}</span></button><Show when=move||ui.state.get()["role"]!="guest"&&!ui.state.get()["role"].is_null()><button class:active=move||ui.tab.get()=="dashboard" on:click=move |_|ui.tab.set("dashboard".into())>"◫ "{move||ui.tr("dashboard")}</button></Show><Show when=move||ui.state.get()["role"]=="staff"||ui.state.get()["role"]=="owner">
    <button class:active=move||ui.tab.get()=="templates" on:click=move |_|ui.tab.set("templates".into())>{move||label(ui,"메뉴 생성 템플릿","Menu templates")}</button>
    <button class:active=move||ui.tab.get()=="photo" on:click=move |_|ui.tab.set("photo".into())>{move||label(ui,"메뉴 사진 입력","Menu photo input")}</button>
    <button class:active=move||ui.tab.get()=="drafts" on:click=move |_|ui.tab.set("drafts".into())>{move||label(ui,"메뉴 초안","Menu drafts")}</button>
    </Show><Show when=move||ui.state.get()["role"]=="owner"><button class:active=move||ui.tab.get()=="management" on:click=move |_|ui.tab.set("management".into())>{move||label(ui,"매장 관리","Shop management")}</button></Show></nav><Show when=move||ui.state.get()["role"].as_str().unwrap_or("guest")=="guest"><button class="wallet-login" on:click=move |_|login.set(true)>{move||if ui.state.get()["wallet"].is_null(){label(ui,"게스트 로그인","Guest login")}else{label(ui,"내 지갑","My wallet")}}</button></Show><button on:click=move |_|settings.set(true)>{move||ui.tr("settings")}</button></div></header>
    <Show when=move||ui.state.get()["simulation"]==true&&ui.state.get()["role"]!="guest"><div class="simulation">{move||label(ui,"테스트 매장 · 편하게 주문해 보세요","Practice cafe · Try an order")}</div></Show>
    <main class="shell" class:horizontal-layout=move||!vertical.get() class:guest-chat-screen=move||ui.state.get()["role"]=="guest"&&ui.tab.get()=="chat" class:guest-shell=move||ui.state.get()["role"].as_str().unwrap_or("guest")=="guest"><Show when=move||matches!(ui.state.get()["role"].as_str(),Some("owner"|"staff"))><AgentControl ui=ui/></Show>
    <div class=move||format!("workspace{}{}{}",if vertical.get(){" vertical"}else{""},if ui.state.get()["role"].as_str().unwrap_or("guest")=="guest"{" guest-workspace"}else{""},if ui.tab.get()=="chat"{" chat-home"}else{""})>
    <section class="content-panel" hidden=move||ui.tab.get()=="chat"><Show when=move||ui.tab.get()=="faucet"&&ui.state.get()["faucet"]["enabled"]==true><Faucet ui=ui/></Show>
    <Show when=move||ui.tab.get()=="menu"><div class="section-title"><div><span class="eyebrow">"FRESH FROM OUR LITTLE KITCHEN"</span><h2>{move||ui.tr("menu")}</h2></div><span class="muted">"HKD"</span></div>
    <div class="filters">{move||{let mut cats=vec!["all".to_string()];for m in arr(&ui.state.get(),"menu"){let c=val(&m,"category");if !cats.contains(&c){cats.push(c);}}cats.into_iter().map(|c|{let c2=c.clone();let c3=c.clone();view!{<button class:selected=move||category.get()==c2 on:click=move |_|category.set(c3.clone())>{ui.tr(&c)}</button>}}).collect_view()}}</div>
    <div class="menu-grid">{move||arr(&ui.state.get(),"menu").into_iter().filter(|m|category.get()=="all"||m["category"]==category.get()).map(|m|{let id=val(&m,"id");let id2=id.clone();let name=food(&ui.lang.get(),&id,&val(&m,"name"));let available=m["available"]==true;let image_path=val(&m,"image_path");let has_image=!image_path.is_empty();let emoji=match id.as_str(){"milk-tea"=>"☕","yuenyeung"=>"🧋","lemon-tea"=>"🍋","pineapple-bun"=>"🥐","egg-tart"=>"🥧","french-toast"=>"🍞","macaroni"=>"🍲",_=>"🍚"};view!{<article class="menu-card"><div class=format!("food-art {}",val(&m,"category").to_lowercase())><Show when=move||has_image fallback=move||view!{<span>{emoji}</span>}><img class="menu-photo" src=image_path.clone() alt="AI-generated menu illustration"/></Show><span class="food-tag">{ui.tr(&val(&m,"category"))}</span></div><div class="food-info"><h3>{name}</h3><p>{val(&m,"description")}</p><small class="allergens">{if val(&m,"allergens").is_empty(){"Ask us about allergens".into()}else{format!("Contains: {}",val(&m,"allergens"))}}</small><div class="food-bottom"><strong>{money(m["price"].as_i64().unwrap_or(0))}</strong><button class="add" aria-label=format!("Add {}",val(&m,"name")) disabled=move||!available||ui.busy.get() on:click=move |_|ui.run("cart_add",json!({"id":id2,"quantity":1}))>{if available{format!("+ {}",ui.tr("add"))}else{"Sold out".into()}}</button></div></div></article>}}).collect_view()}</div></Show>
    <Show when=move||ui.tab.get()=="orders"><Orders ui=ui/></Show>
    <Show when=move||ui.tab.get()=="dashboard"&&ui.state.get()["role"]!="guest"><Dashboard ui=ui/></Show>
    <Show when=move||(ui.tab.get()=="templates"||ui.tab.get()=="photo")&&ui.state.get()["role"]!="guest"><MenuStudio ui=ui/></Show>
    <Show when=move||ui.tab.get()=="drafts"&&ui.state.get()["role"]!="guest"><Drafts ui=ui/></Show>
    <Show when=move||ui.tab.get()=="management"&&ui.state.get()["role"]=="owner"><Management ui=ui/></Show>
    <Show when=move||ui.tab.get()=="print"&&ui.state.get()["role"]=="owner"><QrPrint ui=ui/></Show>
    </section>
    <aside class="chat-panel" aria-label="Cafe chat"><Show when=move||ui.state.get()["role"]=="guest"><div class="panda-stage" class:panda-listening=move||ui.listening.get() class:panda-thinking=move||ui.busy.get()><div class="panda-character" role="img" aria-label="Panda, your café companion"></div><div class="panda-dialogue"><span class="panda-name">"PANDA · 銅鑼灣"</span><h2>{move||label(ui,"어서 와! 뭐 먹을까?","Hey! What sounds tasty?")}</h2><p role="status">{move||if ui.listening.get(){label(ui,"응, 듣고 있어!","I'm listening!")}else if ui.busy.get(){label(ui,"잠깐만, 내가 도와줄게!","One moment, I'm on it!")}else{label(ui,"먹고 싶은 걸 말해줘. 나머지는 내가 도와줄게.","Tell me what you'd like. I'll help with the rest.")}}</p></div><span class="stage-sticker" aria-hidden="true">"你好!"</span></div></Show><div class="chat-heading"><span class="avatar">"🐼"</span><div><h2>{move||ui.tr("chat")}</h2><small><span class="online-dot"></span>{move||if ui.listening.get(){"Listening · microphone on"}else{"Your friendly café companion"}}</small></div><span class="sparkle">"✧"</span></div>
    <div class="chat-messages" role="log" aria-live="polite"><div class="bubble assistant">{move||if ui.state.get()["role"].as_str().unwrap_or("guest")=="guest"{label(ui,"안녕하세요! 말하기를 누르거나 원하는 음식을 적어 주세요.","Welcome! Tap Speak or type what you would like.")}else{label(ui,"무엇을 도와드릴까요? 주문 번호와 할 일을 말해 주세요. 메뉴·직원·테이블 관리도 요청할 수 있어요.","How can I help? Tell me the order number and what to do. You can also ask to manage menus, staff, or tables.")}}</div><Show when=move||ui.state.get()["role"]=="guest"&&ui.tab.get()=="chat"><div class="bubble assistant chat-menu"><strong>{move||ui.tr("menu")}</strong>{move||arr(&ui.state.get(),"menu").into_iter().map(|m|{let id=val(&m,"id");let name=val(&m,"name");let available=m["available"]==true;view!{<div class="chat-menu-row"><span>{food(&ui.lang.get(),&id,&name)}<small>{money(m["price"].as_i64().unwrap_or(0))}</small></span><button aria-label=format!("Add {name}") disabled=move||ui.busy.get()||!available on:click=move |_|ui.run("cart_add",json!({"id":id,"quantity":1}))>{if available{"+"}else{"Sold out"}}</button></div>}}).collect_view()}</div></Show>{move||arr(&ui.state.get(),"messages").into_iter().filter(|m|!val(m,"content").starts_with("Simulation mode · Try")&&!val(m,"content").starts_with("What would you like to eat or drink? Tell me the item")).map(|m|view!{<div class=format!("bubble {}",val(&m,"role"))>{val(&m,"content")}</div>}).collect_view()}<Show when=move||ui.busy.get()><div class="thinking">{move||label(ui,"팬더가 생각하고 있어요…","Panda is on it…")}</div></Show><Show when=move||!ui.notice.get().is_empty()><div class="notice" role="status">{move||ui.notice.get()}</div></Show></div>
    <Show when=move||!ui.state.get()["pending"].is_null()><div class="confirmation"><strong>"Review this change"</strong><pre>{move||pending_summary(&ui.state.get()["pending"])}</pre><button disabled=move||ui.busy.get() on:click=move |_|ui.run("confirm",json!({}))>{move||ui.tr("confirm")}</button><button disabled=move||ui.busy.get() on:click=move |_|ui.run("cancel",json!({}))>{move||ui.tr("cancel")}</button></div></Show>
    <div class="composer"><form on:submit=move|e|{e.prevent_default();ui.chat();}><textarea on:keydown=move|e|{if e.key()=="Enter" && !e.shift_key() && !e.is_composing() && e.key_code()!=229 {e.prevent_default();ui.chat();}} rows="2" enterkeyhint="send" autocomplete="off" spellcheck="true" aria-label="Message Panda" placeholder=move||ui.tr("placeholder") prop:value=move||ui.input.get() on:input=move|e|ui.input.set(event_target_value(&e))/><button class="send" type="submit" aria-label="Send" disabled=move||ui.busy.get()||ui.token.get().is_empty()>"↑"</button></form><button class="voice" class:listening=move||ui.listening.get() disabled=move||ui.busy.get()&&!ui.listening.get() on:click=move |_|ui.voice()><svg class="mic-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><rect x="9" y="2" width="6" height="12" rx="3"/><path d="M5 10v2a7 7 0 0 0 14 0v-2M12 19v3M8 22h8"/></svg>{move||ui.tr(if ui.listening.get(){"stop"}else{"speak"})}</button><div class="voice-options"><label><input type="checkbox" prop:checked=move||ui.handsfree.get() on:change=move|e|ui.handsfree.set(event_target_checked(&e))/>"Hands-free"</label><small>"You speak. Panda writes."</small></div></div>
    <Show when=move||ui.state.get()["role"].as_str().unwrap_or("guest")!="guest"><Basket ui=ui/></Show>
    </aside><Show when=move||ui.state.get()["role"].as_str().unwrap_or("guest")=="guest"><aside class="guest-order-sheet" aria-label="Your order"><GuestOrderSheet ui=ui/><OrderVisual ui=ui/></aside></Show></div>
    <footer><span>"銅鑼灣 · CAUSEWAYBAY CAFE"</span><button on:click=move |_|login.set(true)>{move||ui.tr("login")}</button><span>"Take a moment. Stay a little."</span></footer></main>
    <Show when=move||settings.get()><div class="modal-backdrop"><section class="modal" role="dialog" aria-modal="true" aria-label="Settings"><button class="close" on:click=move |_|settings.set(false)>"×"</button><h2>{move||ui.tr("settings")}</h2><label>"Language"<select prop:value=move||ui.lang.get() on:change=move|e|ui.lang.set(event_target_value(&e))>{LANGUAGES.into_iter().map(|(code,name)|view!{<option value=code>{name}</option>}).collect_view()}</select></label><div class="setting-row"><span>"Theme"</span><button on:click=move |_|dark.update(|v|*v=!*v)>{move||if dark.get(){"☾ Dark"}else{"☀ Light"}}</button></div><div class="setting-row"><span>"Layout"</span><button on:click=move |_|vertical.update(|v|*v=!*v)>{move||if vertical.get(){"↕ Vertical"}else{"↔ Horizontal"}}</button></div><div class="setting-row"><span>"Text size"</span><button aria-label="Decrease font" on:click=move |_|font.update(|v|*v=(*v-1).max(14))>"A−"</button><span>{move||font.get()}</span><button aria-label="Increase font" on:click=move |_|font.update(|v|*v=(*v+1).min(24))>"A+"</button></div>
    <div class="camera-box"><video id="camera-preview" autoplay muted playsinline style:display=move||if ui.camera.get(){"block"}else{"none"}></video><button on:click=move |_|{if ui.camera.get_untracked(){camera_stop();ui.camera.set(false);}else{spawn_local(async move{ui.camera.set(true);match camera_start().await{Ok(_)=>{},Err(e)=>{ui.camera.set(false);ui.notice.set(error_js(e));}}});}}>{move||if ui.camera.get(){"◉ Camera on · Turn off"}else{"▣ Enable camera"}}</button><p>"When enabled, one camera frame accompanies each message you send. No video is stored. Microphone and camera stop when this page closes."</p></div>
    <div class="qr-card"><img src=move||format!("/qr?table={}",table_code(&ui.state.get(),ui.state.get()["table"].as_i64().unwrap_or(1))) alt="Scan to order at this table"/><span>"Scan, sit back, and say hello."</span></div></section></div></Show>
    <Show when=move||login.get()><Login ui=ui open=login/></Show>
    </div>}
}
#[component]
fn GuestOrderSheet(ui: Ui) -> impl IntoView {
    view! {<Show when=move||arr(&ui.state.get(),"cart").is_empty()&&!arr(&ui.state.get(),"orders").is_empty() fallback=move||view!{<Basket ui=ui/>}>{move||{let orders=arr(&ui.state.get(),"orders");let order=orders.iter().find(|o|!matches!(o["status"].as_str(),Some("cancelled"|"refunded"))).or(orders.first()).cloned().unwrap_or(Value::Null);view!{<section class="order-receipt"><h3>{format!("{} #{}",label(ui,"내 주문","Your order"),order["id"])}</h3><p class="receipt-status">{ui.tr(&val(&order,"status"))}</p>{arr(&order,"items").into_iter().map(|item|view!{<div class="receipt-line"><span>{format!("{} × {}",item["quantity"],food(&ui.lang.get(),&val(&item,"id"),&val(&item,"name")))}</span><strong>{money(item["price"].as_i64().unwrap_or(0)*item["quantity"].as_i64().unwrap_or(0))}</strong></div>}).collect_view()}<div class="basket-total"><span>{label(ui,"합계","Total")}</span><strong>{money(order["total"].as_i64().unwrap_or(0))}</strong></div><small>{if order["payment_status"]=="paid"{label(ui,"결제 완료","Paid")}else{label(ui,"미결제 · 직원에게 결제해 주세요","Unpaid · Pay with staff")}}</small></section>}}}</Show>}
}
#[component]
fn OrderVisual(ui: Ui) -> impl IntoView {
    view! {<Show when=move||!ui.state.get()["order_visual"].is_null()><section class="order-visual" aria-label="Order illustration">
      <Show when=move||ui.state.get()["order_visual"]["status"]=="complete"><img src=move||val(&ui.state.get()["order_visual"],"image_path") alt=move||label(ui,"주문 음식의 AI 참고 이미지","AI illustration of your selected food")/><small>{move||label(ui,"AI 참고 이미지 · 실제 음식과 수량은 주문서를 확인해 주세요.","AI illustration · Your item list confirms quantities and prices.")}</small></Show>
      <Show when=move||ui.state.get()["order_visual"]["status"]=="available"><button class="full" disabled=move||ui.busy.get() on:click=move |_|ui.run("order_visual",json!({}))>{move||label(ui,"내 음식 그림으로 보기","Picture my order")}</button></Show>
      <Show when=move||matches!(ui.state.get()["order_visual"]["status"].as_str(),Some("pending"|"generating"))><p role="status">{move||label(ui,"팬더가 음식 그림을 만들고 있어요. 주문은 계속할 수 있어요.","Panda is drawing your food. You can keep ordering.")}</p></Show>
      <Show when=move||ui.state.get()["order_visual"]["status"]=="paused"><p role="status">{move||label(ui,"그림을 만들지 못했어요. 주문 내역은 그대로 확인할 수 있어요.","The picture is unavailable. Your item list is still ready.")}</p></Show>
    </section></Show>}
}
#[component]
fn Basket(ui: Ui) -> impl IntoView {
    view! {<section class="basket"><Show when=move||ui.state.get()["role"]!="guest"><label>"주문 테이블 / Order table"<select prop:value=move||ui.state.get()["table"].to_string() on:change=move|e|ui.run("table_select",json!({"table":event_target_value(&e).parse::<i64>().unwrap_or(1)}))>{move||arr(&ui.state.get(),"tables").into_iter().filter(|t|t["active"]==true).map(|t|view!{<option value=t["id"].to_string()>{val(&t,"code")}</option>}).collect_view()}</select></label></Show><div class="basket-title"><h3>"♧ "{move||ui.tr("basket")}</h3><span>{move||arr(&ui.state.get(),"cart").iter().map(|v|v["quantity"].as_i64().unwrap_or(0)).sum::<i64>()}</span></div><Show when=move||arr(&ui.state.get(),"cart").is_empty()><p class="muted">{move||ui.tr("empty")}</p></Show>{move||arr(&ui.state.get(),"cart").into_iter().enumerate().map(|(index,v)|{let item=v.clone();view!{<div class="basket-line"><span>{format!("{} × {}",v["quantity"],food(&ui.lang.get(),&val(&v,"id"),&val(&v,"name")))}<small>{val(&v,"note")}</small></span><strong>{money(v["price"].as_i64().unwrap_or(0)*v["quantity"].as_i64().unwrap_or(0))}</strong><button class="basket-remove" aria-label=format!("Remove {}",val(&v,"name")) disabled=move||ui.busy.get() on:click=move |_|ui.run("cart_remove",json!({"index":index,"item":item}))>"×"</button></div>}}).collect_view()}<Show when=move||!arr(&ui.state.get(),"cart").is_empty()><div class="basket-total"><span>"Total"</span><strong>{move||money(arr(&ui.state.get(),"cart").iter().map(|v|v["price"].as_i64().unwrap_or(0)*v["quantity"].as_i64().unwrap_or(0)).sum())}</strong></div><button class="primary full" disabled=move||ui.busy.get() on:click=move |_|ui.run("checkout",json!({"request_id":format!("{}-{}",js_sys::Date::now(),js_sys::Math::random())}))>{move||ui.tr("checkout")} " →"</button><button class="text-button" disabled=move||ui.busy.get() on:click=move |_|ui.run("cart_clear",json!({}))>{move||ui.tr("clear")}</button></Show></section>}
}
#[component]
fn Login(ui: Ui, open: RwSignal<bool>) -> impl IntoView {
    let phrase = NodeRef::<leptos::html::Input>::new();
    let wallet_index = RwSignal::new("0".to_string());
    let working = RwSignal::new(false);
    let message = RwSignal::new(String::new());
    view! {<div class="modal-backdrop"><section class="modal" role="dialog" aria-modal="true" aria-label="Wallet login"><button class="close" on:click=move |_|open.set(false)>"×"</button><span class="avatar">"🐼"</span><h2>{move||ui.tr("login")}</h2><p>"Enter your wallet recovery phrase. It is used locally to sign a one-time challenge, never sent to the café or AI."</p><form on:submit=move|e|{e.prevent_default();if working.get_untracked(){return;}let index=match wallet_index.get_untracked().parse::<u32>(){Ok(index) if index < (1u32<<31)=>index,_=>{message.set("Wallet index must be a whole number from 0 to 2147483647.".into());return;}};message.set(String::new());working.set(true);let words=zeroize::Zeroizing::new(phrase.get().map(|el|{let v=el.value();el.set_value("");v}).unwrap_or_default());spawn_local(async move{let result=async {let identity=Identity::from_mnemonic_at(&words,index)?;drop(words);let c=rpc("","challenge",json!({"wallet":identity.address()})).await?;let signature=identity.sign(c["message"].as_str().ok_or("Missing challenge")?)?;let session=rpc("","login",json!({"nonce":c["nonce"],"signature":signature})).await?;ui.token.set(val(&session,"token"));if let Some(s)=storage(){let _=s.set_item("cafe-token",&ui.token.get_untracked());}ui.refresh().await?;ui.tab.set(if ui.state.get_untracked()["role"]=="guest"{"chat"}else{"orders"}.into());open.set(false);Ok::<(),String>(())}.await;if let Err(e)=result{message.set(e);}working.set(false);});}><label>"Recovery phrase"<input node_ref=phrase type="password" autocomplete="off" spellcheck="false" placeholder="12 or 24 words" required/></label>
    <label>"Wallet index"<input aria-label="Wallet index" type="number" inputmode="numeric" min="0" max="2147483647" step="1" required prop:value=move||wallet_index.get() on:input=move|e|wallet_index.set(event_target_value(&e))/></label>
    <p class="wallet-index-help">"Default: 0. Use the index of the wallet your café owner authorized."</p>
    <button class="primary full" type="submit" disabled=move||working.get()>{move||if working.get(){"Signing in…"}else{"Unlock café"}}</button></form><p role="alert">{move||message.get()}</p><Show when=move||!ui.state.get()["wallet"].is_null()><button class="text-button" on:click=move |_|{stop_listening();camera_stop();ui.listening.set(false);ui.camera.set(false);spawn_local(async move{let _=rpc(&ui.token.get_untracked(),"logout",json!({})).await;if let Some(s)=storage(){let _=s.remove_item("cafe-token");}let _=web_sys::window().unwrap().location().reload();});}>"Sign out"</button></Show><small>"Guest wallets can sign in too. Faucet credits are stored in the backend against your wallet and remain after signing out. Anonymous ordering is also available."</small></section></div>}
}
