import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { parseEnv } from 'node:util';
import pg from '../../deploy/directus/node_modules/pg/lib/index.js';

// Isolate fixtures in a temporary schema. No existing catalog rows are changed.
const root = fileURLToPath(new URL('../../', import.meta.url));
const settings = { ...parseEnv(readFileSync(join(root, '.env'), 'utf8')), ...process.env };
const schema = `flagship_override_test_${randomUUID().replaceAll('-', '')}`;
const client = new pg.Client({ connectionString: settings.DATABASE_URL });
const directusUrl = new URL(settings.DATABASE_URL);
directusUrl.username = 'flagship_directus';
directusUrl.password = settings.DIRECTUS_DB_PASSWORD;
const directus = new pg.Client({ connectionString: directusUrl.href });
const fixtureUrl = 'https://fixture.invalid/product';
const legacyTimestamp = '2025-01-01T00:00:00.123456789Z';
const discordTimestamp = '2027-01-01T00:00:00.123456789Z';
const base = {
  id: 'original-id', url: fixtureUrl, title: 'Raw title', retailer: 'fixture', section: 'GPU',
  description: '', image: '', images: [], status: 'InStock', price: 100, original_price: null,
  components: { CPU: 'Original CPU', Memory: '16 GB' }, notes: [], approved: false, views: 0,
  history: [], added_at: '2026-01-01T00:00:00Z', updated_at: null, removed_at: null,
};
let normalized = false;
const details = product => Object.fromEntries(Object.entries(product).filter(([key]) => !["id","url","section","retailer","price","status","title","description","image","components"].includes(key)));
const insertProduct = product => normalized ? client.query(
  "INSERT INTO products(id,url,section,retailer,price,status,title,description,image,components,details) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
  [product.id,product.url,product.section,product.retailer,product.price,product.status,product.title,product.description,product.image,product.components,details(product)],
) : client.query(
  'INSERT INTO products(id,url,section,retailer,price,status,payload) VALUES($1,$2,$3,$4,$5,$6,$7)',
  [product.id, product.url, product.section, product.retailer, product.price, product.status, JSON.stringify(product)],
);
const count = async () => (await client.query('SELECT count(*)::int AS count FROM product_overrides')).rows[0].count;
const migrationFiles = readdirSync(join(root, 'migrations')).filter(name => /^\d+_.+\.sql$/.test(name)).sort();
const cliUrl = new URL(settings.DATABASE_URL);
cliUrl.searchParams.set('options', `-c search_path=${schema}`);
const runReplay = (expectSuccess = true) => {
  const result = spawnSync(settings.FLAGSHIP_BIN || join(root, 'target', 'debug', process.platform === 'win32' ? 'Flagship.exe' : 'Flagship'), ['--apply-overrides'], {
    cwd: root, windowsHide: true, stdio: expectSuccess ? 'inherit' : 'ignore',
    env: { ...process.env, DATABASE_URL: cliUrl.href, ENABLE_SCANNING: 'false', RUST_LOG: 'info' },
  });
  if (result.error) throw result.error;
  if (expectSuccess) assert.equal(result.status, 0, 'Replay command must succeed');
  else assert.notEqual(result.status, 0, 'Invalid replay must fail');
};
let created = false;
let directusConnected = false;
await client.connect();
try {
  await client.query(`CREATE SCHEMA "${schema}"`);
  created = true;
  await client.query(`SET search_path TO "${schema}"`);
  for (const filename of migrationFiles) {
    const sql = readFileSync(join(root, 'migrations', filename), 'utf8')
      .replaceAll('SET search_path=public,pg_temp', `SET search_path="${schema}",pg_temp`);
    await client.query(sql);
    if (filename.startsWith('0006_')) normalized = true;
    if (filename.startsWith('0001_')) {
      await insertProduct(base);
      await client.query("INSERT INTO users(id,email,created_at,updated_at) VALUES('fixture-user','schema-fixture@example.invalid','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')");
      await client.query("INSERT INTO alerts(id,user_id,rule,state,cursor,created_at,expires_at) VALUES('live-watch','fixture-user',$1,'pending',1,'2026-01-01T00:00:00Z','2999-01-01T00:00:00Z'),('closed-watch','fixture-user',$1,'delivered',3,'2026-01-01T00:00:00Z','2999-01-01T00:00:00Z')",[JSON.stringify({target:{mode:'product',product_id:base.id},target_price:50,restock:false,recurring_low:true,retailers:[],statuses:['InStock']})]);
      for(let sequence=1;sequence<=3;sequence++) await client.query('INSERT INTO alert_events(event_key,section,product_id,observed_at,payload) VALUES($1,$2,$3,$4,$5)',['event-'+sequence,'GPU',base.id,'2026-01-01T00:00:00Z',JSON.stringify({product:base,previous:null})]);
      await client.query("INSERT INTO alert_deliveries(alert_id,payload,next_attempt_at,lease_token,lease_until,sent_at) VALUES('live-watch',$1,'2026-01-01T00:00:00Z','sending','2999-01-01T00:00:00Z',NULL),('closed-watch',$1,'2026-01-01T00:00:00Z',NULL,NULL,'2026-01-01T00:00:00Z')",[JSON.stringify([base])]);
      await client.query("INSERT INTO pending_registrations(id,email,password_hash,expires_at) VALUES('pending','pending@example.invalid','fixture-hash','2999-01-01T00:00:00Z')");
      await client.query("INSERT INTO registration_tokens(token_hash,registration_id) VALUES('registration-token','pending'),('resend-token','pending')");
      await client.query("INSERT INTO email_tokens(id,user_id,purpose,token_hash,expires_at,created_at) VALUES('reset-token','fixture-user','reset_password','reset-hash','2999-01-01T00:00:00Z','2026-01-01T00:00:00Z')");
      await client.query("INSERT INTO feature_hype(feature_id,user_id) VALUES('pc-builder','fixture-user')");
      await client.query("INSERT INTO guest_feature_hype(feature_id,browser_hash) VALUES('pc-builder','browser'),('marketplace','browser')");
      await client.query('INSERT INTO product_info_cache(url,expires_at,payload) VALUES($1,$2,$3)', [fixtureUrl,'2999-01-01T00:00:00Z',JSON.stringify({timestamp:'2026-01-01T00:00:00Z',description:'Cached text',images:['https://fixture.invalid/image']})]);
      await client.query('INSERT INTO variants(id,payload) VALUES($1,$2)', ['variant',JSON.stringify({id:'variant',section:'GPU',fields:{},representative_product_id:null,product_ids:[],current_snapshot:{timestamp:'2026-01-01T00:00:00Z',lowest_price:null,lowest_price_product_id:null,offer_count:0,in_stock_count:0,retailer_count:0},history:[],views:0,active:false,created_at:'2026-01-01T00:00:00Z',updated_at:'2026-01-01T00:00:00Z',last_seen_at:'2026-01-01T00:00:00Z',lowest_price_updated_at:null})]);
      await client.query('INSERT INTO scrape_errors(key,retailer,active,reviewed,last_seen,payload) VALUES($1,$2,true,false,$3,$4)', ['error','fixture','2026-01-01T00:00:00Z',JSON.stringify({key:'error',retailer:'fixture',active:true,reviewed:false,reviewed_at:null,last_seen:'2026-01-01T00:00:00Z',message:'Fixture failure'})]);
      await client.query("INSERT INTO scrape_error_reviews(error_key,reviewed,note) VALUES('error',true,'Reviewed fixture')");
      await client.query('INSERT INTO product_curation(product_id,title) VALUES($1,$2)', [base.id, 'Curated title']);
      await client.query('INSERT INTO app_state(key,payload) VALUES($1,$2)', ['product_overrides', JSON.stringify({
        [fixtureUrl]: [{ timestamp: legacyTimestamp, changes: [{ field: 'title', old_value: 'Raw title', new_value: 'Legacy title' }] }],
      })]);
    }
  }
  assert.deepEqual((await client.query('SELECT sequence::int AS sequence FROM alert_events ORDER BY sequence')).rows,[{sequence:2},{sequence:3}],'Migration must retain unprocessed observations');
  assert.equal((await client.query('SELECT alert_event_cursor()::int AS cursor')).rows[0].cursor,3,'Cleanup watermark must persist');
  assert.deepEqual((await client.query('SELECT alert_id,lease_token FROM alert_deliveries')).rows,[{alert_id:'live-watch',lease_token:'sending'}],'Pending mail and sending leases must survive migration');
  for(const table of ['app_state','retailers','oauth_attempts']) assert.equal((await client.query('SELECT to_regclass($1) AS name',[table])).rows[0].name,null);
  assert.equal(await count(), 2, 'Both legacy systems must migrate');
  assert.equal((await client.query('SELECT count(*)::int AS count FROM account_tokens')).rows[0].count,3,'All previously delivered token links must survive');
  assert.equal((await client.query('SELECT count(*)::int AS count FROM feature_votes')).rows[0].count,3,'Guest and account votes must survive');
  await assert.rejects(client.query("INSERT INTO feature_votes(feature_id,user_id,browser_hash) VALUES('pc-builder','fixture-user','invalid')"), /check constraint/);
  await assert.rejects(client.query("INSERT INTO account_tokens(id,user_id,purpose,token_hash,expires_at) VALUES('bad','fixture-user','verify_registration','bad','2999-01-01')"), /check constraint/);
  assert.equal((await client.query('SELECT review_note FROM scrape_errors')).rows[0].review_note,'Reviewed fixture');
  assert.equal((await client.query('SELECT reviewed FROM scrape_errors')).rows[0].reviewed,true);
  assert.equal((await client.query("SELECT payload ? 'reviewed' AS duplicated FROM scrape_errors")).rows[0].duplicated,false);
  assert.equal((await client.query("SELECT payload ? 'id' AS duplicated FROM variants")).rows[0].duplicated,false);
  const cached=(await client.query('SELECT description,images FROM product_info_cache')).rows[0];
  assert.deepEqual(cached,{description:'Cached text',images:['https://fixture.invalid/image']});
  assert.equal((await client.query('SELECT product_document(products)::text AS payload FROM products')).rows[0].payload.includes('Curated title'),true);
  await assert.rejects(client.query("UPDATE products SET details=details||'{\"title\":\"duplicate\"}'::jsonb"), /check constraint/);
  assert.equal((await client.query("SELECT timestamp FROM product_overrides WHERE source='legacy'")).rows[0].timestamp, legacyTimestamp);
  assert.equal((await client.query("SELECT to_regclass('app_state') AS name")).rows[0].name, null);
  assert.equal((await client.query("SELECT to_regclass('product_curation') AS name")).rows[0].name, null);
  assert.equal((await client.query('SELECT title FROM products')).rows[0].title, 'Curated title');
  await client.query(`GRANT USAGE ON SCHEMA "${schema}" TO flagship_directus`);
  await client.query(`GRANT SELECT,INSERT,UPDATE,DELETE,REFERENCES ON ALL TABLES IN SCHEMA "${schema}" TO flagship_directus`);
  await client.query(`GRANT USAGE,SELECT,UPDATE ON ALL SEQUENCES IN SCHEMA "${schema}" TO flagship_directus`);
  await directus.connect();
  directusConnected = true;
  await directus.query(`SET search_path TO "${schema}"`);

  await directus.query('UPDATE products SET price=80,title=$1 WHERE id=$2', ['Directus title',base.id]);
  assert.equal(await count(), 3);
  const edited = JSON.parse((await client.query('SELECT product_document(products)::text AS payload FROM products')).rows[0].payload);
  assert.equal(edited.price, 80, 'The document must use canonical columns');
  assert.equal(edited.title, 'Directus title');
  assert.equal(edited.history.length, 1);
  assert.equal((await client.query("SELECT source FROM product_overrides ORDER BY id DESC LIMIT 1")).rows[0].source, 'directus');
  await assert.rejects(directus.query('UPDATE products SET id=$1 WHERE id=$2', ['changed-id', base.id]), /Product ID cannot be modified/);
  assert.equal(await count(), 3, 'Rejected Directus edits must not create history');

  await directus.query("UPDATE scrape_errors SET reviewed=false,review_note='Needs investigation' WHERE key='error'");
  let review=(await client.query('SELECT reviewed,reviewed_at,review_note FROM scrape_errors')).rows[0];
  assert.deepEqual(review,{reviewed:false,reviewed_at:null,review_note:'Needs investigation'});
  await directus.query("UPDATE scrape_errors SET reviewed=true WHERE key='error'");
  assert((await client.query('SELECT reviewed_at FROM scrape_errors')).rows[0].reviewed_at instanceof Date);

  const revisionBefore = (await client.query('SELECT revision FROM admin_revision WHERE id=1')).rows[0].revision;
  await directus.query("INSERT INTO products(id,url,section,retailer,price,status,title,description,image,components,details) SELECT 'temporary',url,section,retailer,price,status,title,description,image,components,details FROM products WHERE id=$1",[base.id]);
  const revisionAfterCreate = (await client.query('SELECT revision FROM admin_revision WHERE id=1')).rows[0].revision;
  assert(BigInt(revisionAfterCreate)>BigInt(revisionBefore),'Directus creates must refresh the catalog');
  await directus.query("DELETE FROM products WHERE id='temporary'");
  assert(BigInt((await client.query('SELECT revision FROM admin_revision WHERE id=1')).rows[0].revision)>BigInt(revisionAfterCreate),'Directus deletes must refresh the catalog');

  const discordEdit = {
    timestamp: discordTimestamp,
    changes: [
      { field: 'components', old_value: structuredClone(edited.components), new_value: { CPU: 'Correct CPU', Memory: '16 GB' } },
      { field: 'notes', old_value: [], new_value: ['Staff note'] },
    ],
  };
  edited.components.CPU = 'Correct CPU';
  edited.notes = ['Staff note'];
  edited.history.push(discordEdit);
  edited.updated_at = discordTimestamp;
  await client.query('BEGIN');
  await client.query("SELECT set_config('flagship.override_source','discord',true)");
  await client.query('UPDATE products SET title=$1,components=$2,details=$3 WHERE id=$4', [edited.title,edited.components,details(edited),base.id]);
  await client.query('COMMIT');
  assert.equal(await count(), 4);
  assert.equal((await client.query("SELECT timestamp FROM product_overrides WHERE source='discord'")).rows[0].timestamp, discordTimestamp);

  // A failed product transaction must roll back its override entry too.
  edited.title = 'Rolled back title';
  edited.history.push({ timestamp: '2028-01-01T00:00:00Z', changes: [{ field: 'title', old_value: 'Directus title', new_value: edited.title }] });
  await client.query('BEGIN');
  await client.query("SELECT set_config('flagship.override_source','discord',true)");
  await client.query('UPDATE products SET title=$1,components=$2,details=$3 WHERE id=$4', [edited.title,edited.components,details(edited),base.id]);
  await client.query('ROLLBACK');
  assert.equal(await count(), 4);
  assert.equal((await client.query('SELECT title FROM products')).rows[0].title, 'Directus title');

  await client.query("UPDATE products SET price=999");
  assert.equal(await count(), 4, 'Ordinary scanner saves must not create overrides');
  await client.query('DELETE FROM products');
  assert.equal(await count(), 4, 'Deleting the catalog must preserve URL-based edits');
  for (const id of ['fresh-id-1', 'fresh-id-2']) await insertProduct({
    ...base, id, components: { CPU: 'Fresh CPU', Memory: '32 GB', Storage: '1 TB' },
  });
  assert.equal((await client.query('SELECT title FROM products LIMIT 1')).rows[0].title, 'Raw title', 'Replay must be explicit');

  // Mark the already-applied fixture migrations with their original checksums,
  // so the real command only exercises replay in this isolated schema.
  await client.query('CREATE TABLE _sqlx_migrations (LIKE public._sqlx_migrations INCLUDING ALL)');
  for (const filename of migrationFiles) await client.query(
    'INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES($1,$2,true,$3,0)',
    [Number(filename.split('_')[0]), filename, createHash('sha384').update(readFileSync(join(root, 'migrations', filename))).digest()],
  );
  if (!process.argv.includes('--skip-cli')) {
    runReplay();
    const corrected = (await client.query('SELECT id,product_document(products)::text AS payload FROM products ORDER BY id')).rows;
    for (const row of corrected) {
      const product = JSON.parse(row.payload);
      assert.equal(product.id, row.id);
      assert.equal(product.title, 'Directus title');
      assert.equal(product.price, 80);
      assert.deepEqual(product.components, { CPU: 'Correct CPU', Memory: '32 GB', Storage: '1 TB' });
      assert.deepEqual(product.notes, ['Staff note']);
      assert.equal(product.history.length, 4);
      assert.equal(product.history[0].timestamp, legacyTimestamp);
    }
    runReplay();
    assert.deepEqual((await client.query('SELECT id,product_document(products)::text AS payload FROM products ORDER BY id')).rows, corrected, 'Repeated replay must preserve history without duplicates');
    assert.equal(await count(), 4, 'Replay must not record itself as an edit');
    await client.query('INSERT INTO product_overrides(product_url,timestamp,changes) VALUES($1,$2,$3),($1,$4,$5)', [
      fixtureUrl, '2029-01-01T00:00:00Z', JSON.stringify([{ field: 'price', old_value: 80, new_value: 70 }]),
      '2030-01-01T00:00:00Z', JSON.stringify([{ field: 'price', old_value: 70, new_value: 'invalid' }]),
    ]);
    runReplay(false);
    assert.deepEqual((await client.query('SELECT id,product_document(products)::text AS payload FROM products ORDER BY id')).rows, corrected, 'An invalid edit must roll back the entire replay');
  }
  console.log(process.argv.includes('--skip-cli')
    ? 'Override database checks passed: legacy migration, Directus and Discord capture, atomic history, and catalog reset. CLI checks skipped.'
    : 'Override smoke passed: legacy migration, Directus and Discord capture, atomic history, catalog reset, fresh IDs, nested deltas, idempotent replay, and invalid-edit rollback.');
} finally {
  if (directusConnected) await directus.end();
  if (created) {
    assert(/^flagship_override_test_[a-f0-9]{32}$/.test(schema));
    await client.query(`DROP SCHEMA "${schema}" CASCADE`);
  }
  await client.end();
}
