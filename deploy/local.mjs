import { existsSync, readFileSync, readdirSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseEnv } from 'node:util';
import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:net';
import { createHash } from 'node:crypto';
import { createInterface } from 'node:readline/promises';
import { setTimeout as delay } from 'node:timers/promises';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const website = join(root, 'website');
const directus = join(root, 'deploy', 'directus');
const windows = process.platform === 'win32';
const jobs = new Set();
let stopping = false;
let settings;
let database;
let postgresBin;

const help = `Flagship launcher (Windows and Linux)
  setup     Create missing database roles/database, migrate, install dependencies,
            install Chromium with system dependencies, and configure Directus.
            Install PostgreSQL and Node.js 22 LTS first.
  dev       Backend + live website at the configured loopback origin (default 127.0.0.1:5173).
  directus  Staff editor only; stop it when you do not need it.
  all       Backend + live website + staff editor.
  serve     Build the website and serve it with the backend; no live frontend server.
  production Build and run the release backend with the configured public HTTPS origin.
  production-directus Staff editor using the already-built release binary for migrations.
  apply-overrides  Replay saved URL-based product edits into the current catalog and exit.
  reset     Delete the local flagship database and recreate/setup it (confirmation required).
  --help    Show this help.
Run start.bat (Windows) or sh start.sh (Linux) without arguments for a menu.
For production: sh start.sh production. Ctrl+C stops processes started here.
PostgreSQL stays running as a service. Setup preserves data; reset deletes all database data.`;

function required(name) {
  if (!settings[name]?.trim()) throw new Error(`Set ${name} in the root .env first.`);
  return settings[name];
}

function localWebsiteOrigin(settings, live, backendOrigin) {
  if (!live) return backendOrigin;
  const candidate = settings.LOCAL_APP_PUBLIC_URL || settings.APP_PUBLIC_URL;
  let configured;
  try { configured = candidate && new URL(candidate); } catch { /* Use the local default. */ }
  const valid = configured?.protocol === 'http:' && ['localhost', '127.0.0.1', '[::1]'].includes(configured.hostname)
    && configured.port === '5173' && !configured.username && !configured.password
    && configured.pathname === '/' && !configured.search && !configured.hash;
  if (settings.LOCAL_APP_PUBLIC_URL && !valid) throw new Error('LOCAL_APP_PUBLIC_URL must be an HTTP loopback origin on port 5173.');
  return valid ? configured.origin : 'http://127.0.0.1:5173';
}

function start(command, args, cwd = root, env = {}, detached = !windows) {
  const childEnv = { ...settings, ...process.env, ...env };
  // npm lifecycle scripts must use the same Node as the launcher, not a newer system Node.
  const pathKey = Object.keys(childEnv).find(key => key.toLowerCase() === 'path') || 'PATH';
  childEnv[pathKey] = dirname(process.execPath) + (windows ? ';' : ':') + (childEnv[pathKey] || '');
  const child = spawn(command, args, { cwd, env: childEnv, stdio: 'inherit', windowsHide: true, detached });
  child.flagshipDetached = detached;
  jobs.add(child);
  const done = new Promise((resolveDone, reject) => {
    child.once('error', error => { jobs.delete(child); reject(new Error(`Could not start ${command}: ${error.message}`)); });
    child.once('exit', (code, signal) => { jobs.delete(child); resolveDone({ code, signal }); });
  });
  // Long-running jobs are awaited after startup, but can fail before then.
  done.catch(() => {});
  return { child, done };
}

async function run(command, args, cwd = root, env = {}, detached = !windows) {
  const result = await start(command, args, cwd, env, detached).done;
  if (result.code !== 0) throw new Error(`${command} failed. Check its output above.`);
}

function stop(child) {
  if (!jobs.has(child) || !child.pid) return;
  // Kill only this launcher's process tree, including Cargo's backend child.
  if (windows) spawnSync('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true });
  else { try { process.kill(child.flagshipDetached ? -child.pid : child.pid, 'SIGTERM'); } catch { /* Already stopped. */ } }
  jobs.delete(child);
}

function cleanup() {
  stopping = true;
  for (const child of jobs) stop(child);
}

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.once(signal, () => { cleanup(); process.exit(0); });
}

function npmCli() {
  const candidates = [
    join(dirname(process.execPath), 'node_modules', 'npm', 'bin', 'npm-cli.js'),
    join(dirname(process.execPath), '..', 'lib', 'node_modules', 'npm', 'bin', 'npm-cli.js'),
    ...(!windows ? ['/usr/share/nodejs/npm/bin/npm-cli.js', '/usr/share/npm/bin/npm-cli.js'] : []),
    process.env.APPDATA && join(process.env.APPDATA, 'npm', 'node_modules', 'npm', 'bin', 'npm-cli.js'),
  ];
  const cli = candidates.find(path => path && existsSync(path));
  if (!cli) throw new Error('npm was not found beside Node.js. Install Node.js with npm included.');
  return cli;
}

function dependencyFingerprint(directory) {
  const lock = join(directory, 'package-lock.json');
  return createHash('sha256').update(readFileSync(join(directory, 'package.json')))
    .update(existsSync(lock) ? readFileSync(lock) : '').update(process.versions.modules).digest('hex');
}

async function installDependencies(directory, omitDev = false) {
  const stamp = join(directory, 'node_modules', '.flagship-install');
  if (existsSync(stamp) && readFileSync(stamp, 'utf8') === dependencyFingerprint(directory)) return;
  const command = existsSync(join(directory, 'package-lock.json')) ? 'ci' : 'install';
  await run(process.execPath, [npmCli(), command, ...(omitDev ? ['--omit=dev'] : []), '--no-audit', '--no-fund'], directory);
  // Write only after a successful install, so partial installs and Node ABI changes are retried.
  writeFileSync(stamp, dependencyFingerprint(directory));
}

function pgTool(name) {
  return postgresBin ? join(postgresBin, name + (windows ? '.exe' : '')) : name;
}

function locatePostgres() {
  const configured = settings.POSTGRES_BIN_DIR;
  if (configured) {
    if (!existsSync(join(configured, 'psql' + (windows ? '.exe' : '')))) throw new Error('POSTGRES_BIN_DIR must contain psql.');
    return configured;
  }
  if (!windows) return undefined;
  const onPath = spawnSync('where.exe', ['psql.exe'], { encoding: 'utf8', windowsHide: true });
  if (onPath.status === 0) return dirname(onPath.stdout.trim().split(/\r?\n/)[0]);
  const base = join(process.env.ProgramFiles || 'C:\\Program Files', 'PostgreSQL');
  if (existsSync(base)) {
    for (const version of readdirSync(base).sort((a, b) => Number(b) - Number(a))) {
      const bin = join(base, version, 'bin');
      if (existsSync(join(bin, 'psql.exe'))) return bin;
    }
  }
  throw new Error('Install PostgreSQL 17 for Windows first, including command-line tools and its Windows service. For a custom location, set POSTGRES_BIN_DIR in .env.');
}

function pgEnvironment(user = 'flagship', password = settings.FLAGSHIP_DB_PASSWORD, db = 'flagship') {
  return {
    PGHOST: database.hostname.replace(/^\[|\]$/g, ''), PGPORT: database.port || '5432',
    PGUSER: user, PGPASSWORD: password, PGDATABASE: db, PGCONNECT_TIMEOUT: '5',
    PGSSLMODE: database.searchParams.get('sslmode') || 'prefer',
  };
}

function pgProbe(args, env) {
  return spawnSync(pgTool('psql'), ['-X', '-w', '-v', 'ON_ERROR_STOP=1', ...args], {
    cwd: root, env: { ...process.env, ...env }, stdio: 'ignore', windowsHide: true,
  }).status === 0;
}

async function ensurePostgres() {
  postgresBin = locatePostgres();
  const env = pgEnvironment();
  const ready = () => spawnSync(pgTool('pg_isready'), ['-q'], {
    env: { ...process.env, ...env }, stdio: 'ignore', windowsHide: true,
  }).status === 0;
  if (!ready() && windows) {
    await run('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', join(root, 'deploy', 'start-postgres.ps1')], root, {
      POSTGRES_SERVICE: settings.POSTGRES_SERVICE || '',
    });
  }
  for (let attempt = 0; attempt < 30; attempt++) {
    if (ready()) return;
    await delay(1000);
  }
  throw new Error('PostgreSQL is not accepting connections at DATABASE_URL. Start it and check the configured port.');
}

function ensureDatabase() {
  if (!pgProbe(['-Atqc', 'SELECT 1'], pgEnvironment())) {
    throw new Error('Cannot connect as flagship. Run start.bat setup (Windows) or sh start.sh setup (Linux), or check DATABASE_URL and the existing database password.');
  }
}

async function migrate(installBrowser = false) {
  if (installBrowser && !windows) {
    // Compile before authenticating so a long build cannot expire the sudo timestamp.
    await run('cargo', ['build', '--locked']);
    console.log('Authenticate sudo to install Chromium system dependencies.');
    // Keep the controlling terminal/session for sudo and Playwright's subsequent sudo call.
    await run('sudo', ['-v'], root, {}, false);
  }
  await run('cargo', ['run', '--locked', '--', '--migrate-only',
    ...(installBrowser ? ['--install-browser'] : [])], root, {}, installBrowser ? false : !windows);
}
const directusCli = () => join(directus, 'node_modules', 'directus', 'cli.js');

function directusEnvironment() {
  const port = settings.DIRECTUS_PORT || '8055';
  return {
    HOST: '127.0.0.1', PORT: port, SECRET: required('DIRECTUS_SECRET'),
    PUBLIC_URL: settings.DIRECTUS_PUBLIC_URL || `http://localhost:${port}`,
    DB_CLIENT: 'pg', DB_HOST: database.hostname.replace(/^\[|\]$/g, ''), DB_PORT: database.port || '5432',
    DB_DATABASE: 'flagship', DB_USER: 'flagship_directus', DB_PASSWORD: required('DIRECTUS_DB_PASSWORD'),
    ADMIN_EMAIL: required('DIRECTUS_ADMIN_EMAIL'), ADMIN_PASSWORD: required('DIRECTUS_ADMIN_PASSWORD'),
    ADMIN_TOKEN: required('DIRECTUS_ADMIN_TOKEN'), CACHE_ENABLED: 'false', TELEMETRY: 'false', WEBSOCKETS_ENABLED: 'false',
    DB_EXCLUDE_TABLES: '',
    STORAGE_LOCAL_ROOT: join(root, 'state', 'directus', 'uploads'),
    EXTENSIONS_PATH: join(root, 'state', 'directus', 'extensions'),
  };
}

async function waitFor(url, job, service = 'Directus', attempts = 120, headers = {}) {
  for (let attempt = 0; attempt < attempts; attempt++) {
    if (!jobs.has(job.child)) throw new Error(`${service} stopped before it was ready. Check its output above.`);
    let response;
    try {
      response = await fetch(url, { signal: AbortSignal.timeout(1500), headers });
    } catch { /* Connection refusals and timeouts are expected during startup. */ }
    if (response?.ok) return;
    if (response?.status >= 400 && response.status < 500) {
      throw new Error(`${service} readiness check returned HTTP ${response.status}: ${(await response.text()).slice(0, 300)}`);
    }
    await delay(1000);
  }
  throw new Error(`${service} did not become ready within ${attempts} startup checks.`);
}

async function ensurePortFree(port, host = '127.0.0.1') {
  const value = Number(port);
  if (!Number.isInteger(value) || value < 1 || value > 65535) throw new Error('Service ports must be between 1 and 65535.');
  await new Promise((resolveFree, reject) => {
    const server = createServer();
    server.once('error', () => reject(new Error(`Port ${value} is already in use or unavailable. Stop the existing service before launching another copy.`)));
    server.listen(value, host, () => server.close(resolveFree));
  });
}

async function startDirectus() {
  if (!existsSync(directusCli())) throw new Error('Directus is not installed. Run start.bat setup (Windows) or sh start.sh setup (Linux).');
  if (!pgProbe(['-Atqc', 'SELECT 1'], pgEnvironment('flagship_directus', required('DIRECTUS_DB_PASSWORD')))) {
    throw new Error('Cannot connect as flagship_directus. Run setup or check DIRECTUS_DB_PASSWORD against the existing role.');
  }
  const env = directusEnvironment();
  await ensurePortFree(env.PORT);
  // Grant Directus full read/write access to application tables and sequences.
  await run(pgTool('psql'), ['-X', '-q', '-v', 'flagship_mode=grants', '-f', join(root, 'deploy', 'postgres.sql')], root, pgEnvironment());
  mkdirSync(env.STORAGE_LOCAL_ROOT, { recursive: true });
  mkdirSync(env.EXTENSIONS_PATH, { recursive: true });
  await run(process.execPath, ['--no-node-snapshot', directusCli(), 'bootstrap'], directus, env);
  const job = start(process.execPath, ['--no-node-snapshot', directusCli(), 'start'], directus, env);
  const base = `http://127.0.0.1:${env.PORT}`;
  try {
    await waitFor(`${base}/server/ping`, job);
    await run(process.execPath, [join(root, 'deploy', 'setup-directus.mjs')], root, {
      DIRECTUS_SETUP_URL: base, DIRECTUS_ADMIN_TOKEN: env.ADMIN_TOKEN,
    });
    console.log(`Directus: http://localhost:${env.PORT}`);
    return job;
  } catch (error) { stop(job.child); throw error; }
}

async function setup() {
  const admin = pgEnvironment(settings.POSTGRES_ADMIN_USER || 'postgres', undefined, 'postgres');
  admin.PGPASSWORD = settings.POSTGRES_ADMIN_PASSWORD;
  if (!admin.PGPASSWORD || !pgProbe(['-Atqc', 'SELECT 1'], admin)) {
    admin.PGPASSWORD = undefined;
    console.log('Enter the PostgreSQL administrator password chosen during installation when psql asks.');
  }
  await run(pgTool('psql'), ['-X', '-v', 'flagship_mode=setup', '-f', join(root, 'deploy', 'postgres.sql')], root, {
    ...admin, FLAGSHIP_DB_PASSWORD: required('FLAGSHIP_DB_PASSWORD'), DIRECTUS_DB_PASSWORD: required('DIRECTUS_DB_PASSWORD'),
  });
  ensureDatabase();
  await migrate(true);
  await installDependencies(website);
  if (!existsSync(join(website, 'dist', 'index.html'))) {
    await run(process.execPath, [npmCli(), 'run', 'build'], website);
  }
  await installDependencies(directus, true);
  const editor = await startDirectus();
  stop(editor.child);
  await editor.done;
  console.log('Setup complete. Use start.bat (Windows) or sh start.sh (Linux). Existing data was preserved.');
}

async function resetDatabase() {
  // Validate setup credentials before offering the destructive operation.
  directusEnvironment();
  npmCli();
  const input = createInterface({ input: process.stdin, output: process.stdout });
  try {
    console.log('\nThis deletes ALL data in the local flagship database, including accounts, catalog, alerts, saved overrides, and Directus configuration. Stop Flagship and Directus and back up anything needed first.');
    if ((await input.question('Type RESET flagship to continue: ')).trim() !== 'RESET flagship') {
      console.log('Reset cancelled.');
      return;
    }
  } finally { input.close(); }
  const admin = pgEnvironment(settings.POSTGRES_ADMIN_USER || 'postgres', undefined, 'postgres');
  admin.PGPASSWORD = settings.POSTGRES_ADMIN_PASSWORD;
  if (!admin.PGPASSWORD || !pgProbe(['-Atqc', 'SELECT 1'], admin)) {
    admin.PGPASSWORD = undefined;
    console.log('Enter the PostgreSQL administrator password when psql asks.');
  }
  // Fixed local target; no FORCE, so active connections prevent deletion.
  await run(pgTool('psql'), ['-X', '-v', 'flagship_mode=reset', '-f', join(root, 'deploy', 'postgres.sql')], root, admin);
  await setup();
  console.log('Local database reset and setup complete.');
}

async function menu() {
  console.log('\nFlagship\n1. First-time setup\n2. Website + backend (live editing)\n3. Directus only\n4. Everything\n5. Built website + backend (local)\n6. Reset database (deletes all data)\n7. Production website + backend\n0. Exit\n');
  const input = createInterface({ input: process.stdin, output: process.stdout });
  try {
    const choice = (await input.question('Choose: ')).trim();
    const choices = { '1': 'setup', '2': 'dev', '3': 'directus', '4': 'all', '5': 'serve', '6': 'reset', '7': 'production', '0': 'exit' };
    if (!(choice in choices)) throw new Error('Choose a number from 0 to 7.');
    return choices[choice];
  } finally { input.close(); }
}

async function main() {
  const mode = process.argv[2] || await menu();
  if (['--help', '-h', 'help'].includes(mode)) { console.log(help); return; }
  if (mode === 'exit') return;
  if (!['setup', 'dev', 'directus', 'all', 'serve', 'production', 'production-directus', 'apply-overrides', 'reset'].includes(mode)) throw new Error(help);
  const version = process.versions.node.split('.').map(Number);
  if (version[0] < 22 || (version[0] === 22 && version[1] < 12)) throw new Error('Install Node.js 22.12 or newer; Node.js 22 LTS is recommended for native Directus dependencies.');
  if (['setup', 'directus', 'production-directus', 'all', 'reset'].includes(mode) && version[0] !== 22) {
    throw new Error('This Directus version requires Node.js 22 for its native dependencies. Run start.bat to use state/node22, or install Node.js 22 LTS.');
  }
  if (!existsSync(join(root, '.env'))) throw new Error('Create the root .env first. See docs/setup.md.');
  settings = { ...parseEnv(readFileSync(join(root, '.env'), 'utf8')), ...process.env };
  try { database = new URL(required('DATABASE_URL')); } catch { throw new Error('DATABASE_URL must be a valid PostgreSQL connection URL.'); }
  if (!['postgres:', 'postgresql:'].includes(database.protocol) || !['localhost', '127.0.0.1', '[::1]'].includes(database.hostname)
      || database.username !== 'flagship' || database.pathname !== '/flagship') {
    throw new Error('This local launcher expects DATABASE_URL for the flagship role/database on localhost.');
  }
  if (decodeURIComponent(database.password) !== required('FLAGSHIP_DB_PASSWORD')) throw new Error('DATABASE_URL password must match FLAGSHIP_DB_PASSWORD (URL-encode it in the URL).');
  await ensurePostgres();
  if (mode === 'reset') { await resetDatabase(); return; }
  if (mode === 'setup') { await setup(); return; }
  ensureDatabase();
  if (mode === 'apply-overrides') {
    await run('cargo', ['run', '--locked', '--', '--apply-overrides']);
    return;
  }
  const running = [];
  if (['directus', 'production-directus'].includes(mode)) {
    if (mode === 'production-directus') {
      await run(join(root, 'target', 'release', windows ? 'Flagship.exe' : 'Flagship'), ['--migrate-only']);
    } else {
      await migrate();
    }
    running.push(await startDirectus());
  }
  if (['dev', 'all', 'serve', 'production'].includes(mode)) {
    const production = mode === 'production';
    const live = !['serve', 'production'].includes(mode);
    if (!existsSync(join(website, 'node_modules', 'vite', 'package.json'))) throw new Error('Run start.bat setup (Windows) or sh start.sh setup (Linux) to install website dependencies.');
    if (!live) await run(process.execPath, [npmCli(), 'run', 'build'], website);
    const bind = settings.BIND_ADDRESS || '127.0.0.1:3000';
    const origin = new URL(`http://${bind}`);
    await ensurePortFree(origin.port || '80', origin.hostname.replace(/^\[|\]$/g, ''));
    if (live) await ensurePortFree(5173);
    if (origin.hostname === '0.0.0.0') origin.hostname = '127.0.0.1';
    if (origin.hostname === '[::]') origin.hostname = '[::1]';
    const scanning = settings.ENABLE_SCANNING?.trim().toLowerCase() === 'true';
    // This local launcher connects directly or through Vite, without an HTTPS proxy.
    // Override .env proxy trust so local requests and readiness probes need no X-Real-IP.
    const websiteOrigin = production ? required('APP_PUBLIC_URL') : localWebsiteOrigin(settings, live, origin.origin);
    if (production) {
      const publicUrl = new URL(websiteOrigin);
      if (publicUrl.protocol !== 'https:' || publicUrl.username || publicUrl.password || publicUrl.pathname !== '/' || publicUrl.search || publicUrl.hash) {
        throw new Error('Production APP_PUBLIC_URL must be an HTTPS origin without credentials, path, query or fragment.');
      }
    }
    const env = { ENABLE_SCANNING: String(scanning), FLAGSHIP_WEB_DIR: join(website, 'dist'), BIND_ADDRESS: bind,
      ...(production ? { FLAGSHIP_LOCAL_DEV: 'false' } : { TRUSTED_PROXY_IPS: '', FLAGSHIP_LOCAL_DEV: 'true', APP_PUBLIC_URL: websiteOrigin }) };
    if (!production && settings.GOOGLE_CLIENT_ID?.trim()) {
      env.GOOGLE_REDIRECT_URL = `${websiteOrigin}/api/auth/google/callback`;
      console.log(`Google OAuth callback: ${env.GOOGLE_REDIRECT_URL} (register this exact URI in Google Cloud).`);
    }
    if (production) await run('cargo', ['build', '--release', '--locked'], root, env);
    const backend = production
      ? start(join(root, 'target', 'release', windows ? 'Flagship.exe' : 'Flagship'), [], root, env)
      : start('cargo', ['run', '--locked'], root, env);
    running.push(backend);
    console.log(`Automatic scanning: ${scanning ? 'enabled' : 'disabled'} (ENABLE_SCANNING).`);
    console.log('Waiting for the backend to finish building, apply migrations, and become ready...');
    await waitFor(`${origin.origin}/health/ready`, backend, 'Flagship backend', 1800,
      production ? { 'X-Real-IP': '127.0.0.1' } : {});
    console.log(`Backend ready: ${origin.origin}`);
    if (live) running.push(start(process.execPath, [npmCli(), 'run', 'dev'], website, { API_PROXY_TARGET: origin.origin, FLAGSHIP_APP_ORIGIN: websiteOrigin }));
    console.log(`Website: ${websiteOrigin}\nCtrl+C stops this launcher's processes. PostgreSQL stays running.`);
    if (mode === 'all') {
      console.log('Starting Directus...');
      running.push(await startDirectus());
    }
  }
  const result = await Promise.race(running.map(job => job.done));
  if (!stopping && result.code !== 0) throw new Error('A service stopped unexpectedly. Check its output above.');
}

try { await main(); } catch (error) { console.error(`\n${error.message}`); process.exitCode = 1; }
finally { cleanup(); }
