export function localWebsiteOrigin(settings, live, backendOrigin) {
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
