# AstroBox-NG
一款由 Rust 驱动、高度可扩展且便携的可穿戴设备工具箱，聚焦于在你的穿戴设备上安装和管理第三方应用程序。

这是旧项目[AstroBox (Legacy)](https://github.com/AstralSightStudios/AstroBox-Public)的完全重构版本

## 技术特性
1. 核心与平台无关，使用高可扩展性的ECS架构，支持多设备同时连接
2. App端基于Tauri框架，可在Windows、macOS、Linux、Android、iOS五大平台上运行
3. Core针对WebAssembly特别适配，支持浏览器端运行
4. 基于wit / wasi技术栈的插件系统，提供近乎原生级别的插件性能
5. 具有抽象IPC层的由Rsbuild + React构建的现代化Web前端

## 搭建开发环境
为了更好地管理不同的模块，我们自己编写了一套Repo管理系统，称为`abtools`，这套工具同时也集成了项目初始化、编译、运行、打包等功能。

**该项目强制要求使用[pnpm](https://pnpm.io/)，请不要使用其它包管理器安装依赖**

首先，请确保你的电脑上安装了这些工具：
1. [Rust Toolchain](https://rust-lang.org/)
2. [Python 3](https://python.org)
3. [pnpm](https://pnpm.io/)
4. [Node.js](https://nodejs.org/)
5. 别告诉我你没装Git

然后，克隆该仓库：
```shell
git clone https://github.com/AstralSightStudios/AstroBox-NG
```

接着，使用`abtools`拉取项目模块并初始化开发环境：
```shell
python abtools.py init

# 如果你具有私有模块的访问权限，请使用下面这条命令
python abtools.py init --private
```

## 调试
### WebUI
运行以下命令以启动AstroBox前端的开发服务器 (仅在拥有webui仓库访问权限的情况下可用，AstroBox 2.0正式版发布后会向所有人开放该权限)：
```shell
pnpm run dev
```

### CoreLib
运行以下命令以单独编译AstroBox的核心模块：
```shell
cargo test -p corelib --manifest-path src-tauri/Cargo.toml
```

### WASM
运行以下命令以编译AstroBox的wasm版本：
```shell
# 先执行这条命令安装wasm-pack，装过了就不需要再装
cargo install wasm-pack

wasm-pack build src-tauri/modules/app_wasm --target web
```

### Tauri APP
运行以下命令以启动本地开发服务器，并以Debug编译配置运行App版AstroBox (仅在拥有app模块访问权限的情况下可用)：
```shell
python abtools.py dev --tauri
```

### 网页版
运行以下命令以启动本地开发服务器，并以Debug编译配置运行基于WebAssembly的网页版AstroBox
```shell
python abtools.py dev --wasm
```