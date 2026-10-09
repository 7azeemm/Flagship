-- Indexed and editable product fields have one canonical copy. Flexible
-- attributes and historical deltas remain JSON; they are not duplicate rows.
DROP TRIGGER product_override_capture ON products;
ALTER TABLE products ALTER COLUMN title DROP EXPRESSION;
ALTER TABLE products ALTER COLUMN description DROP EXPRESSION;
ALTER TABLE products ALTER COLUMN image DROP EXPRESSION;
ALTER TABLE products ALTER COLUMN components DROP EXPRESSION;
ALTER TABLE products ADD COLUMN details JSONB;
UPDATE products SET details=payload::jsonb-ARRAY['id','section','retailer','url','price','status','title','description','image','components'];
ALTER TABLE products DROP COLUMN payload;
ALTER TABLE products ALTER COLUMN details SET NOT NULL;
ALTER TABLE products ALTER COLUMN title SET NOT NULL;
ALTER TABLE products ALTER COLUMN description SET NOT NULL;
ALTER TABLE products ALTER COLUMN image SET NOT NULL;
ALTER TABLE products ALTER COLUMN components SET NOT NULL;
ALTER TABLE products ADD CONSTRAINT products_components_valid CHECK (valid_product_components(components));
ALTER TABLE products ADD CONSTRAINT products_status_valid CHECK (status IN ('InStock','OutOfStock','Arriving','OnDemand'));
ALTER TABLE products ADD CONSTRAINT products_details_valid CHECK (
    jsonb_typeof(details)='object'
    AND NOT details ?| ARRAY['id','section','retailer','url','price','status','title','description','image','components']
    AND jsonb_typeof(details->'notes')='array'
    AND NOT jsonb_path_exists(details->'notes','$[*] ? (@.type() != "string")')
    AND jsonb_typeof(details->'images')='array'
    AND NOT jsonb_path_exists(details->'images','$[*] ? (@.type() != "string")')
    AND jsonb_typeof(details->'history')='array'
    AND jsonb_typeof(details->'approved')='boolean'
    AND jsonb_typeof(details->'views')='number'
    AND details ?& ARRAY['notes','images','history','approved','views']
);
CREATE FUNCTION product_document(product products) RETURNS JSONB LANGUAGE sql IMMUTABLE STRICT
SET search_path=public,pg_temp AS $$
    SELECT product.details || jsonb_build_object(
        'id',product.id,'section',product.section,'retailer',product.retailer,
        'url',product.url,'price',product.price,'status',product.status,
        'title',product.title,'description',product.description,'image',product.image,'components',product.components)
$$;
CREATE OR REPLACE FUNCTION capture_product_override() RETURNS trigger LANGUAGE plpgsql
SET search_path=public,pg_temp AS $$
DECLARE
    old_product JSONB := product_document(OLD);
    edited_product JSONB := product_document(NEW);
    edit JSONB;
    changes JSONB;
    source TEXT;
    edited_at TEXT;
BEGIN
    IF current_user='flagship_directus' THEN source := 'directus';
    ELSIF current_setting('flagship.override_source',true)='discord' THEN source := 'discord';
    ELSE RETURN NEW;
    END IF;
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

-- No repeated variant identifier inside JSON. PostgreSQL validates JSON itself.
ALTER TABLE variants ALTER COLUMN payload TYPE JSONB USING payload::jsonb-'id';
ALTER TABLE variants ADD CONSTRAINT variants_payload_valid CHECK (jsonb_typeof(payload)='object' AND NOT payload ? 'id');
ALTER TABLE app_state ALTER COLUMN payload TYPE JSONB USING payload::jsonb;

-- Cache metadata and content are explicit fields, rather than an opaque copy.
ALTER TABLE product_info_cache ADD COLUMN fetched_at TIMESTAMPTZ;
ALTER TABLE product_info_cache ADD COLUMN description TEXT;
ALTER TABLE product_info_cache ADD COLUMN images JSONB;
UPDATE product_info_cache SET fetched_at=(payload::jsonb->>'timestamp')::timestamptz,
    description=payload::jsonb->>'description',images=payload::jsonb->'images';
ALTER TABLE product_info_cache ALTER COLUMN expires_at TYPE TIMESTAMPTZ USING expires_at::timestamptz;
ALTER TABLE product_info_cache ALTER COLUMN fetched_at SET NOT NULL;
ALTER TABLE product_info_cache ALTER COLUMN description SET NOT NULL;
ALTER TABLE product_info_cache ALTER COLUMN images SET NOT NULL;
ALTER TABLE product_info_cache ADD CONSTRAINT cache_images_valid CHECK (jsonb_typeof(images)='array' AND NOT jsonb_path_exists(images,'$[*] ? (@.type() != "string")'));
ALTER TABLE product_info_cache DROP COLUMN payload;

-- Reviews belong to the error they describe. Scanner updates never overwrite
-- these staff-owned columns, so concurrent Directus reviews are preserved.
ALTER TABLE scrape_errors ADD COLUMN reviewed_at TIMESTAMPTZ;
ALTER TABLE scrape_errors ADD COLUMN review_note TEXT;
UPDATE scrape_errors SET reviewed_at=(payload::jsonb->>'reviewed_at')::timestamptz;
UPDATE scrape_errors error SET reviewed=review.reviewed,review_note=review.note,
    reviewed_at=CASE WHEN review.reviewed THEN coalesce(error.reviewed_at,now()) ELSE NULL END
FROM scrape_error_reviews review WHERE review.error_key=error.key;
DROP TABLE scrape_error_reviews;
ALTER TABLE scrape_errors ALTER COLUMN last_seen TYPE TIMESTAMPTZ USING last_seen::timestamptz;
ALTER TABLE scrape_errors ALTER COLUMN payload TYPE JSONB USING payload::jsonb-ARRAY['key','retailer','active','reviewed','reviewed_at','last_seen'];
ALTER TABLE scrape_errors ADD CONSTRAINT scrape_errors_payload_valid CHECK (jsonb_typeof(payload)='object' AND NOT payload ?| ARRAY['key','retailer','active','reviewed','reviewed_at','last_seen']);
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
