import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// Tauri 固定从环境变量读取 dev 端口；这里写死 1420 与 tauri.conf.json 的 devUrl 对应
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    // Tauri 的 webview 支持范围
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
