# Guest conversation backend

Production guest chat enters `App::guest_conversation` in `crates/server/src/guest_agent.rs`.
It no longer uses the legacy keyword/JSON-action planner. Staff management and explicitly
offline test fixtures remain separate in `ai.rs`.

1. Validate guest session, language, message length and client-generated request ID.
2. Return an already completed turn from SQLite; never rerun a running/interrupted turn.
3. Save the guest message before any network call.
4. Build context from current menu, the guest's basket/orders, wallet balance, aggregate
   sales and up to 16 recent messages. Owner credentials and other guests are excluded.
5. Call Meta with native function schemas. Deserialize arguments into Rust types,
   reject unknown fields/tools, then use existing authorized transactional business functions.
6. Feed the complete assistant tool-call message and matching tool results back to Meta.
7. Once a tool commits an order, stop the model loop and render the receipt from the database. Model prose cannot contradict a successful order. Store the final reply and authoritative order/payment details.

Limits: at most 3 model responses and 4 tool calls per response, one successful mutation
per turn. Further steps can read data but cannot repeat the mutation. A vague follow-up
after a recent order cannot authorize another order. Existing token budgets apply to every
request, including tool definitions and results. No polling is introduced.

`guest_turns` stores replayable results; `guest_tool_events` stores tool execution evidence.
Both participate in the SQLite-to-JSONL journal with session tokens excluded. Checkout's
request key is stable for a turn, preventing repeat wallet charges. If the model fails
after successful execution, a server receipt is returned instead of retrying the operation.
An interrupted turn without a final reply must be reconciled by checking current orders;
it is intentionally not replayed automatically.

The browser retains a pending request ID across transport failure and refresh, scoped to
its session, language and text. Once a result is received and state refreshed, it clears
that ID. No automatic provider or mutation retry is performed.

Offline tests cover authorization, typed validation, wallet replay, interrupted turns,
persistence and existing workflows. Ignored live tests exercise real Meta conversations:

```sh
cargo test -p cafe-server --test workflows live_meta_ -- --ignored --test-threads=1
```

Meta tool-call protocol reference:
https://github.com/meta-models/meta-model-cookbook/blob/main/01_api_fundamentals/03_tool_calling.ipynb
