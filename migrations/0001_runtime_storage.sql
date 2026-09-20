CREATE TABLE products (
    id TEXT PRIMARY KEY NOT NULL,
    section TEXT NOT NULL,
    retailer TEXT NOT NULL,
    url TEXT NOT NULL,
    price INTEGER NOT NULL,
    status TEXT NOT NULL,
    removed INTEGER NOT NULL DEFAULT 0 CHECK (removed IN (0, 1)),
    payload TEXT NOT NULL
);

CREATE INDEX products_section_idx ON products(section) WHERE removed = 0;
CREATE INDEX products_retailer_idx ON products(retailer) WHERE removed = 0;
CREATE INDEX products_url_idx ON products(url);
CREATE INDEX products_price_idx ON products(price) WHERE removed = 0;

CREATE TABLE variants (
    id TEXT PRIMARY KEY NOT NULL,
    payload TEXT NOT NULL
);

CREATE TABLE product_info_cache (
    url TEXT PRIMARY KEY NOT NULL,
    expires_at TEXT NOT NULL,
    payload TEXT NOT NULL
);

CREATE INDEX product_info_cache_expiry_idx ON product_info_cache(expires_at);

CREATE TABLE app_state (
    key TEXT PRIMARY KEY NOT NULL,
    payload TEXT NOT NULL
);
