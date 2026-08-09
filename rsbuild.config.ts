import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { defineConfig } from "@rsbuild/core";
import { pluginReact } from "@rsbuild/plugin-react";
import { pluginSvgr } from "@rsbuild/plugin-svgr";

const projectRoot = path.dirname(fileURLToPath(import.meta.url));
const webSrc = path.resolve(projectRoot, "web/src");

const buildTime = process.env.ASTROBOX_BUILD_TIME ?? new Date().toISOString();
const buildUser =
  process.env.ASTROBOX_BUILD_USER ??
  process.env.USER ??
  process.env.LOGNAME ??
  "unknown";
const buildEnv =
  process.env.ASTROBOX_BUILD_ENV ?? process.env.NODE_ENV ?? "development";
const buildDefines = {
  __ASTROBOX_BUILD_TIME__: JSON.stringify(buildTime),
  __ASTROBOX_BUILD_USER__: JSON.stringify(buildUser),
  __ASTROBOX_BUILD_ENV__: JSON.stringify(buildEnv),
} satisfies Record<string, string>;
const isProduction = buildEnv === "production";
const isDev = !isProduction;
const rspackCacheDir = path.resolve(projectRoot, "node_modules/.cache/rspack");
const watchIgnorePattern = /[\\/](?:src-tauri|dist|target)[\\/]/;

export default defineConfig({
  plugins: [pluginReact(), pluginSvgr()],
  source: {
    entry: {
      app: "./web/src/index.tsx",
    },
    define: buildDefines,
  },
  html: {
    title: "AstroBox",
    favicon: "./web/favicon.svg",
    tags: [
      {
        tag: "style",
        children: `
html { background: #000; }
html[data-startup-theme]::after {
  content: "";
  position: fixed;
  inset: 0;
  z-index: 2147483647;
  background: #000;
  opacity: 1;
  border-radius: var(--window-radius, 0);
  pointer-events: none;
  transition: opacity 350ms ease;
}
html[data-startup-theme].startup-theme-ready::after {
  opacity: 0;
}
`,
        head: true,
        append: false,
      },
      {
        tag: "script",
        children:
          'document.documentElement.setAttribute("data-startup-theme", "")',
        head: true,
        append: false,
      },
    ],
    meta: {
      referrer: "no-referrer",
      viewport:
        "viewport-fit=cover ,width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no",
    },
  },
  server: {
      port: 9191,
      host: '0.0.0.0'
  },
  tools: {
    lightningcssLoader: false,
    htmlPlugin(config, { entryName }) {
      if (entryName === "app") {
        config.filename = "index.html";
      }
    },
    rspack(config) {
      config.watchOptions = {
        ...config.watchOptions,
        ignored: watchIgnorePattern,
      };
      const wasmEntry = path.resolve(
        projectRoot,
        "src-tauri/modules/app_wasm/pkg/astrobox_ng_wasm.js",
      );
      config.resolve = {
        ...config.resolve,
        alias: {
          ...config.resolve?.alias,
          "@": webSrc,
          ...(fs.existsSync(wasmEntry) ? { "@app-wasm": wasmEntry } : {}),
        },
      };
      if (isDev) {
        config.devtool = "eval-cheap-module-source-map";
        config.cache = {
          type: "filesystem",
          cacheDirectory: rspackCacheDir,
          buildDependencies: {
            config: [path.resolve(projectRoot, "rsbuild.config.ts")],
            packageJson: [path.resolve(projectRoot, "package.json")],
            lockfile: [path.resolve(projectRoot, "pnpm-lock.yaml")],
            tsconfig: [path.resolve(projectRoot, "tsconfig.json")],
          },
        };
        const managedPaths = [
          ...(config.snapshot?.managedPaths ?? []),
          /[\\/]node_modules[\\/]/,
          /[\\/]node_modules[\\/]\\.pnpm[\\/]/,
        ];
        const immutablePaths = [
          ...(config.snapshot?.immutablePaths ?? []),
          /[\\/]node_modules[\\/]\\.pnpm[\\/]/,
        ];
        config.snapshot = {
          ...(config.snapshot ?? {}),
          managedPaths,
          immutablePaths,
        };
      }
    },
  },
  performance: {
    removeConsole: isProduction,
    chunkSplit: isProduction ? { strategy: "split-by-experience" } : false,
  },
  dev: {
    lazyCompilation: true,
  },
  output: {
    polyfill: isProduction ? "usage" : "off",
    injectStyles: isDev,
  },
});
