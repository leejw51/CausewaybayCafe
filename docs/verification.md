# Latest verification — 2026-11-03

- `make stresstest` passed: **40 Rust tests** (7 server unit, 28 workflow, 5 shared), **60 Playwright tests**, and **64 complete simulated order/service/payment flows with 8 concurrent clients**.
- SQLite integrity, JSONL consistency and zero paid AI calls were checked for the synthetic stress run.
- Repeated layout/theme changes retain guest CSS classes; 80-message conversations scroll internally with the composer in view on desktop, tablet and phone dimensions.
- Owner 100× virtual service updates table status through server events; real guest orders remain unchanged. Owner-only floor-plan application validates IDs, seats and coordinates and requires confirmation.
- Real Meta guest order-image smoke test passed: one image generated, duplicate requests reused one job, output decoded and stored. This is separate from the offline suite.
- After the final receipt change, 24 checkout/layout browser tests passed again and all 40 Rust tests passed. Committed orders are displayed in the side sheet and no further model step can deny a successful write.
- Physical restaurant-photo recognition, physical microphone quality and real public judge access still require manual acceptance checks. Floor-plan output is approximate, not measured geometry.
- Historical entries below describe earlier builds; the README and this section describe current behavior. In particular, the current app uses server events rather than five-second order polling.

---

# Verification — 2026-11-02

- Rust backend and Leptos WebAssembly build passed.
- 15 offline tests passed: wallet derivation/index/signatures, malformed Cap’n Proto, session privacy, authoritative pricing, idempotent checkout, owner/staff permissions and revocation, migration restart, confirmation punctuation, stale order edits, duplicate payments, payment/refund revenue, atomic management batches, table code/QR and shop GUID checks, Run gate, concurrent token reservations, unknown draft prices and draft version checks.
- Live Meta chat passed with `MUSE_API_KEY`: two milk teas, then explicit confirmation, created the expected HK$56 in-memory test order.
- Live Meta vision + image generation passed: the synthetic menu fixture was recognized as HK$28, HK$18 and one unknown/null price. One Muse Image generation completed and decoded successfully. These two calls reported 18,107 input and 2,396 output tokens in that test. Reservation estimates were subsequently raised for photo requests; unknown/internal tokens mean admission limits are not an absolute provider billing cap.
- Browser checked Owner menu-photo tab, Staff wallet-index login, Owner/Staff daily/hourly token dashboards, role-restricted management controls, and mobile 390px photo screen. Screenshots are in `docs/previews/`. Role tests used a separate temporary database and public test identities, not the real cafe wallet.
- Prior HTTP/3 localhost QUIC integration passed for health and Cap’n Proto; HTTP/3 implementation unchanged by management additions.
- Existing live SQLite databases backed up under `.run/backups/` before migration. The preview runs at `http://127.0.0.1:8790`; `localhost` in this browser has another app's cached service worker. That cache was left untouched.

Hardware camera capture, microphone permissions and all seven speech languages still require device validation. Optional OpenAI/Grok and public HTTPS hosting have not been exercised. Core navigation supports seven languages; secondary management copy is partly English/Korean. Payment actions record receipts/refunds and do not charge cards or transfer money.

Re-run offline checks with `make test`, build with `make check`, or opt into paid provider checks with `make test-meta` / `make test-meta-photo`. The photo test uses `crates/server/tests/fixtures/menu-photo.jpg` and an in-memory database; generated test image files are removed.

## Makefile / remote binding / repeatable Playwright update

- `make start` and `make stop` exercised successfully. Frontend binds `0.0.0.0:8791`; backend binds `0.0.0.0:8788`. Port 8790 belongs to another app (`workerd`) and was left untouched. The current LAN URL `http://172.30.1.54:8791/health` returned `ok`; QR base URL was updated from localhost to that LAN origin.
- `make stresstest` passed: 16 offline Rust tests plus 3 Playwright scenarios covering Owner management, Guest checkout, Staff service/payment/revocation, 8 concurrent guests and mobile preferences. These use temporary databases and no provider credentials.
- The concurrent browser test exposed SQLite read-to-write transaction lock upgrades. Checkout and management confirmation now use `BEGIN IMMEDIATE`, with WAL mode for file databases. A 16-way barrier-synchronized file-database checkout regression test passes without retries or dropped orders.
- Added Cargo migration-directory tracking so newly added SQL migrations are embedded when rebuilding.
- Browser report: `playwright-report/index.html`. Persistent screenshots: `docs/previews/playwright-*.png`. Remote browser microphone/camera streaming still requires HTTPS; these checks do not simulate physical devices or call paid providers.

- Repeated the 8-guest browser concurrency scenario three times after fixes (24 additional guest flows): all passed. The Python frontend now uses HTTP/1.1 persistent connections and a 128-connection accept queue; normal client disconnects are handled without attempting to send another response on a closed socket.

## Always-on backend / AI-only minute interval

- Backend HTTP handling remains continuous and independent of AI Run/Stop. Only the AI queue worker waits 60 seconds between work passes.
- Database-only UI refresh now runs every 5 seconds while visible, skips overlapping requests, and rejects stale background responses after a newer action or login. Process health monitoring checks every second (and on child exit signals).
- `make stresstest` passed again: 16 Rust tests and 3 Playwright scenarios. The full-flow scenario now explicitly waits for the Guest screen to receive Staff payment completion automatically while AI remains stopped, without a manual refresh.

## Guest text activates AI on demand

- Guest chat no longer requires Owner/Staff Run. The exception applies only to guest chat reservations; queued images, transcription and staff AI retain the Run gate. Guest messages do not modify the global Run flag and still obey token budgets/rate limits.
- 17 offline Rust tests and 4 Playwright scenarios passed, including typed guest ordering and confirmation with Run off.
- The real Meta chat test also passed with Run off: two milk teas produced the correct basket and a confirmed HK$56 order in an isolated in-memory database.

## Default customer functions

- Added `menu_list`, `choose_menu`, `cancel_menu` (`can_menu` alias) and `current_progress` as session-authenticated RPC operations and AI planner actions. Exact chat commands bypass the model and consume no AI tokens.
- Menu prices/availability and order progress are returned from the database. Cancellation removes only the caller's basket selections; it validates quantities, preserves remaining notes and cannot alter submitted orders or another guest's basket.
- Four chat shortcuts have labels for all seven UI languages.
- `make stresstest` passed: 18 Rust tests and 5 Playwright scenarios. The new scenario exercises list → choose → cancel selected food → empty progress → typed order → actual progress. Screenshot: `docs/previews/default-functions.png`.
- “can menu” is provisionally interpreted as “cancel menu”; invoking it without an item asks which basket selection to remove, never clears the basket automatically.

## Order polling removed — supersedes the earlier five-second refresh

- Removed the order refresh interval completely. Requests execute immediately; other screens receive authenticated SSE change notifications and fetch their session-authorized state only on an event. Reconnect performs an initial resync. SSE heartbeat comments do not trigger state requests or AI calls.
- The frontend proxy streams SSE without buffering, with bearer credentials in headers rather than URLs. HTTP/3 now streams response chunks instead of buffering whole response bodies.
- `make stresstest`: 19 Rust tests and 6 Playwright scenarios passed. New checks cover missing stream authorization, no notifications from state reads, notifications after checkout, and zero periodic RPC requests during an idle 6.5-second browser observation. Cross-screen payment completion is checked within 3 seconds without manual refresh.
- `make test-http3` passed, including authenticated SSE headers and the first change event over QUIC.
- Only queued background AI work retains the 60-second work interval.

## Test faucet

- Added opt-in HK$10 test credits, persisted separately from payments, with one atomic grant per wallet or anonymous guest session. The backend determines the recipient and amount.
- `make stresstest` passed: 20 offline Rust tests and 7 Playwright scenarios. Coverage includes concurrent duplicate claims, account isolation, persistence after reopening SQLite, disabled faucet rejection, and claiming/reloading through the UI.
- Browser screenshot: `docs/previews/test-faucet.png`.

## Owner simulation mode

- Simulation is the initial startup default. Owner-only, reviewed mode changes persist in SQLite and the JSONL journal. Guest/Staff cannot propose or execute mode changes. Switching stops background AI without changing databases or deleting orders.
- Simulation blocks paid AI reservations; deterministic ordering and direct controls remain available. Background images skip simulation. Existing provider requests may finish.
- 22 Rust tests and 8 Playwright scenarios passed, including mode persistence across database reopening, role rejection, simulated paid-call rejection, and the Owner UI toggle with confirmation and reload.

## Automatic order/service commands

- `make simulationtest` passed: 12 orders automatically placed, prepared, marked ready, served and paid.
- `make stresstest` passed: 22 Rust tests, 8 Playwright scenarios and 64 complete flows with 8 concurrent clients.
- Disposable loopback test servers verified guest isolation, checkout idempotency, prices, completion/payment counts and totals, SQLite integrity, complete JSONL export and zero AI usage. Normal cafe processes/data were left intact.
- `make start` remains ordinary server operation, preserving the Owner mode setting; no automatic customer/service worker is started.

## Guest simulation meal controls

- Guest Ready marks their own new/preparing order ready; Pay accepts ready/served orders and atomically records a simulation payment plus completed/served status. Mode and ownership are checked inside the write transaction. Duplicate submissions do not duplicate payments.
- `make stresstest` passed: 23 Rust tests, 9 Playwright scenarios, 64 automatic service/payment flows. Added checks for cross-guest rejection, Live rejection, Pay-before-Ready rejection, repeated/concurrent Pay, zero AI usage and UI completion/reload persistence.

## Explicit Makefile launch modes

- `make start` now explicitly sets Simulation at startup; `make normaltest` explicitly sets Live. Owner can still change mode during operation. Both reuse the configured database, preserve data, stop background AI at startup and do not start automatic order workers.
- Verified actual Makefile invocations on isolated ports/database: start → normaltest → start yielded simulation 1 → 0 → 1, with background AI stopped and a saved shop name preserved. `make stop` cleaned up the isolated processes.

## Guest chat-first interface

- Guest starts on the Chat tab with a wide conversation, visible composer and voice controls, no hero, and optional menu/order/Faucet tabs. Staff/Owner login and restored sessions open Orders.
- All 10 Playwright scenarios passed. The new check verifies initial chat, hidden menu, mobile composer within the 390×844 viewport, no horizontal overflow, ordering and basket persistence across navigation. Desktop/mobile screenshots visually reviewed: `docs/previews/guest-chat-desktop.png` and `docs/previews/guest-chat-mobile.png`.

## Quantity-based group chat ordering

- `3명 메뉴 3개, 음료수 3개` proposes exactly three food items and three drinks, with authoritative current prices and an explicit confirmation step. It keeps existing baskets, avoids unavailable items, and never multiplies totals by party size. The exact pattern runs locally in both modes; Live AI can invoke the same group tool. Extra dietary/budget text is excluded from the simple matcher.
- 24 Rust tests, 11 Playwright scenarios and 64 automatic service flows passed. New tests cover exact quantities/total, no order before confirmation, repeated requests preserving the basket, sold-out categories, invalid counts, zero AI usage, and the Korean chat-to-confirmed-order flow.

## Primary demand-aware automatic composition

- Ranked recommendations use internal 7-day demand, last-hour demand and positive growth against the previous 7 days. Cancelled/refunded, unavailable/archived, future and old records are excluded. New orders record their mode; Live/Simulation histories and unknown legacy records are kept separate for ranking. Quotes show the basis and counts, with an explicit no-history fallback.
- Guest greetings in seven languages and the prominent Build my order button make automatic composition the main entry point. Confirmation remains required before submission.
- 25 Rust tests, 11 Playwright scenarios and 64 concurrent automatic flows passed for trend ranking. After the primary UI entry point was added, all 11 Playwright scenarios passed again, including mobile composer visibility and the group-composition button.

## Full Admin / Staff / Guest verification (latest run)

- `make stresstest`: 25 Rust tests, 12 Playwright scenarios, and 64 complete order/service/payment flows with 8 concurrent clients all passed. SQLite/JSONL consistency and zero AI calls in simulation were verified.
- Owner: login, staff appointment/revocation, menu/table management, QR display, dashboard/limits and Simulation/Live controls. Staff: indexed login, permissions, order editing, preparing/ready/served transitions, payment and completion. Guest: manual/chat/group ordering, private carts/orders, Ready/Pay in simulation, wallet login, HK$1,000 Faucet and persistence across independent browser sessions.
- `make test-http3`, `make test-meta`, `make test-meta-photo` passed. Real Meta chat, photo recognition and one generated image were exercised against isolated test state. Photo/image calls reported 10,812 input and 1,772 output tokens across 2 requests (1 image), no estimated usage.
- Physical microphone/camera capture, physical QR printing, external-device browser access and all seven language variants were not exhaustively exercised by this automated run. The app records payments; no actual card charge or blockchain transfer was performed.
