# AstroBox — Copilot instructions for AI coding agents

Short, actionable guidance so an AI agent can be immediately productive in this repository.

Principles
- Language: reply and edit in Simplified Chinese by default for user-facing text. Code may use English identifiers.
- Safety: do not run heavy GUI builds for the whole Tauri app in an agent environment. Prefer focused crate tests/builds.

Big picture (what to know quickly)
- This is a Tauri-based multi-crate workspace. Native/Rust code and frontend live together under `src-tauri/` and `src/`.
- Key Rust crates under `src-tauri/modules/`: `core` (ECS-based logic components), `bluetooth` (platform bridge using `btclassic-spp`), `pb` (Protobuf types), `app` and `app_wasm` (app entrypoints). Plugins live under `src-tauri/plugins/` (e.g., `btclassic-spp`, `live-activity`).
- Frontend is a React/TypeScript app in `src/` (uses `pnpm`, `rsbuild`, Tailwind). UI components live under `src/components` and `src/layout`.

Where to implement features
- Device-facing logic (BLE/SPP, device commands, state) -> `src-tauri/modules/core` as a LogicComponent (ECS pattern). Store persistent per-entity state in Components rather than globals.
- Platform-specific adapters -> `src-tauri/modules/bluetooth` or `src-tauri/plugins/btclassic-spp` depending on scope.
- Frontend UI/UX or API surfaces -> `src/` and expose Tauri commands in `src-tauri/modules/app` for the renderer to call.

Build / test / debug workflows (concrete commands)
- Install frontend deps: `pnpm install` (run from repo root).
- Frontend dev server: `pnpm dev` (serves at http://localhost:3000).
- Frontend build: `pnpm build`; preview: `pnpm preview`.
- Rust/core focused test: if you modify `core`, run:
  `cargo test -p corelib --manifest-path src-tauri/Cargo.toml`
- Wasm crate build: for `app_wasm` use:
  `wasm-pack build src-tauri/modules/app_wasm --target web`

Agent constraints and guidelines
- Do not attempt to compile the entire Tauri app or run full GUI builds in the agent environment — they're slow and brittle because of native GUI/toolchain dependencies.
- Prefer small, targeted builds/tests: crate-level `cargo test`, `wasm-pack` for wasm changes, `pnpm build` for frontend TypeScript syntax checks.
- If you change only frontend TypeScript, run `pnpm build` to detect syntax/type errors; only run linters if requested by a human.

Project-specific conventions
- ECS-first: `core` uses an Entity-Component-System. Before adding global state, prefer adding a Component or LogicComponent.
- LogicComponent pattern: device-facing operations should be implemented as LogicComponents in `core`. Use Components to store state and avoid global locks where possible.
- Crate placement: if a change impacts device comms or platform internals, prefer placing it in an existing crate under `src-tauri/modules/`. If none fit, it's acceptable to create a new crate and document it in `src-tauri/Cargo.toml`.

Files and code examples to look at
- Entry points / workspace: `src-tauri/Cargo.toml`, `src-tauri/modules/app/Cargo.toml`.
- Core ECS docs/examples: `src-tauri/modules/core/src/ecs/README.md` (contains patterns and helpers).
- Plugins: `src-tauri/plugins/btclassic-spp/README.md`, `src-tauri/plugins/live-activity/README.md`.
- Frontend start/build: `package.json`, `rsbuild.config.ts`, `src/index.tsx`, `src/pages`.

Communication & PR behavior
- When proposing code changes, output a clear summary of what was changed and why (1–3 short paragraphs) and list affected files.
- If a user questions whether functionality is missing, first search the repo for the relevant area; if truly missing, implement it; if present, explain why it exists and cite the file(s).
- For risky or wide-scope changes (cross-crate or UI + native), prefer leaving a design note in the PR rather than making large unilateral edits.

Examples of phrasing and edits
- Good: "在 `src-tauri/modules/core` 添加了一个 LogicComponent `DeviceSync`，用于将设备状态映射到 Entity 的 `DeviceState` Component。测试命令：`cargo test -p corelib --manifest-path src-tauri/Cargo.toml`。"
- Bad: "我会直接重构整个仓库去删除全局变量" — avoid broad refactors without design discussion.

Edge cases & diagnostics to check
- Changing ECS layout: ensure Components serialization and Protobuf mappings (see `src-tauri/modules/pb/`) remain compatible.
- Device protocol changes: check `src-tauri/modules/bluetooth` and `src-tauri/plugins/btclassic-spp` for platform differences.
- Frontend-only fixes: run `pnpm build` to catch TS errors; check `src/pages` and `src/layout` for routing/props patterns.

If uncertain
- When in doubt about where to implement a change, prefer creating a small crate under `src-tauri/modules/` and add a brief `README.md` and update `src-tauri/Cargo.toml`.

Ask for feedback
- I added this file from repository docs and READMEs. Tell me if you want more examples, stricter commit/PR rules, or added code snippets for common tasks.
