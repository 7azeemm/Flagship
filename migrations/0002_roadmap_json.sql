-- Feature definitions now live in config/roadmap.json. Keep all existing votes;
-- their feature IDs are validated by the application against that file.
ALTER TABLE feature_hype DROP CONSTRAINT IF EXISTS feature_hype_feature_id_fkey;
ALTER TABLE guest_feature_hype DROP CONSTRAINT IF EXISTS guest_feature_hype_feature_id_fkey;
DROP TABLE roadmap_features;
