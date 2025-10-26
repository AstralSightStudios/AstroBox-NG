<p align="center">
    <img src="images/icon.png" alt="AstroBox 图标" width="64">
</p>
<h1 align="center">AstroBox</h1>
<p align="center">Rust 驱动的下一代可穿戴生态工具箱，聚焦第三方应用的安装、调试与分发。</p>
<p align="center">
    <a href="https://github.com/AstralSightStudios/AstroBox-Public">Legacy 版本</a> ·
    <a href="src-tauri">Rust Workspace</a> ·
    <a href="web">Web 前端</a>
</p>
<p align="center">
    <img src="https://img.shields.io/badge/rust-1.90.0%20+-orange.svg?style=flat-square" alt="Rust 1.90.0+">
    <img src="https://img.shields.io/badge/tauri-v2-lightgrey.svg?style=flat-square" alt="Tauri v2">
    <img src="https://img.shields.io/badge/license-AGPLv3-red.svg?style=flat-square" alt="license gpl-v3">
    <img src="https://img.shields.io/badge/pnpm-required-02ACFA.svg?style=flat-square" alt="pnpm required">
</p>

---

> 这是 AstroBox (Legacy) 的完全重构版本，AstroBox-NG (next-generation) 将继续演化为一个可插拔、跨平台、极速部署的穿戴设备强力辅助工具。

## 项目成就
该项目达成了多个“全球首个”，具体有：
1.	全球首个使用 Rust 语言 **同时在 Windows、macOS、Linux、Android** 等多平台上实现 **经典蓝牙 SPP 通信** 的项目，实现了系统级跨平台蓝牙协议统一。
2.	全球首个在 **PC 与 iOS 平台** 上成功实现 **小米穿戴设备连接及第三方资源安装** 的项目，打破了官方生态封闭限制，构建出开放互联的新范式。
3.	全球首个使用 Rust 语言实现 **基于 Vela 系统的小米穿戴设备蓝牙通信协议栈近 99% 完整还原** 的项目。
4.	全球首个将 **WIT + WebAssembly System Interface（WASI**） 驱动的插件系统 **深度集成进 Tauri 应用** 并投入生产环境的项目，开创了桌面与 WebAssembly 融合的新形态。

## 技术特性
1. 核心与平台无关，使用高可扩展性的ECS架构，支持多设备同时连接
2. App端基于Tauri框架，可在Windows、macOS、Linux、Android、iOS五大平台上运行
3. Core针对WebAssembly特别适配，支持浏览器端与单片机平台运行
4. 基于wit / wasi技术栈的插件系统，提供近乎原生级别的插件性能
5. 具有抽象IPC层的由Rsbuild + React构建的现代化Web前端

## 实机效果
暂未解禁

## 快速上手

### 环境要求
- Rust Toolchain 1.90.0+（需启用 2024 edition 与 resolver = "3"）
- Python 3
- Node.js 与 [pnpm](https://pnpm.io/)（**强制使用 pnpm**）
- Git（别让我发现你没装它）

### 克隆仓库
```shell
git clone https://github.com/AstralSightStudios/AstroBox-NG
cd AstroBox-NG
```

### 初始化工作区
```shell
python abtools.py init

# 拥有私有模块访问权限时：
python abtools.py init --private
```

## 日常开发命令

> 在执行任何编译操作前，请确保 Rust 工具链满足最低版本要求。

### Web UI（需要对应仓库访问权限）
```shell
pnpm run dev
```

### CoreLib
```shell
cargo test -p corelib --manifest-path src-tauri/Cargo.toml
```

### WASM（浏览器端）
```shell
# 初次使用需安装 wasm-pack
cargo install wasm-pack

wasm-pack build src-tauri/modules/app_wasm --target web
```

### Tauri App
```shell
python abtools.py dev --tauri
```

### WebAssembly Demo
```shell
python abtools.py dev --wasm
```
