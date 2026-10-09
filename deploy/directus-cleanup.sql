\set ON_ERROR_STOP on
-- Only obsolete metadata is removed; application tables were already migrated.
DO $$ DECLARE obsolete TEXT; BEGIN
    FOREACH obsolete IN ARRAY ARRAY['roadmap_features','product_curation','feature_hype','guest_feature_hype','email_tokens','registration_tokens','scrape_error_reviews','app_state','retailers','oauth_attempts'] LOOP
        IF to_regclass('public.'||obsolete) IS NULL THEN
            DELETE FROM directus_fields WHERE collection=obsolete;
            DELETE FROM directus_relations WHERE many_collection=obsolete OR one_collection=obsolete;
            DELETE FROM directus_permissions WHERE collection=obsolete;
            DELETE FROM directus_presets WHERE collection=obsolete;
            UPDATE directus_collections SET "group"=NULL WHERE "group"=obsolete;
            DELETE FROM directus_collections WHERE collection=obsolete;
        END IF;
    END LOOP;
    DELETE FROM directus_fields field WHERE collection IN ('products','product_info_cache','alert_deliveries')
        AND NOT EXISTS (SELECT 1 FROM information_schema.columns column_info
            WHERE column_info.table_schema='public' AND column_info.table_name=field.collection AND column_info.column_name=field.field)
        AND field.field IN ('payload','sent_at');
END $$;
