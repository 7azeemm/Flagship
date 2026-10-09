-- Activity columns identify the snapshot; JSON contains only its changing data.
ALTER TABLE catalog_activity ALTER COLUMN payload TYPE JSONB USING payload::jsonb-ARRAY['id','title'];
ALTER TABLE catalog_activity ADD CONSTRAINT activity_payload_valid CHECK (
    jsonb_typeof(payload)='object' AND NOT payload ?| ARRAY['id','title']
);

-- A pending alert already owns the frozen match used for its email. The delivery
-- row tracks scheduling and leases, without storing the same match a second time.
UPDATE alerts alert SET last_match=delivery.payload
FROM alert_deliveries delivery WHERE delivery.alert_id=alert.id AND delivery.sent_at IS NULL;
ALTER TABLE alert_deliveries DROP COLUMN payload;
