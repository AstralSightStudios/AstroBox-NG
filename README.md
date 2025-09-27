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
为了更好地管理不同的模块，我们自己编写了一套Repo管理系统，称为abtools，这套工具同时也集成了项目初始化、编译、运行、打包等功能。