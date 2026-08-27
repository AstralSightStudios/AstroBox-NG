import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * 把项目自己的 CSS 统一收进 `app` 层。
 *
 * 背景：Radix Themes 与 Tailwind 的样式都在 `@layer` 里（层序见
 * web/src/tailwind.css 顶部的声明），项目自己的 CSS 则一直是无层的。
 * 此前 postcss-preset-env 的 cascade-layers polyfill 用堆 `:not(#\#)`
 * 特异性的方式模拟层序，而它是**按文件**处理的 —— 只看得见带 @layer 的
 * tailwind.css 那一份，看不见 App.css / theme.css / *.module.css。于是
 * 项目 CSS 一直停在天然特异性上，实际优先级是：
 *
 *   tailwind.css 自身的无层规则 > utilities > components > radix-themes
 *   > base > theme > 项目 CSS 与第三方运行时样式
 *
 * 注意 `:not(#\#)` 里的 `#\#` 是 ID 选择器，每个 `:not()` 加的是 (1,0,0)，
 * 所以任何被抬过的规则都压过任何没被抬过的规则，跟类的个数无关。全仓样式
 * 都是照着这个顺序写的（所以才有「写 CSS 文件要加 !important」这条经验）。
 *
 * 改用原生 @layer 后，规范规定「无层规则压过所有层叠层」，顺序会整个翻转。
 * 这里把项目 CSS 包进声明在最前面的 `app` 层，等价还原原来的顺序，同时省掉
 * polyfill 那 1.1 MB 产物和 11 万个多余的选择器匹配。
 *
 * 第三方库在运行时注入的 <style>（sonner / vaul 等）同样是无层的，由
 * web/src/logic/vendorStyleLayer.ts 在运行时补进同一个 `app` 层。
 */

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

/** 唯一带层声明的入口，原样放行。 */
const LAYERED_ENTRY = path.resolve(projectRoot, "web/src/tailwind.css");

const LAYER_NAME = "app";

/** 这些 at-rule 不参与层叠层排序，留在层外以保持与改造前完全一致的语义。 */
const HOISTED_AT_RULES = new Set([
  "charset",
  "import",
  "namespace",
  "layer",
  "font-face",
  "property",
  "keyframes",
  "-webkit-keyframes",
  "-moz-keyframes",
  "-o-keyframes",
]);

/**
 * 顶层的 `@layer` 也在 HOISTED 里，所以文件可以自己显式写
 * `@layer theme { ... }` 把某几条规则钉在别的层上 —— 那几条原本挂着
 * `:not(#\#)` 特异性堆叠的规则就是这么迁过来的，其余部分照常进 `app`。
 */
function staysOutsideLayer(node) {
  if (node.type === "atrule") {
    return HOISTED_AT_RULES.has(node.name.toLowerCase());
  }
  return false;
}

const plugin = () => ({
  postcssPlugin: "astrobox-app-layer",
  // OnceExit 保证跑在 @tailwindcss/postcss 与 postcss-preset-env 之后，
  // 看到的是各自展开完的最终结果。
  OnceExit(root, { AtRule }) {
    const file = root.source?.input?.file;
    if (file && path.resolve(file) === LAYERED_ENTRY) return;

    const wrapped = root.nodes.filter((node) => !staysOutsideLayer(node));
    if (wrapped.length === 0) {
      // 整份文件都留在层外（例如只有 @font-face）。仍然补一条空的层声明：
      // 它本身是 no-op，但让「样式表里出现过 @layer」成为「这是我们自己的
      // 产物」的可靠判据 —— 运行时的 vendorStyleLayer 正是靠这个把第三方
      // 注入的 <style> 和构建产物区分开的。
      if (root.nodes.length > 0) {
        const marker = new AtRule({ name: "layer", params: LAYER_NAME });
        marker.raws.before = "\n";
        root.append(marker);
      }
      return;
    }

    const layer = new AtRule({ name: "layer", params: LAYER_NAME });
    layer.raws.before = "\n";
    layer.raws.between = " ";
    layer.raws.after = "\n";

    wrapped[0].before(layer);
    for (const node of wrapped) {
      node.remove();
      layer.append(node);
    }
  },
});

plugin.postcss = true;

export default plugin;
