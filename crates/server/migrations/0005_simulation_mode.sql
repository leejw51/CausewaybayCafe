-- NULL is initialized once from the startup default; Owner changes then persist.
ALTER TABLE shop ADD COLUMN simulation INTEGER CHECK(simulation IN (0,1));
