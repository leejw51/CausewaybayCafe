-- Unknown historical mode is excluded from recommendations rather than assumed live.
ALTER TABLE orders ADD COLUMN simulation INTEGER CHECK(simulation IN (0,1));
CREATE INDEX orders_trends ON orders(simulation,created);
