import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { defineConfig } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';

const projectRoot = path.dirname(fileURLToPath(import.meta.url));
const webSrc = path.resolve(projectRoot, 'web/src');

const buildTime = process.env.ASTROBOX_BUILD_TIME ?? new Date().toISOString();
const buildUser = process.env.ASTROBOX_BUILD_USER ?? process.env.USER ?? process.env.LOGNAME ?? 'unknown';
const buildEnv = process.env.ASTROBOX_BUILD_ENV ?? process.env.NODE_ENV ?? 'development';
const buildDefines = {
  __ASTROBOX_BUILD_TIME__: JSON.stringify(buildTime),
  __ASTROBOX_BUILD_USER__: JSON.stringify(buildUser),
  __ASTROBOX_BUILD_ENV__: JSON.stringify(buildEnv),
} satisfies Record<string, string>;

export default defineConfig({
  plugins: [pluginReact()],
  source: {
    entry: {
      app: './web/src/index.tsx',
    },
    define: buildDefines,
  },
  html: {
    title: "AstroBox",
    favicon:"./web/favicon.svg",
    meta: {
      referrer: 'no-referrer',
      viewport: 'viewport-fit=cover ,width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no'
    },
  },
  tools: {
    lightningcssLoader: false,
    htmlPlugin(config, { entryName }) {
      if (entryName === 'app') {
        config.filename = 'index.html';
      }
    },
    rspack(config) {
      config.watchOptions = {
        ...config.watchOptions,
        ignored: /[\\/](src-tauri)[\\/]/,
      };
      const wasmEntry = path.resolve(projectRoot, 'src-tauri/modules/app_wasm/pkg/astrobox_ng_wasm.js');
      config.resolve = {
        ...config.resolve,
        alias: {
          ...config.resolve?.alias,
          '@': webSrc,
          ...(fs.existsSync(wasmEntry) ? { '@app-wasm': wasmEntry } : {}),
        },
      };
    }
  },
  performance: {
    removeConsole: process.env.NODE_ENV === 'production',
  },
  dev: {
    lazyCompilation: true,
  }
});
