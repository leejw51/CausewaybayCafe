-- Test credits are deliberately separate from real order/payment accounting.
CREATE TABLE test_faucet_credits (
    account TEXT PRIMARY KEY,
    balance INTEGER NOT NULL CHECK(balance >= 0),
    claimed_at INTEGER NOT NULL
);
