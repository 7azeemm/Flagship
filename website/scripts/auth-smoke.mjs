import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { parseEnv } from 'node:util';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { createServer } from 'node:net';
import { setTimeout as delay } from 'node:timers/promises';
import pg from '../../deploy/directus/node_modules/pg/lib/index.js';
import argon2 from '../../deploy/directus/node_modules/argon2/argon2.cjs';
import { localWebsiteOrigin } from '../../deploy/local-origin.mjs';

const origin = 'http://127.0.0.1:5173';
assert.equal(localWebsiteOrigin({ APP_PUBLIC_URL: origin }, true, ''), origin);
assert.equal(localWebsiteOrigin({ APP_PUBLIC_URL: 'https://flagship.example' }, true, ''), origin);
assert.equal(localWebsiteOrigin({}, false, 'http://127.0.0.1:3000'), 'http://127.0.0.1:3000');
assert.throws(() => localWebsiteOrigin({ LOCAL_APP_PUBLIC_URL: 'http://example.com:5173' }, true, ''), /loopback/);
const root = fileURLToPath(new URL('../../', import.meta.url));
const settings = { ...parseEnv(readFileSync(join(root, '.env'), 'utf8')), ...process.env };
const schema = `flagship_auth_test_${randomUUID().replaceAll('-', '')}`;
const owner = new pg.Client({ connectionString: settings.DATABASE_URL });
await owner.connect();
const url = new URL(settings.DATABASE_URL);
url.searchParams.set('options', `-c search_path=${schema}`);
const socket = createServer();
socket.listen(0, '127.0.0.1');
await once(socket, 'listening');
const port = socket.address().port;
await new Promise(resolve => socket.close(resolve));
const env = {
  ...process.env, DATABASE_URL: url.href, BIND_ADDRESS: `127.0.0.1:${port}`,
  FLAGSHIP_WEB_DIR: join(root, 'website', 'dist'), FLAGSHIP_LOCAL_DEV: 'true',
  APP_PUBLIC_URL: origin, TRUSTED_PROXY_IPS: '', ENABLE_SCANNING: 'false',
  GOOGLE_CLIENT_ID: 'fixture-client', GOOGLE_CLIENT_SECRET: 'fixture-secret',
  GOOGLE_REDIRECT_URL: origin + '/api/auth/google/callback',
  SMTP_HOST: 'smtp.example.invalid', SMTP_USERNAME: 'fixture', SMTP_PASSWORD: 'fixture',
  SMTP_FROM: 'fixture@example.invalid', SMTP_PORT: '587', RUST_LOG: 'warn',
};
const binary = settings.FLAGSHIP_BIN || join(root, 'target', 'debug', 'Flagship.exe');
let child;
let ended;
try {
  await owner.query(`CREATE SCHEMA "${schema}"`);
  await owner.query(`SET search_path TO "${schema}"`);
  const files = readdirSync(join(root,'migrations')).filter(name => /^\d+_.+\.sql$/.test(name)).sort();
  for (const file of files) await owner.query(readFileSync(join(root,'migrations',file),'utf8')
    .replaceAll('SET search_path=public,pg_temp',`SET search_path="${schema}",pg_temp`));
  await owner.query('CREATE TABLE _sqlx_migrations (LIKE public._sqlx_migrations INCLUDING ALL)');
  for (const file of files) await owner.query('INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES($1,$2,true,$3,0)',
    [Number(file.split('_')[0]),file,createHash('sha384').update(readFileSync(join(root,'migrations',file))).digest()]);
  const password = 'Fixture password 123!';
  const hash = await argon2.hash(password, { type: argon2.argon2id });
  await owner.query('INSERT INTO users(id,email,password_hash,email_verified_at,created_at,updated_at) VALUES($1,$2,$3,$4,$4,$4)',
    ['fixture-user', 'auth-fixture@example.invalid', hash, new Date().toISOString()]);
  child = spawn(binary, ['--serve-only'], { cwd: root, env, windowsHide: true, stdio: 'inherit' });
  let spawnError;
  child.on('error', error => { spawnError = error; });
  ended = once(child, 'exit');
  const request = (path, options = {}) => fetch(`http://127.0.0.1:${port}${path}`, { ...options, signal: AbortSignal.timeout(10000) });
  let ready = false;
  for (let i = 0; i < 100 && !ready; i++) {
    if (spawnError) throw spawnError;
    assert.equal(child.exitCode, null, 'Backend must stay running');
    try { ready = (await request('/health/ready')).status === 200; } catch { /* Still starting. */ }
    if (!ready) await delay(100);
  }
  assert(ready);
  const credentials = JSON.stringify({ email: 'auth-fixture@example.invalid', password });
  const loginOptions = { method: 'POST', headers: { Origin: origin, 'Content-Type': 'application/json', 'Sec-Fetch-Site': 'same-origin' }, body: credentials };
  const foreign = await request('/api/auth/login', { ...loginOptions, headers: { ...loginOptions.headers, Origin: 'https://foreign.example' } });
  assert.equal(foreign.status, 403);
  const login = await request('/api/auth/login', loginOptions);
  assert.equal(login.status, 200, await login.clone().text());
  const cookies = login.headers.getSetCookie();
  assert(cookies.some(cookie => cookie.startsWith('flagship_session=')));
  assert(cookies.every(cookie => !cookie.includes('; Secure')), 'HTTP local cookies must be usable by browsers');
  const cookie = cookies.map(value => value.split(';')[0]).join('; ');
  const csrf = cookies.find(value => value.startsWith('flagship_csrf=')).split(';')[0].split('=')[1];
  const session = await request('/api/auth/session', { headers: { Cookie: cookie } });
  assert.equal((await session.json()).user.email, 'auth-fixture@example.invalid');
  for(const table of ['oauth_attempts','app_state','retailers']) assert.equal((await owner.query('SELECT to_regclass($1) AS name',[table])).rows[0].name,null);
  const oauth = await request('/api/auth/google/start', { redirect: 'manual' });
  assert.equal(oauth.status, 307);
  assert.equal(new URL(oauth.headers.get('location')).searchParams.get('redirect_uri'), origin + '/api/auth/google/callback');
  assert(!oauth.headers.get('set-cookie').includes('; Secure'));
  const cancelledState = new URL(oauth.headers.get('location')).searchParams.get('state');
  const oauthCookie = oauth.headers.getSetCookie()[0].split(';')[0];
  const cancelled = await request('/api/auth/google/callback?error=access_denied&state='+encodeURIComponent(cancelledState),{redirect:'manual',headers:{Cookie:oauthCookie}});
  assert.equal(cancelled.headers.get('location'),'/login?error=google_cancelled');
  const replay = await request('/api/auth/google/callback?code=fixture&state='+encodeURIComponent(cancelledState),{redirect:'manual',headers:{Cookie:oauthCookie}});
  assert.equal(replay.headers.get('location'),'/login?error=google_failed','Cancelled attempts must be consumed');
  const unfinished=await request('/api/auth/google/start',{redirect:'manual'});
  const unfinishedState=new URL(unfinished.headers.get('location')).searchParams.get('state');
  const unfinishedCookie=unfinished.headers.getSetCookie()[0].split(';')[0];

  const features = await request('/api/future-plans');
  const guestCookie = features.headers.getSetCookie()[0].split(';')[0];
  const voteOptions = { method: 'POST', headers: { Origin: origin, 'Content-Type': 'application/json', Cookie: guestCookie }, body: '{"vote":1}' };
  assert.equal((await request('/api/future-plans/pc-builder/hype', voteOptions)).status, 200);
  const accountVote = await request('/api/future-plans/pc-builder/hype', { ...voteOptions, headers: { ...voteOptions.headers, Cookie: cookie + '; ' + guestCookie, 'X-CSRF-Token': csrf } });
  assert.equal(accountVote.status, 200, await accountVote.clone().text());
  const votes = (await owner.query('SELECT user_id,browser_hash FROM feature_votes')).rows;
  assert.deepEqual(votes, [{ user_id: 'fixture-user', browser_hash: null }], 'Guest votes must merge without double-counting');
  const withoutCsrf = await request('/api/auth/logout', { method: 'POST', headers: { Origin: origin, Cookie: cookie } });
  assert.equal(withoutCsrf.status, 403);
  const logout = await request('/api/auth/logout', { method: 'POST', headers: { Origin: origin, Cookie: cookie, 'X-CSRF-Token': csrf } });
  assert.equal(logout.status, 204);
  // Exercise both migrated token ownership paths through the real endpoints.
  const digest = raw => createHash('sha256').update(raw).digest('base64url');
  const postToken = (path,body) => request(path,{method:'POST',headers:{Origin:origin,'Content-Type':'application/json'},body:JSON.stringify(body)});
  const registrationRaw = 'registration-fixture-'.padEnd(43,'x');
  const resendRaw = 'resend-fixture-'.padEnd(43,'x');
  await owner.query("INSERT INTO pending_registrations(id,email,password_hash,expires_at) VALUES('pending','new-account@example.invalid',$1,'2999-01-01T00:00:00Z')",[hash]);
  for(const raw of [registrationRaw,resendRaw]) await owner.query("INSERT INTO account_tokens(id,registration_id,purpose,token_hash,expires_at) VALUES($1,'pending','verify_registration',$2,'2999-01-01')",[digest(raw),digest(raw)]);
  const registrationVerified=await postToken('/api/auth/email/verify',{token:resendRaw});
  assert.equal(registrationVerified.status,200,await registrationVerified.clone().text());
  assert.equal((await owner.query("SELECT count(*)::int AS n FROM account_tokens WHERE registration_id='pending'")).rows[0].n,0);
  assert.equal((await owner.query("SELECT count(*)::int AS n FROM users WHERE email='new-account@example.invalid' AND email_verified_at IS NOT NULL")).rows[0].n,1);
  assert.equal((await postToken('/api/auth/email/verify',{token:registrationRaw})).status,400,'Sibling registration links must be invalidated');
  const verifyRaw='legacy-verify-fixture-'.padEnd(43,'x');
  await owner.query("INSERT INTO users(id,email,password_hash,created_at,updated_at) VALUES('token-user','token-user@example.invalid',$1,$2,$2)",[hash,new Date().toISOString()]);
  await owner.query("INSERT INTO account_tokens(id,user_id,purpose,token_hash,expires_at) VALUES('verify','token-user','verify_email',$1,'2999-01-01')",[digest(verifyRaw)]);
  const verified=await postToken('/api/auth/email/verify',{token:verifyRaw});
  assert.equal(verified.status,200,await verified.clone().text());
  const resetRaw='reset-fixture-'.padEnd(43,'x');
  for(const raw of [resetRaw,resetRaw+'sibling']) await owner.query("INSERT INTO account_tokens(id,user_id,purpose,token_hash,expires_at) VALUES($1,'token-user','reset_password',$2,'2999-01-01')",[digest(raw),digest(raw)]);
  const resetPassword='Changed fixture password 456!';
  const reset=await postToken('/api/auth/password/reset',{token:resetRaw,password:resetPassword});
  assert.equal(reset.status,200,await reset.clone().text());
  assert.equal((await owner.query("SELECT count(*)::int AS n FROM account_tokens WHERE user_id='token-user'")).rows[0].n,0);
  assert(await argon2.verify((await owner.query("SELECT password_hash FROM users WHERE id='token-user'")).rows[0].password_hash,resetPassword));
  await owner.query('DELETE FROM account_notifications'); // Fixture mail is never sent.
  child.kill(); await ended;
  const productionOrigin = 'https://flagship.example';
  const productionEnv = {...env,FLAGSHIP_LOCAL_DEV:'false',APP_PUBLIC_URL:productionOrigin,GOOGLE_REDIRECT_URL:productionOrigin+'/api/auth/google/callback'};
  child = spawn(binary,['--serve-only'],{cwd:root,env:productionEnv,windowsHide:true,stdio:'inherit'});
  ended = once(child,'exit');
  ready=false;
  for(let i=0;i<100&&!ready;i++) {
    assert.equal(child.exitCode,null);
    try {ready=(await request('/health/ready')).status===200;} catch {}
    if(!ready)await delay(100);
  }
  assert(ready);
  const afterRestart=await request('/api/auth/google/callback?code=fixture&state='+encodeURIComponent(unfinishedState),{redirect:'manual',headers:{Cookie:unfinishedCookie}});
  assert.equal(afterRestart.headers.get('location'),'/login?error=google_failed','In-memory attempts must not survive restart');
  const productionLogin=await request('/api/auth/login',{...loginOptions,headers:{...loginOptions.headers,Origin:productionOrigin}});
  assert.equal(productionLogin.status,200);
  assert(productionLogin.headers.getSetCookie().every(cookie=>cookie.includes('; Secure')),'Production cookies must remain Secure');
  assert((await request('/api/auth/google/start',{redirect:'manual'})).headers.get('set-cookie').includes('; Secure'));
  assert((await request('/api/future-plans')).headers.get('set-cookie').includes('; Secure'));
  child.kill();await ended;
  const invalidHttp = spawnSync(binary,['--serve-only'],{cwd:root,env:{...env,FLAGSHIP_LOCAL_DEV:'false'},windowsHide:true,stdio:'ignore'});
  if(invalidHttp.error)throw invalidHttp.error;
  assert.notEqual(invalidHttp.status,0,'Unmarked HTTP deployment must be rejected');
  console.log('Authentication smoke passed: password login/session, exact Google callback, in-memory OAuth cancellation/replay/restart, local cookies, production Secure cookies, HTTP deployment rejection, origin/CSRF rejection, guest vote merge, registration/verification/reset tokens, and logout.');
} finally {
  if (child && child.exitCode === null) { child.kill(); await ended; }
  assert(/^flagship_auth_test_[a-f0-9]{32}$/.test(schema));
  await owner.query(`DROP SCHEMA IF EXISTS "${schema}" CASCADE`);
  await owner.end();
}
