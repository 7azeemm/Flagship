import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, rm } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';

const root = fileURLToPath(new URL('../../', import.meta.url));
assert(process.env.TEST_DATABASE_URL?.startsWith('postgres'), 'Set TEST_DATABASE_URL to an empty, disposable PostgreSQL database.');
const binary =
  process.env.FLAGSHIP_BIN ||
  path.join(root, 'target', 'debug', process.platform === 'win32' ? 'Flagship.exe' : 'Flagship');
const temporary = await mkdtemp(path.join(tmpdir(), 'flagship-startup-'));
const socket = createServer();
socket.listen(0, '127.0.0.1');
await once(socket, 'listening');
const port = socket.address().port;
await new Promise((resolve) => socket.close(resolve));
const env = {
  ...process.env,
  DATABASE_URL: process.env.TEST_DATABASE_URL,
  BIND_ADDRESS: `127.0.0.1:${port}`,
  FLAGSHIP_WEB_DIR: path.join(root, 'website', 'dist'),
  APP_PUBLIC_URL: 'https://flagship.example',
  SMTP_HOST: 'smtp.example.invalid',
  SMTP_PORT: '587',
  SMTP_USERNAME: 'fixture',
  SMTP_PASSWORD: 'fixture',
  SMTP_FROM: 'fixture@example.test',
  GOOGLE_CLIENT_ID: '',
  GOOGLE_CLIENT_SECRET: '',
  GOOGLE_REDIRECT_URL: '',
  DISCORD_TOKEN: '',
  ENABLE_SCANNING: 'false',
  TRUSTED_PROXY_IPS: '127.0.0.1',
  RUST_LOG: 'warn',
  PLAYWRIGHT_BROWSERS_PATH: path.join(temporary, 'no-browsers'),
};
const child = spawn(binary, ['--serve-only'], {
  cwd: root,
  env,
  windowsHide: true,
  stdio: ['ignore', 'pipe', 'pipe'],
});
let output = '';
let spawnError;
child.on('error', (error) => {
  spawnError = error;
});
for (const stream of [child.stdout, child.stderr])
  stream.on('data', (data) => {
    output = (output + data).slice(-10000);
  });
const exited = new Promise((resolve) => child.once('exit', resolve));
const get = (route, ip = '192.0.2.1') =>
  fetch(`http://127.0.0.1:${port}${route}`, {
    headers: { 'X-Real-IP': ip },
    signal: AbortSignal.timeout(3000),
  });
try {
  let ready = false;
  for (let attempt = 0; attempt < 300; attempt++) {
    if (spawnError) throw spawnError;
    if (child.exitCode !== null) throw new Error(`Server exited during startup:\n${output}`);
    try {
      ready = (await get('/health/ready')).status === 200;
    } catch {
      /* Wait for listener. */
    }
    if (ready) break;
    await delay(100);
  }
  assert(ready, `Server did not become ready:\n${output}`);
  const homepageResponse = await get('/');
  assert.equal(homepageResponse.status, 200);
  assert.match(
    await homepageResponse.text(),
    /<link rel="canonical" href="https:\/\/flagship.example\/">/,
  );
  const robots = await get('/robots.txt');
  assert.equal(robots.status, 200);
  assert.match(await robots.text(), /Sitemap: https:\/\/flagship.example\/sitemap.xml/);
  const sitemap = await get('/sitemap.xml');
  assert.equal(sitemap.status, 200);
  assert.match(await sitemap.text(), /sitemaps\/pages.xml/);
  const pages = await (await get('/sitemaps/pages.xml')).text();
  assert.match(pages, /flagship.example\/GamingLaptop/);
  assert.doesNotMatch(pages, /\/(login|alerts|api|settings)</);
  const missing = await get('/product/not-a-real-product');
  assert.equal(missing.status, 404);
  assert.match(missing.headers.get('x-robots-tag'), /noindex/);
  const privatePage = await get('/alerts');
  assert.match(privatePage.headers.get('x-robots-tag'), /noindex/);
  const info = await (await get('/api/info')).json();
  const featured = Object.values(info.categories)
    .flat()
    .filter((category) => category.featured)
    .map((category) => category.url);
  assert.deepEqual([...featured].sort(), ['GamingLaptop', 'GamingPC', 'Smartphone'].sort());
  const home = await (await get('/api/homepage')).json();
  assert.deepEqual(home.featured_sections, featured);
  assert.equal(home.market_today.price_drops, 0);
  assert.equal(home.market_today.biggest_drop, null);
  const cachedHome = await get('/api/homepage');
  assert.equal(cachedHome.headers.get('x-flagship-cache'), 'HIT');
  assert.deepEqual(await cachedHome.json(), home);
  assert.equal((await get('/api/GamingLaptop/products?sort=best_deals')).status, 200);
  const activity = await (await get('/api/catalog-activity')).json();
  assert.deepEqual(activity.totals, { added: 0, edited: 0, removed: 0 });
  assert.equal((await get('/api/auth/session')).status, 200);
  for (let attempt = 1; attempt < 120; attempt++)
    assert.equal((await get('/api/auth/session')).status, 200);
  assert.equal((await get('/api/auth/session')).status, 429);
  assert.equal(
    (await get('/api/auth/session', '192.0.2.2')).status,
    200,
    'One visitor must not exhaust another visitor’s limit',
  );
  assert.equal(
    (await fetch(`http://127.0.0.1:${port}/health/live`)).status,
    400,
    'Trusted proxy requests need an explicit client IP',
  );
  console.log(
    'Startup smoke passed: fresh migrations, website, API, serve-only without Discord/browser/scanning, and isolated proxy rate limits.',
  );
} finally {
  if (child.exitCode === null && !spawnError) {
    child.kill('SIGTERM');
    await Promise.race([exited, delay(5000)]);
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
    await exited;
  }
  assert.equal(path.dirname(temporary), path.resolve(tmpdir()));
  await rm(temporary, { recursive: true, force: true });
}
