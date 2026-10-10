-- Fresh release schema. Rust owns these tables; Directus creates its own tables.
-- Older development schemas must be backed up and reset before using this baseline.
CREATE FUNCTION valid_product_components(value JSONB) RETURNS BOOLEAN
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE WHEN jsonb_typeof(value)='object' THEN NOT EXISTS (
        SELECT 1 FROM jsonb_each(value) entry WHERE NOT coalesce(
            jsonb_typeof(entry.value)='string' OR (
                jsonb_typeof(entry.value)='object'
                AND jsonb_typeof(entry.value->'id')='string'
                AND jsonb_typeof(entry.value->'label')='string'),false)
    ) ELSE false END
$$;

CREATE TABLE admin_revision (
    id INTEGER PRIMARY KEY CHECK (id=1),
    revision BIGINT NOT NULL DEFAULT 0,
    alert_sequence BIGINT NOT NULL DEFAULT 0 CHECK (alert_sequence>=0)
);
INSERT INTO admin_revision(id) VALUES(1);

-- Independent counters: detail reads must not acquire catalog-row/table locks.
CREATE TABLE catalog_views (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    target_type TEXT NOT NULL CHECK (target_type IN ('product','variant')),
    target_id TEXT NOT NULL,
    views BIGINT NOT NULL DEFAULT 0 CHECK (views>=0),
    UNIQUE(target_type,target_id)
);

CREATE TABLE products (
    id TEXT PRIMARY KEY,
    section TEXT NOT NULL,
    retailer TEXT NOT NULL,
    url TEXT NOT NULL,
    price INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('InStock','OutOfStock','Arriving','OnDemand')),
    removed BOOLEAN NOT NULL DEFAULT false,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    image TEXT NOT NULL,
    components JSONB NOT NULL CHECK (valid_product_components(components)),
    details JSONB NOT NULL CHECK (
        jsonb_typeof(details)='object'
        AND NOT details ?| ARRAY['id','section','retailer','url','price','status','title','description','image','components','views']
        AND jsonb_typeof(details->'notes')='array'
        AND NOT jsonb_path_exists(details->'notes','$[*] ? (@.type() != "string")')
        AND jsonb_typeof(details->'images')='array'
        AND NOT jsonb_path_exists(details->'images','$[*] ? (@.type() != "string")')
        AND jsonb_typeof(details->'history')='array'
        AND jsonb_typeof(details->'approved')='boolean'
        AND details ?& ARRAY['notes','images','history','approved']
    )
);
CREATE INDEX products_section_idx ON products(section) WHERE removed=false;
CREATE INDEX products_retailer_idx ON products(retailer) WHERE removed=false;
CREATE INDEX products_url_idx ON products(url);
CREATE INDEX products_price_idx ON products(price) WHERE removed=false;

CREATE TABLE variants (
    id TEXT PRIMARY KEY,
    payload JSONB NOT NULL CHECK (jsonb_typeof(payload)='object' AND NOT payload ?| ARRAY['id','views'])
);
CREATE TABLE product_info_cache (
    url TEXT PRIMARY KEY,
    expires_at TIMESTAMPTZ NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL,
    description TEXT NOT NULL,
    images JSONB NOT NULL CHECK (jsonb_typeof(images)='array' AND NOT jsonb_path_exists(images,'$[*] ? (@.type() != "string")'))
);
CREATE INDEX product_info_cache_expiry_idx ON product_info_cache(expires_at);

CREATE TABLE users (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL UNIQUE CONSTRAINT users_email_normalized CHECK (email=lower(email)),
    given_name TEXT,
    family_name TEXT,
    avatar_url TEXT,
    password_hash TEXT,
    email_verified_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE auth_identities (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    provider_subject TEXT NOT NULL,
    provider_email TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(provider,provider_subject)
);
CREATE INDEX auth_identities_user_idx ON auth_identities(user_id);
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    csrf_hash TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    user_agent TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX sessions_user_idx ON sessions(user_id);
CREATE INDEX sessions_expiry_idx ON sessions(expires_at);
CREATE TABLE pending_registrations (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    expires_at TEXT NOT NULL
);
CREATE INDEX pending_registrations_email ON pending_registrations(email);
CREATE INDEX pending_registrations_expiry ON pending_registrations(expires_at);
CREATE TABLE account_tokens (
    id TEXT PRIMARY KEY,
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    registration_id TEXT REFERENCES pending_registrations(id) ON DELETE CASCADE,
    purpose TEXT NOT NULL CHECK (purpose IN ('verify_registration','verify_email','reset_password')),
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (num_nonnulls(user_id,registration_id)=1),
    CHECK ((purpose='verify_registration')=(registration_id IS NOT NULL))
);
CREATE INDEX account_tokens_user_purpose ON account_tokens(user_id,purpose);
CREATE INDEX account_tokens_registration ON account_tokens(registration_id);
CREATE INDEX account_tokens_expiry ON account_tokens(expires_at);
CREATE TABLE account_notifications (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('welcome','password_changed','password_reset')),
    created_at TEXT NOT NULL,
    sent_at TEXT,
    attempts BIGINT NOT NULL DEFAULT 0,
    next_attempt_at TEXT NOT NULL,
    lease_token TEXT,
    lease_until TEXT
);
CREATE INDEX account_notifications_pending ON account_notifications(sent_at,next_attempt_at);
CREATE TABLE wishlist_items (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    target_type TEXT NOT NULL CHECK (target_type IN ('variant','product')),
    target_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(user_id,target_type,target_id)
);
CREATE INDEX wishlist_items_user_created_idx ON wishlist_items(user_id,created_at DESC);

CREATE TABLE alerts (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    rule TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('active','paused','pending','delivered','failed','expired')),
    cursor BIGINT NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    last_match TEXT,
    last_matched_at TEXT,
    last_delivery_at TEXT,
    lowest_seen INTEGER
);
CREATE INDEX alerts_user ON alerts(user_id,created_at DESC);
CREATE INDEX alerts_active ON alerts(state,cursor);
CREATE INDEX alerts_expiry ON alerts(expires_at) WHERE state IN ('active','paused','pending','failed');
CREATE INDEX alerts_user_live ON alerts(user_id,expires_at) WHERE state IN ('active','pending');
CREATE TABLE alert_events (
    sequence BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    event_key TEXT NOT NULL UNIQUE,
    section TEXT NOT NULL,
    product_id TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    payload TEXT NOT NULL,
    batch_id TEXT
);
CREATE INDEX alert_events_product_sequence ON alert_events(product_id,sequence);
CREATE INDEX alert_events_section_sequence ON alert_events(section,sequence);
CREATE INDEX alert_events_batch ON alert_events(batch_id,sequence);
CREATE FUNCTION alert_event_cursor() RETURNS BIGINT LANGUAGE sql STABLE
SET search_path=public,pg_temp AS $$
    SELECT greatest(alert_sequence,(SELECT coalesce(max(sequence),0) FROM alert_events))
    FROM admin_revision WHERE id=1
$$;
CREATE TABLE alert_deliveries (
    alert_id TEXT PRIMARY KEY REFERENCES alerts(id) ON DELETE CASCADE,
    attempts BIGINT NOT NULL DEFAULT 0,
    next_attempt_at TEXT NOT NULL,
    lease_token TEXT,
    lease_until TEXT
);
CREATE INDEX alert_delivery_pending ON alert_deliveries(next_attempt_at);

CREATE TABLE scrape_errors (
    key TEXT PRIMARY KEY,
    retailer TEXT NOT NULL,
    active BOOLEAN NOT NULL,
    reviewed BOOLEAN NOT NULL,
    last_seen TIMESTAMPTZ NOT NULL,
    reviewed_at TIMESTAMPTZ,
    review_note TEXT,
    payload JSONB NOT NULL CHECK (jsonb_typeof(payload)='object' AND NOT payload ?| ARRAY['key','retailer','active','reviewed','reviewed_at','last_seen'])
);
CREATE INDEX scrape_errors_status ON scrape_errors(active,retailer);
CREATE TABLE catalog_activity (
    sequence BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    event_key TEXT NOT NULL UNIQUE,
    observed_at TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('added','edited','removed')),
    section TEXT NOT NULL,
    retailer TEXT NOT NULL,
    product_id TEXT NOT NULL,
    title TEXT NOT NULL,
    payload JSONB NOT NULL CHECK (jsonb_typeof(payload)='object' AND NOT payload ?| ARRAY['id','title'])
);
CREATE INDEX catalog_activity_date ON catalog_activity(observed_at,sequence);
CREATE INDEX catalog_activity_timeline ON catalog_activity(observed_at DESC,product_id ASC,sequence DESC);
CREATE INDEX catalog_activity_section_timeline ON catalog_activity(section,observed_at DESC,product_id ASC,sequence DESC);
CREATE INDEX catalog_activity_retailer_timeline ON catalog_activity(retailer,observed_at DESC,product_id ASC,sequence DESC);
CREATE INDEX catalog_activity_retailer_section_timeline ON catalog_activity(retailer,section,observed_at DESC,product_id ASC,sequence DESC);

CREATE TABLE feature_votes (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    feature_id TEXT NOT NULL,
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    browser_hash TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    CHECK (num_nonnulls(user_id,browser_hash)=1),
    UNIQUE(feature_id,user_id),
    UNIQUE(feature_id,browser_hash)
);
CREATE INDEX feature_votes_user ON feature_votes(user_id);
CREATE INDEX feature_votes_browser ON feature_votes(browser_hash);
CREATE TABLE product_overrides (
    id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    product_url TEXT NOT NULL CHECK (length(product_url)>0),
    timestamp TEXT NOT NULL DEFAULT (to_char(clock_timestamp() AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"')) CHECK (timestamp::timestamptz IS NOT NULL),
    changes JSONB NOT NULL CHECK (jsonb_typeof(changes)='array' AND jsonb_array_length(changes)>0),
    source TEXT NOT NULL DEFAULT 'directus'
);
CREATE INDEX product_overrides_url_history ON product_overrides(product_url,timestamp,id);

CREATE FUNCTION product_document(product products,view_count BIGINT) RETURNS JSONB LANGUAGE sql IMMUTABLE STRICT
SET search_path=public,pg_temp AS $$
    SELECT product.details || jsonb_build_object(
        'id',product.id,'section',product.section,'retailer',product.retailer,
        'url',product.url,'price',product.price,'status',product.status,
        'title',product.title,'description',product.description,'image',product.image,'components',product.components,
        'views',view_count)
$$;
CREATE FUNCTION product_document(product products) RETURNS JSONB LANGUAGE sql STABLE STRICT
SET search_path=public,pg_temp AS $$
    SELECT product_document(product,coalesce((SELECT views FROM catalog_views WHERE target_type='product' AND target_id=product.id),0))
$$;
CREATE OR REPLACE FUNCTION capture_product_override() RETURNS trigger LANGUAGE plpgsql
SET search_path=public,pg_temp AS $$
DECLARE
    old_product JSONB;
    edited_product JSONB;
    edit JSONB;
    changes JSONB;
    source TEXT;
    edited_at TEXT;
BEGIN
    IF current_user='flagship_directus' THEN source := 'directus';
    ELSIF current_setting('flagship.override_source',true)='discord' THEN source := 'discord';
    ELSE
        IF NEW IS DISTINCT FROM OLD THEN UPDATE admin_revision SET revision=revision+1 WHERE id=1; END IF;
        RETURN NEW;
    END IF;
    old_product := product_document(OLD);
    edited_product := product_document(NEW);
    IF NEW.id IS DISTINCT FROM OLD.id THEN RAISE EXCEPTION 'Product ID cannot be modified'; END IF;
    IF source='directus' THEN
        SELECT jsonb_agg(jsonb_build_object('field',field.key,'old_value',old_product->field.key,'new_value',field.value) ORDER BY field.key)
        INTO changes FROM jsonb_each(edited_product) field
        WHERE old_product ? field.key AND field.key NOT IN ('id','history','updated_at')
            AND old_product->field.key IS DISTINCT FROM field.value;
        -- History is maintained by the application; normal Directus field edits
        -- automatically append a URL-based override in the same transaction.
        NEW.details := jsonb_set(NEW.details,'{history}',old_product->'history');
        IF changes IS NOT NULL THEN
            edited_at := to_char(clock_timestamp() AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"');
            edit := jsonb_build_object('timestamp',edited_at,'changes',changes);
            NEW.details := jsonb_set(NEW.details,'{history}',old_product->'history'||jsonb_build_array(edit));
            NEW.details := jsonb_set(NEW.details,'{updated_at}',to_jsonb(edited_at));
        END IF;
    ELSE
        edit := edited_product->'history'->-1;
        IF edit IS NOT NULL AND NOT (old_product->'history') @> jsonb_build_array(edit) THEN
            changes := edit->'changes'; edited_at := edit->>'timestamp';
        END IF;
    END IF;
    IF changes IS NOT NULL AND jsonb_array_length(changes)>0 THEN
        INSERT INTO product_overrides(product_url,timestamp,changes,source) VALUES(OLD.url,edited_at,changes,source);
    END IF;
    IF NEW IS DISTINCT FROM OLD THEN UPDATE admin_revision SET revision=revision+1 WHERE id=1; END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER product_override_capture BEFORE UPDATE ON products FOR EACH ROW EXECUTE FUNCTION capture_product_override();


CREATE FUNCTION capture_scrape_review() RETURNS trigger LANGUAGE plpgsql SET search_path=public,pg_temp AS $$
BEGIN
    IF NEW.reviewed IS DISTINCT FROM OLD.reviewed THEN
        NEW.reviewed_at := CASE WHEN NEW.reviewed THEN clock_timestamp() ELSE NULL END;
    END IF;
    IF (NEW.reviewed,NEW.reviewed_at,NEW.review_note) IS DISTINCT FROM (OLD.reviewed,OLD.reviewed_at,OLD.review_note) THEN
        UPDATE admin_revision SET revision=revision+1 WHERE id=1;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER scrape_review_capture BEFORE UPDATE ON scrape_errors FOR EACH ROW EXECUTE FUNCTION capture_scrape_review();

-- All writers advance the database revision, including application upserts.
-- A published catalog can be reused only while this revision still matches.
CREATE FUNCTION capture_product_structure() RETURNS trigger LANGUAGE plpgsql
SET search_path=public,pg_temp AS $$
BEGIN
    UPDATE admin_revision SET revision=revision+1 WHERE id=1;
    RETURN NULL;
END $$;
CREATE TRIGGER product_structure_capture AFTER INSERT OR DELETE ON products
    FOR EACH STATEMENT EXECUTE FUNCTION capture_product_structure();
