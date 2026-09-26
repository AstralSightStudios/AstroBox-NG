#!/usr/bin/env node
/**
 * 主题包校验 / 打包工具（给制作主题的人和 AI 用）。
 *
 *   node scripts/validate-theme.mjs <主题文件夹 | theme.json | 主题包.abtheme>
 *   node scripts/validate-theme.mjs <主题文件夹> --pack 输出.abtheme
 *
 * 读包、校验走的是 App 里**同一份代码**（web/src/logic/themeUi/packageFormat.ts 与
 * schema.ts）：App 导入时会静默丢掉的字段、被夹到范围内的数值，这里逐条报出来。
 * 另外检查素材引用、没被引用的素材、会卡的动画 SVG、过大的图片 / 视频。
 *
 * 退出码：有错误（✖）为 1，只有警告（⚠）为 0。
 * 编写规范见 web/docs/theme-authoring.md。
 */
import { registerHooks } from "node:module";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const webSrc = path.join(root, "web/src");

// 源码按打包器约定写（无后缀相对路径、@/ 别名），Node 自己不认；
// md3.ts 顶层引用了 IPC，这里换成空实现。
registerHooks({
  resolve(specifier, context, next) {
    if (specifier === "@/ipc/invoke") {
      return { shortCircuit: true, url: "data:text/javascript,export const invoke=async()=>undefined" };
    }
    if (specifier === "@/ipc/runtime") {
      return { shortCircuit: true, url: "data:text/javascript,export const isTauriRuntime=false" };
    }
    const target = specifier.startsWith("@/")
      ? pathToFileURL(path.join(webSrc, specifier.slice(2))).href
      : specifier;
    try {
      return next(target, context);
    } catch (error) {
      if (target.startsWith(".") || target.startsWith("file:")) {
        for (const ext of [".ts", ".tsx"]) {
          try {
            return next(`${target}${ext}`, context);
          } catch {}
        }
      }
      throw error;
    }
  },
});

const schema = await import(pathToFileURL(path.join(webSrc, "logic/themeUi/schema.ts")).href);
const format = await import(pathToFileURL(path.join(webSrc, "logic/themeUi/packageFormat.ts")).href);
const { ACCENT_PRESETS } = await import(pathToFileURL(path.join(webSrc, "logic/accentColor.ts")).href);
const { SCHEME_VARIANTS } = await import(pathToFileURL(path.join(webSrc, "logic/md3.ts")).href);
const { zipSync } = await import("fflate");

// ==================== 输出 ====================

let errors = 0;
let warnings = 0;
const error = (msg) => {
  errors += 1;
  console.log(`  ✖ ${msg}`);
};
const warn = (msg) => {
  warnings += 1;
  console.log(`  ⚠ ${msg}`);
};
const ok = (msg) => console.log(`  ✔ ${msg}`);
const section = (title) => console.log(`\n${title}`);

// ==================== 读取输入 ====================

const args = process.argv.slice(2);
const packIndex = args.indexOf("--pack");
const packOut = packIndex >= 0 ? args[packIndex + 1] : undefined;
const input = args.find(
  (arg, index) => !arg.startsWith("--") && (packIndex < 0 || index !== packIndex + 1),
);
if (!input || (packIndex >= 0 && !packOut)) {
  console.log("用法: node scripts/validate-theme.mjs <主题文件夹 | theme.json | 主题包.abtheme> [--pack 输出.abtheme]");
  process.exit(2);
}

function zipFolder(dir) {
  const files = {};
  const walk = (current, prefix) => {
    for (const name of readdirSync(current)) {
      if (name === ".DS_Store" || name.startsWith("._")) continue;
      const full = path.join(current, name);
      if (statSync(full).isDirectory()) walk(full, `${prefix}${name}/`);
      else files[`${prefix}${name}`] = [new Uint8Array(readFileSync(full)), { level: 0 }];
    }
  };
  walk(dir, "");
  return zipSync(files);
}

let bytes;
const resolved = path.resolve(input);
if (!existsSync(resolved)) {
  console.log(`找不到 ${resolved}`);
  process.exit(2);
}
if (statSync(resolved).isDirectory()) {
  bytes = zipFolder(resolved);
} else if (path.basename(resolved) === "theme.json") {
  bytes = zipFolder(path.dirname(resolved));
} else {
  bytes = new Uint8Array(readFileSync(resolved));
}

// ==================== 包结构 ====================

section("包结构");
let pkg;
try {
  pkg = await format.readThemePackage(bytes);
  ok(`theme.json 可读，包内素材 ${pkg.assets.size} 个，打包后 ${(bytes.length / 1024).toFixed(1)} KB`);
} catch (err) {
  const code = err?.code ?? "invalid";
  error(
    code === "tooLarge"
      ? "素材或整包超出大小上限（图片 16 MB、视频 48 MB、theme.json 4 MB、整包 192 MB）"
      : "不是有效的主题包：需要 zip 内含 theme.json（根目录或唯一一层文件夹里），且 theme.json 是 JSON 对象",
  );
  process.exit(1);
}
const { manifest, assets } = pkg;

// ==================== 顶层字段 ====================

section("主题字段");
const KNOWN_TOP = new Set(["$schema", "id", "name", "accent", "variant", "recolor", "light", "dark", "previewImage", "ui"]);
for (const key of Object.keys(manifest)) {
  if (!KNOWN_TOP.has(key)) warn(`未知字段 "${key}"，导入时会被忽略`);
}
if (typeof manifest.name !== "string" || !manifest.name.trim()) {
  warn(`缺少 name，导入后显示为 "Theme"`);
} else ok(`name: ${manifest.name}`);

const presetNames = ACCENT_PRESETS.map((preset) => preset.name);
const accent = manifest.accent;
if (accent === undefined) {
  warn(`缺少 accent，导入后强调色回落为 blue`);
} else if (accent?.mode === "preset" && presetNames.includes(accent.preset)) {
  ok(`accent: 预设 ${accent.preset}`);
} else if (accent?.mode === "custom" && /^#[0-9a-fA-F]{6}$/.test(accent.color ?? "")) {
  ok(`accent: 自定义 ${accent.color}`);
} else {
  error(`accent 无效，会回落为 blue。可选：{ "mode": "preset", "preset": ${presetNames.join(" | ")} } 或 { "mode": "custom", "color": "#RRGGBB" }`);
}
if (manifest.variant !== undefined && !SCHEME_VARIANTS.includes(manifest.variant)) {
  error(`variant 无效，可选：${SCHEME_VARIANTS.join(" | ")}`);
}
if (manifest.recolor !== undefined && typeof manifest.recolor !== "boolean") {
  error(`recolor 必须是 true / false`);
}
for (const slot of ["light", "dark"]) {
  const palette = manifest[slot];
  if (palette === undefined) continue;
  for (const [key, value] of Object.entries(palette ?? {})) {
    if (!["bg", "card", "text"].includes(key)) warn(`${slot}.${key} 不是已知的配色槽（bg / card / text）`);
    else if (!/^#[0-9a-fA-F]{6}$/.test(String(value))) error(`${slot}.${key} 必须是 #RRGGBB（配色不支持透明度）`);
  }
  warn(`${slot} 配色只对「内置主题」生效；用户导入的主题由 accent + variant + recolor 按 Material You 派生背景 / 卡片 / 文字色`);
}
if (manifest.previewImage !== undefined) {
  const preview = String(manifest.previewImage);
  if (!/^data:image\/(?:png|jpe?g|webp);base64,/i.test(preview)) error(`previewImage 必须是 png / jpeg / webp 的 base64 data URL`);
  else if (preview.length > 700_000) error(`previewImage 超过 700000 字符，会被丢弃`);
}

// ==================== 界面装扮 ====================

section("界面装扮（ui）");
const rawUi = manifest.ui;
const ui = schema.parseThemeUi(rawUi);
if (rawUi === undefined) {
  warn(`没有 ui 字段：这只是一套配色主题`);
} else {
  let diffs = 0;
  const visit = (raw, parsed, trail) => {
    if (raw !== null && typeof raw === "object" && !Array.isArray(raw)) {
      for (const [key, value] of Object.entries(raw)) {
        visit(value, parsed && typeof parsed === "object" ? parsed[key] : undefined, [...trail, key]);
      }
      return;
    }
    const where = `ui.${trail.join(".")}`;
    if (parsed === undefined) {
      diffs += 1;
      warn(`${where} = ${JSON.stringify(raw)} 被丢弃（字段名 / 类型 / 取值不合法，或所在的层缺素材）`);
    } else if (
      !(typeof raw === "string" && typeof parsed === "string" && raw.toUpperCase() === parsed.toUpperCase()) &&
      raw !== parsed
    ) {
      diffs += 1;
      warn(`${where} = ${JSON.stringify(raw)} 被规范为 ${JSON.stringify(parsed)}（超出范围或非整数）`);
    }
  };
  visit(rawUi, ui, []);
  if (diffs === 0) ok(`所有字段合法`);
}

// ==================== 素材 ====================

section("素材");
const referenced = schema.collectThemeUiAssets(schema.parseThemeUi(rawUi));
for (const id of referenced) {
  if (!assets.has(id)) error(`引用了 assets/${id}，但包里没有这个文件（导入时整层会被去掉）`);
}
for (const id of assets.keys()) {
  if (!referenced.has(id)) warn(`assets/${id} 没有被 ui 引用，白占体积`);
}

/** 只能用图片的槽位（按钮底图走 CSS background、图标 / 动图 / 边框 / 进度条是 <img> 或 border-image）。 */
const imageOnly = [];
const collectImageOnly = (value, trail) => {
  if (!value || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    const next = [...trail, key];
    const where = next.join(".");
    if (typeof child === "string" && schema.isThemeAssetId(child)) {
      const mediaSlot =
        key === "asset" &&
        (/^pages\./.test(where) ||
          /^banners\./.test(where) ||
          /^nav\.(background|sideBackground)\./.test(where) ||
          /^cards\.[^.]+(\.[^.]+)?\.image\./.test(where));
      if (!mediaSlot) imageOnly.push([where, child]);
    } else collectImageOnly(child, next);
  }
};
collectImageOnly(ui, []);
for (const [where, id] of imageOnly) {
  if (schema.themeAssetKind(id) === "video") error(`ui.${where} 只支持图片，视频 ${id} 不会显示`);
}

const pngSize = (data) =>
  data[0] === 0x89 && data[1] === 0x50
    ? { width: (data[16] << 24) | (data[17] << 16) | (data[18] << 8) | data[19], height: (data[20] << 24) | (data[21] << 16) | (data[22] << 8) | data[23] }
    : undefined;

for (const [id, data] of assets) {
  const kind = schema.themeAssetKind(id);
  const mb = data.length / 1024 / 1024;
  if (kind === "video" && mb > 15) warn(`${id} 有 ${mb.toFixed(1)} MB，循环背景视频建议 ≤ 720p、≤ 15 MB`);
  if (kind === "image" && mb > 2) warn(`${id} 有 ${mb.toFixed(1)} MB，图片解码会占大量内存，建议压到 2 MB 以内`);
  if (id.toLowerCase().endsWith(".webm")) warn(`${id}：iOS 旧系统不支持 WebM，背景视频优先用 H.264 的 MP4`);
  const size = pngSize(data);
  if (size && Math.max(size.width, size.height) > 2560) {
    warn(`${id} 尺寸 ${size.width}×${size.height}，超过 2560px 没有意义（最大的全屏背景 2x 也用不到）`);
  }
  if (id.toLowerCase().endsWith(".svg")) {
    const text = new TextDecoder().decode(data);
    if (/<script|\son[a-z]+\s*=/i.test(text)) warn(`${id} 含脚本 / 事件属性：当图片加载时不会执行，删掉`);
    if (/@keyframes|<animate|<animateTransform|<animateMotion|<set\s/i.test(text)) {
      warn(`${id} 是动画 SVG：当图片显示时它在主线程上逐帧重绘，切换页面时必卡。点击 / 按压 / 加载动效改用 tapMotion / press.halo / loading.spin，或 GIF / APNG / 动态 WebP`);
    }
    if (/(?:xlink:)?href\s*=\s*["']https?:/i.test(text)) warn(`${id} 引用了外部资源：当图片加载时会被拦截，内联进去`);
  }
}
if (assets.size > 0) ok(`素材检查完成`);

// ==================== 打包 ====================

if (packOut) {
  section("打包");
  if (errors > 0) {
    error(`有错误，未打包`);
  } else {
    writeFileSync(path.resolve(packOut), bytes);
    ok(`已写出 ${path.resolve(packOut)}`);
  }
}

console.log(`\n结果：${errors} 个错误，${warnings} 个警告`);
process.exit(errors > 0 ? 1 : 0);
