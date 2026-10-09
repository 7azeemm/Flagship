-- The optional duplicate catalog cache and unused retailer metadata are removed.
-- OAuth handshakes now live in a bounded, browser-bound in-process store.
DROP TABLE oauth_attempts, app_state, retailers;

-- Keep the last committed event sequence independently of queue retention.
-- Reading the sequence object itself would include uncommitted scan inserts
-- and could cause an evaluator to skip events when that scan later commits.
ALTER TABLE admin_revision ADD COLUMN alert_sequence BIGINT NOT NULL DEFAULT 0 CHECK (alert_sequence>=0);
UPDATE admin_revision SET alert_sequence=(SELECT coalesce(max(sequence),0) FROM alert_events) WHERE id=1;
CREATE FUNCTION alert_event_cursor() RETURNS BIGINT LANGUAGE sql STABLE
SET search_path=public,pg_temp AS $$
    SELECT greatest(alert_sequence,(SELECT coalesce(max(sequence),0) FROM alert_events))
    FROM admin_revision WHERE id=1
$$;

-- Only observations beyond a live watch's cursor still need processing.
-- Pending recurring watches keep their later observations while email sends.
DELETE FROM alert_events WHERE sequence <= coalesce(
    (SELECT min(cursor) FROM alerts WHERE state IN ('active','pending')
        AND expires_at > to_char(clock_timestamp() AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"+00:00"')),
    (SELECT alert_sequence FROM admin_revision WHERE id=1)
);
DELETE FROM alert_deliveries delivery USING alerts alert WHERE alert.id=delivery.alert_id
    AND (delivery.sent_at IS NOT NULL OR alert.state NOT IN ('active','pending'))
    AND (delivery.lease_until IS NULL OR delivery.lease_until <= to_char(clock_timestamp() AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"+00:00"'));

-- Only pending jobs remain; success is recorded on alerts.last_delivery_at.
ALTER TABLE alert_deliveries DROP COLUMN sent_at;
