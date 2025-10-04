import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { defineConfig } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';

const projectRoot = path.dirname(fileURLToPath(import.meta.url));
const webSrc = path.resolve(projectRoot, 'web/src');

export default defineConfig({
  plugins: [pluginReact()],
  source: {
    entry: {
      app: './web/src/index.tsx',
    },
  },
  html: {
    title: "AstroBox",
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
