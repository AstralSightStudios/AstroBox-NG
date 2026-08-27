import tailwindcss from "@tailwindcss/postcss";
import presetEnv from "postcss-preset-env";

import appLayer from "./scripts/postcss-app-layer.mjs";

export default {
  plugins: [
    tailwindcss({ config: "./tailwind.config.cjs" }),
    // cascade-layers 的 polyfill 只按单文件模拟层序，看不见项目自己的 CSS，
    // 结果把层叠顺序拧成了「项目 CSS 恒定垫底」，代价还是 1.1 MB 产物和
    // 11 万个 :not(#\#) 选择器。这里关掉它改用原生 @layer，配合下面的
    // appLayer 还原原有顺序。只关这一个特性——浏览器目标（也就是 SWC 与
    // core-js 的目标）一律不动。
    presetEnv({ features: { "cascade-layers": false } }),
    appLayer(),
  ],
};
