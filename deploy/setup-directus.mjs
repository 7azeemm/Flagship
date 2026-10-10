import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { parseEnv } from 'node:util';

// Run from any directory. Environment overrides the shared root configuration.
try {
  const text = await readFile(fileURLToPath(new URL('../.env', import.meta.url)), 'utf8');
  for (const [key, value] of Object.entries(parseEnv(text))) {
    if (process.env[key] === undefined) process.env[key] = value;
  }
} catch (error) { if (error.code !== 'ENOENT') throw error; }

const base = process.env.DIRECTUS_SETUP_URL || `http://127.0.0.1:${process.env.DIRECTUS_PORT || 8055}`;
const token = process.env.DIRECTUS_ADMIN_TOKEN;
if (!token || token.startsWith('replace-me')) throw new Error('Set DIRECTUS_ADMIN_TOKEN in .env or the environment first.');
const request = async (route, method = 'GET', body) => {
  const response = await fetch(`${base}${route}`, {
    method,
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(15000),
  });
  const result = await response.json();
  if (!response.ok) throw new Error(`${method} ${route}: ${result.errors?.map(e => e.message).join('; ') || response.status}`);
  return result.data;
};

const collections = {
  products: { icon: 'inventory_2', display_template: '{{title}} — {{retailer}}', note: 'Manual edits are saved by URL in Product Overrides. Edit ordinary columns for title, description, image, components, and price. Details JSON holds notes, images, approval, and history.' },
  product_overrides: { icon: 'edit_note', display_template: '{{product_url}} — {{timestamp}}', note: 'URL-based edit history, independent of catalog IDs. Run --apply-overrides after rebuilding the catalog to replay these changes.' },
  scrape_errors: { icon: 'bug_report', display_template: '{{retailer}} — {{key}}', note: 'Scraper diagnostics with staff review status and notes on the same record.' },
  catalog_activity: { icon: 'history', display_template: '{{title}}', note: 'Committed catalog changes.' },
};

const groups = {
  catalog: { label: 'Catalog', icon: 'inventory_2', tables: ['products','variants','catalog_views','product_overrides'] },
  accounts: { label: 'Accounts', icon: 'people', tables: ['users','auth_identities','sessions','account_tokens','pending_registrations','account_notifications'] },
  alerts: { label: 'Alerts', icon: 'notifications', tables: ['alerts','alert_events','alert_deliveries'] },
  community: { label: 'Community', icon: 'favorite', tables: ['wishlist_items','feature_votes'] },
  scraping: { label: 'Scraping', icon: 'bug_report', tables: ['scrape_errors','product_info_cache'] },
  history: { label: 'History', icon: 'history', tables: ['catalog_activity'] },
  system: { label: 'System', icon: 'settings', tables: ['admin_revision','_sqlx_migrations'] },
};
const knownCollections = await request('/collections');
for (const [key, group] of Object.entries(groups)) {
  const collection = 'flagship_' + key;
  const meta = { icon: group.icon, hidden: false, collection, translations: [{ language: 'en-US', translation: group.label }], collapse: 'open', sort: Object.keys(groups).indexOf(key) };
  if (knownCollections.some(item => item.collection === collection)) await request('/collections/' + collection, 'PATCH', { meta });
  else await request('/collections', 'POST', { collection, schema: null, meta });
}
const tables = (await request('/collections')).filter(collection => collection.schema);
for (const { collection } of tables.filter(table => !table.collection.startsWith('directus_'))) {
  const group = Object.entries(groups).find(([, group]) => group.tables.includes(collection));
  await request(`/collections/${collection}`, 'PATCH', { meta: {
    ...collections[collection], hidden: false, group: group ? 'flagship_' + group[0] : null,
    sort: group ? group[1].tables.indexOf(collection) : 0,
    ...(collection === 'wishlist_items' ? { translations: [{ language: 'en-US', translation: 'Wishlist items' }] } : {}),
  } });
  const fields = await request(`/fields/${collection}`);
  for (const field of fields) {
    const json = field.type === 'json';
    const jsonText = ['payload','rule','last_match'].includes(field.field);
    const multiline = ['description', 'note', 'review_note'].includes(field.field);
    const date = ['timestamp','dateTime','date','time'].includes(field.type);
    const boolean = field.type === 'boolean';
    await request(`/fields/${collection}/${field.field}`, 'PATCH', {
      meta: {
        readonly: field.schema?.is_generated === true || field.schema?.has_auto_increment === true,
        hidden: false,
        interface: json || jsonText ? 'input-code' : boolean ? 'boolean' : date ? 'datetime' : multiline ? 'input-multiline' : 'input',
        ...(json ? { options: { language: 'json' }, special: ['cast-json'] } : {}),
        ...(jsonText ? { options: { language: 'json' } } : {}),
        ...(boolean ? { special: ['cast-boolean'] } : {}),
      },
    });
  }
}

const findOrCreate = async (route, name, values) => {
  const existing = await request(`${route}?filter[name][_eq]=${encodeURIComponent(name)}&limit=1`);
  return existing[0] || request(route, 'POST', { name, ...values });
};
const policy = await findOrCreate('/policies', 'Flagship catalog editor', { icon: 'edit_note', app_access: true, admin_access: false });
const role = await findOrCreate('/roles', 'Catalog editor', { icon: 'edit_note', description: 'Read, create, update, and delete records in every database table.' });
await request(`/roles/${role.id}`, 'PATCH', { description: 'Read, create, update, and delete records in every database table.' });
const links = await request(`/access?filter[role][_eq]=${role.id}&filter[policy][_eq]=${policy.id}`);
if (!links.length) await request('/access', 'POST', { role: role.id, policy: policy.id });

for (const { collection } of tables) {
  for (const action of ['read', 'create', 'update', 'delete']) {
    const rules = await request(`/permissions?filter[policy][_eq]=${policy.id}&filter[collection][_eq]=${collection}&filter[action][_eq]=${action}`);
    const values = { policy: policy.id, collection, action, permissions: {}, validation: {}, presets: {}, fields: ['*'] };
    await request(rules[0] ? `/permissions/${rules[0].id}` : '/permissions', rules[0] ? 'PATCH' : 'POST', values);
  }
}
const appCount = tables.filter(table => !table.collection.startsWith('directus_')).length;
console.log(`Directus configured: ${appCount} Flagship tables (including migrations), ${tables.length - appCount} Directus system tables. Full read/write access.`);
