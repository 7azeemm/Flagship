import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv } from 'vite';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const canonicalOrigin = env.FLAGSHIP_APP_ORIGIN || 'http://127.0.0.1:5173';
  return {
    plugins: [react(), tailwindcss(), {
      name: 'flagship-local-origin',
      configureServer(server) {
        server.middlewares.use((request, response, next) => {
          if (['GET', 'HEAD'].includes(request.method || '') && request.headers.host !== new URL(canonicalOrigin).host) {
            response.writeHead(302, { Location: canonicalOrigin + (request.url || '/'), 'Cache-Control': 'no-store' });
            response.end();
            return;
          }
          next();
        });
      },
    }],
    server: {
      port: 5173,
      strictPort: true,
      proxy: {
        '/api': {
          target: env.API_PROXY_TARGET || 'http://127.0.0.1:3000',
          changeOrigin: true,
        },
      },
    },
    build: { sourcemap: false },
  };
});
