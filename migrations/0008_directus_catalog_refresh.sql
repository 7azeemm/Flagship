-- Update capture already refreshes the live catalog. Directus create/delete
-- operations must also invalidate it; scanner bulk upserts maintain their own
-- live storage and skip this counter work.
CREATE FUNCTION capture_product_structure() RETURNS trigger LANGUAGE plpgsql
SET search_path=public,pg_temp AS $$
BEGIN
    IF current_user='flagship_directus' THEN
        UPDATE admin_revision SET revision=revision+1 WHERE id=1;
    END IF;
    RETURN NULL;
END $$;
CREATE TRIGGER product_structure_capture AFTER INSERT OR DELETE ON products
    FOR EACH STATEMENT EXECUTE FUNCTION capture_product_structure();
