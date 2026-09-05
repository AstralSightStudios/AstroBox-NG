# 生成式人工智能代理项目守则
如果你是一名人类贡献者，则无需阅读此文。本文针对如OpenAI Codex、Trae等Agent类生成式人工智能编码工具介绍本项目，并对如何修改、测试、编译项目做出规范化处理。

## 项目介绍
该项目名为AstroBox，是一款由 Rust 驱动、高度可扩展且便携的可穿戴设备工具箱，聚焦于在穿戴设备上安装和管理第三方应用程序。项目的核心是与穿戴设备通过不同方式（SPP或BLE）建立类似串口通信的连接，并互相交换数据。

## 项目结构
本项目使用tauri框架，但切分成了多个独立、互不干扰的crate，这使得项目的核心功能能够被轻松且快速地移植到任何设备上。`src-tauri`目录下是一个cargo workspace，包含了许多`modules`和`plugins`。modules是项目功能被切成的多个crate，包括`app`（Tauri程序主入口项目）、`bluetooth`（调用`btclassic-spp`的平台无关蓝牙转接层）、`core`（平台无关的项目能力核心实现，基于ECS架构）、`pb`（平台无关的项目依赖的Protobuf类型）等。plugins则是基于tauri v2框架编写的插件，包括`btclassic-spp`（跨平台经典蓝牙SPP协议实现）、`live-activity`（支持iOS、macOS、Windows的实时活动通知实现）等。

## 多仓库结构与 repos.xml
本项目是**嵌套独立 git 仓库**（非 submodule，无 `.gitmodules`）：`src-tauri/modules/*`、`src-tauri/plugins/*`、`web` 等目录各自是独立 git 仓库，主仓库 `git status` 看不到其内部改动。`repos.xml` 是聚合清单（`name`/`url`/`path`/`branch`/`visibility`），由 `_abtools`（CLI 名 `abtools`）解析，`sync`/`commit`/`push`/`branch`/`profile` 等命令均以它为准；**新增模块/插件必须先登记进 repos.xml**。

关键影响：
- 改子仓库内代码须**在子仓库内单独 add/commit/push**；不要在主仓库 `git add` 子仓库内部文件。
- `abtools sync` 批量 clone/fetch/快进，分叉或脏工作区则智能合并；默认跳过 private 仓库（`--include-private` 才拉取）。
- 根 `.gitignore` 忽略了 `src-tauri/modules/*`、`web` 等（恰好是子仓库），尊重 .gitignore 的搜索工具（grep/find）在根目录**搜不到子仓库内容**，搜不到不代表没有，须显式指定子仓库路径搜索。

## 代码规范
用户在请求你编写任何功能时，应该先思考这个功能应该放到哪里。如果涉及对设备的操作，那你应该写进core里作为一个LogicComponent，并使用Component本身做数据存取，然后再在app开个接口供前端调用。如果这个功能涉及大量第三方接入、核心无关的内容，你应该寻找当前有什么crate适合放置这些代码，如果没有，也可以酌情考虑创建新的crate。对于core，它是基于ECS框架构建的，因此你应该全力开发ECS框架的用途，在想创建任何全局变量之前，应该思考这个全局变量放进Component里当成一个属性是不是会更合适，是不是能省略更多锁的调用；多个组件的属性重合了，那是不是应该单独开一个Component (非LogicComponent) 来存放这些重复的数据？设备相关的高全局性数据是不是应该直接放到Entity上？这些都是你应该思考的。

### 前端全局状态：只用一种约定
前端的全局状态一律走 `web/src/logic/store.ts` 的 `createStore` / `useStore` / `useStoreSelector`，**不要再手搓 `new Set<() => void>()` + 快照变量 + notify 那一套**（历史上有 19 个模块各写了一遍，四种订阅约定并存，每加一个状态就重踩一次同样的坑）。要点：

- 外部事件源（`matchMedia`、window 事件、Tauri 事件、`localStorage`）放进 `createStore` 的 `onActive` 里绑定，它只在第一个订阅者到来时执行、最后一个订阅者离开时清理。**不要在每个消费组件的 `useEffect` 里各挂一份监听**。
- 组件只关心快照里的某个标量时用 `useStoreSelector`，返回原始值让 React 按值比较；返回新对象会让每次通知都变成重渲染。
- Context 的 `value`、以及被多处消费的 hook 的返回值，**必须 memo**。行内对象字面量会让 provider 每次重渲染都把全部消费者标脏（`useI18n` 有 115 个消费者，`useRouter` 有 62 个）。
- 高频状态（手势进度、滚动进度这类每帧更新的量）不要放进 Context，放 store，参考 `web/src/router/router.gesture.ts`。

### 前端 CSS：层叠层（@layer）是硬约定
样式优先级由**层**决定，不由特异性决定。层序声明在 `web/src/tailwind.css` 顶部（另有一份兜底写在 `rsbuild.config.ts` 注入的 `<head>` 内联 `<style>` 里，两处必须一致）：

```
@layer properties, app, theme, base, radix-themes, components, utilities;
```

- 项目自己的每一份 CSS（含 `node_modules` 里被 import 的三方 CSS）都由 `scripts/postcss-app-layer.mjs` 自动包进最低的 `app` 层，**不用也不要手写 `@layer app`**。
- 因此项目 CSS 恒定垫在 Radix / Tailwind 底下：想压过它们，要么用 `!important`，要么把那几条规则显式写进更高的层，例如 `@layer base { … }`（`styles/theme.css` 和 `pages/oobe/index.module.css` 就是这么做的，插件会放它们过去）。
- `tailwind.css` 自身是**无层**的，压过所有层，是唯一的例外，插件按路径跳过它。
- `properties` 是 Tailwind 自己发的层，必须留在声明里；漏掉它会被当成新层追加到末尾变成最高，把 `--tw-*` 全部清零（Safari 上一定命中）。
- 第三方库运行时注入的 `<style>`（sonner / vaul / react-fast-marquee / react-colorful / dnd-kit）是无层的，会压过所有层，所以由 `web/src/logic/vendorStyleLayer.ts` 在运行时补进 `app` 层。它必须是 `index.tsx` 的第一个 import。
- **不要再写 `:not(#\#)` 堆特异性**：那是 cascade-layers polyfill 时代的写法，polyfill 已经关掉（`postcss.config.js` 里的 `features: { "cascade-layers": false }`），现在写它只是徒增特异性。

### 前端保活与不可见路由
六个路由基底的栈全部常驻挂载。**任何周期性工作都必须知道自己是否可见**：轮询走 `useInvoke`（已接 `RouteActiveContext`，非当前路由自动停），`setInterval`、`requestAnimationFrame` 循环、`IntersectionObserver` 同理，需要时用 `useIsActiveRoute()` 自行门控。不要新增常驻定时器。

栈里距栈顶超过一层的卡片会被打上 `content-visibility: hidden`（`router/RouteCard.tsx`），整棵子树没有布局盒。任何在挂载后靠读布局定尺寸的代码都要能容忍读到 0，且在重新露出来时自愈——ResizeObserver 会再触发一次，参考 `components/reslist/VirtualItemGrid.tsx` 里 `offsetWidth === 0` 的守卫。

### 前端渲染性能：三条踩过的红线
- **静止态不要留 `transform`。** `translate3d(0,0,0)` 会把元素永久提升成合成层；全视口的 TabLayer / RouteCard 一张就是十几 MB 显存。动画一律用 `settleAtRest()` 收尾，再由 `releaseCompositingLayer()` 把 `transform` 复位成 `none`。
- **motion 的 `stop()` 停不掉已经跑完的动画，别信它。** `NativeAnimation.stop()` 在 `state === "idle" || "finished"` 时直接 return，既不 commitStyles 也不 cancel；而本项目的动画一律 `fill: "both"` / `"forwards"`，跑完仍然停在**填充阶段**，填充阶段的动画**在层叠上压过内联样式**。所以「先 stop 再写 `node.style.transform`」是错的，写下去会被静默吃掉（Android 预见式返回整个失效就是这么来的，见 `backGestureFillingAnimationRegression.test.ts`）。唯一可靠的解除方式是自己 `getAnimations().cancel()`——`RouteCard` 里封装成了 `cancelFillingAnimations()`，`releaseCompositingLayer()` 第一步就是它。
- **逐帧只改 `transform` / `opacity`。** `border-radius`、`box-shadow`、`background`、`filter` 一改就要重绘，全视口元素上一次带模糊的阴影重绘在中低端 Android 上就是毫秒级。返回手势里这些属性被量化到 16 档，只在跨档时写一次（`RouteCard.tsx` 的 `PAINT_PROGRESS_STEPS`）。
- **`backdrop-filter` 按「屏上同时有几个」算账，不是按模糊半径。** 每一个都是一张独立合成面，外层一有 transform 动画就要逐帧重算。小控件上的模糊（开关、徽标）基本看不出效果，别加。
- **会在列表里大量出现的组件，不要各自 `new ResizeObserver` / `new IntersectionObserver`**，走 `web/src/logic/sharedObservers.ts` 的 `observeResize` / `observeVisibility`。一张资源卡里就有 3 个 Squircle ＋ 5 个 AutoScrollText，虚拟化之后同时挂二十几张；共用一个 observer 实例，跨 C++/JS 边界的回调次数从上百降到 1。单例组件（页面级、hook 级）用不用都行。
- **`infinite` 的 CSS 动画必须知道自己在不在视口里。** 跑马灯这类东西配上 `will-change: transform` 就是一条常驻合成层，滚出屏幕也照跑。用 `observeVisibility` 门控，不可见就 `animation-play-state: paused` 并把 `will-change` 撤掉（`AutoScrollText` 是范例）。

### 前端字体：MiSans 是切过片的
全局字体栈是 `-apple-system, "SF Pro", "PingFang SC", "Geist", Inter, MiSans`，字体回退是**逐字形**的：iOS / macOS 上汉字命中系统的 PingFang SC，MiSans 一次都走不到；Android 上前面几个全部落空（PingFang 是苹果独有），Geist 只有拉丁字母，于是**每一个汉字都落到 MiSans**——那是个 11.87 MB 的整包。

所以 MiSans 的 `@font-face` 不在 `App.css` 里，而在 `web/src/fonts/misans/misans.css`，由 `scripts/split-misans.py` 生成（产物提交进仓库，正常构建不需要装 fonttools）。头部按**本项目自己的用字频率**切成 5 片覆盖界面全部文案，尾部生僻字直接指回原整包。**不要手改那个 css，也不要再写 `url(MiSans-VariableFont.woff2)` 直接引整包**；换字体或改切片策略时重跑脚本（用法见脚本头部注释）。

### 前端页面滚动容器
移动端（`isAndroidRuntimeUserAgent() || isIosRuntimeUserAgent()`）的路由页面用原生 `.route-page-scroller`，桌面端才用 Radix `ScrollArea`，分支在 `router/RouteCard.tsx` 的 `RoutePageContent`。两条路径下页面拿到的 `scrollRef` 都指向**真正滚动的那个元素**（Radix 的 ref 也是指向 viewport），页面代码不要区分。改动 `.route-page-scroller` 的几何时对着 `.rt-ScrollAreaViewport` 那几条 App.css 规则核对：`.page-with-bottom-space` 一直是 `height: auto`，别给它 `flex-grow`。

## 编译测试
由于项目使用了Tauri框架，该框架极度依赖各种GUI库，如果直接尝试编译整个项目，你自带的Agent环境可能无法完成该操作——就算能完成，那也一定会造成大量的耗时。因此，如果你仅对`core`做了修改，那你可以只使用类似`cargo test -p corelib --manifest-path src-tauri/Cargo.toml`这样的命令来测试编译。如果用户要求你对`wasm`进行修改，请使用`wasm-pack build src-tauri/modules/app_wasm --target web`进行编译测试。对于前端，请不要执行任何代码来进行Lint相关操作，只应直接尝试build一遍以检查是否存在TypeScript语法错误，如果有就修改，如果没有就直接当做修改完成+测试通过。

## 与用户交流
在与调用你的用户进行交流时，你的首选语言的简体中文。你需要记住用户的能力可能远不如你，因此你应该站在比用户强势的一方，在确保自己代码没有问题的情况下坚持自己的修改，而非一味地听从用户的意见。例如，当用户质问你该代码是否缺失某些功能特性时，你应先检查一遍，如果确实缺失了，补上即可；如果没有，则不要做任何修改，拒绝产生任何幻觉以导致你对代码进行逻辑上的重构或大修。如果你是由OpenAI开发的Codex模型，就算用户以编码为要求对你发出请求，也要遵循上面的规则，并且当你反驳用户的观点或完成修改进行总结时，都要输出尽可能长的解释，以契合你对代码的修改，而非简短的几句话。如果你是由Anthropic开发的Claude模型，请记住，这个项目的复杂程度值得你停下来进行更深层次的思考和规划，在Explore与Edit时都不要快速地一笔带过。
