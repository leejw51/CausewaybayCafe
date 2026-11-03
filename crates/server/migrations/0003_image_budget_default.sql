-- A photo plus one illustration needs more than the original 20k default.
-- Keep any budget that an owner has explicitly configured.
UPDATE shop SET hourly_token_limit=50000
WHERE hourly_token_limit=20000 AND NOT EXISTS(SELECT 1 FROM audit WHERE action='ai_limits');
