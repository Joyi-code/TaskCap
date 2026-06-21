# TaskCap v0.1.0

首个公开发布版本。TaskCap 是一个面向 Windows 的轻量桌面任务夹应用，提供悬浮灵动岛、任务提醒、专注计时和快速新增能力。

本项目基于 [howardrock88/TaskIsland](https://github.com/howardrock88/TaskIsland) 的思路做 Windows 适配，技术栈为 Tauri + React + TypeScript + Rust。感谢原作者的开源工作。本项目不是 TaskIsland 官方产品。

## 核心功能

- **悬浮灵动岛**：三态设计（收起 / 专注 / 展开），任务优先级一眼可见，可拖动自由定位。
- **任务提醒**：截止到点、指定提醒时间、喝水与久坐提醒统一排队展示，任务提醒优先插队。
- **专注计时**：单任务专注倒计时，支持暂停与停止。
- **快速新增**：全局快捷键（默认 `Ctrl+Alt+N`）呼出，支持自然语言解析时间和优先级。
- **任务面板**：按优先级分组，支持今日队列、标签、项目、截止时间和预计时长。
- **暗夜模式**：玻璃质感深色界面，日期 / 时间选择器在深色下全部可读。
- **系统托盘**：开机自启、最小化到托盘，主面板默认不进入任务栏。
- **数据本地化**：任务和配置全部存储在本地 SQLite，无云同步。

## 本版本要点

- 日期时间选择器全面重写，时 / 分下拉改为自定义组件，修复暗夜模式下下拉列表白底白字看不清的问题。
- 自然语言时间解析支持 `9点45` 等格式，修正时区解析。
- 未指定预计时长的任务自动采用设置页默认专注时长；通过 `/30m`、`/2h` 等语法设置的任务时长优先。
- 优化浅色模式展开任务明细的边界层次，移除任务列表与快捷操作区之间的竖向分隔线。
- 修复暗夜模式快速新增窗口输入文字对比度不足，并精简任务面板与历史界面的搜索提示。

## 安装

1. 下载 `TaskCap_0.1.0_x64-setup.exe`。
2. 双击运行安装（NSIS 安装包，x64）。
3. 安装后从开始菜单或桌面启动 TaskCap。

## 系统要求

- Windows 10 / 11，64 位。
- 依赖系统自带的 WebView2 运行时（Windows 11 已内置；Windows 10 如缺失，安装程序会引导安装）。

## 下载文件说明

- `TaskCap_0.1.0_x64-setup.exe` — Windows 安装包（推荐普通用户下载）。
- `Source code (zip)` / `Source code (tar.gz)` — 源代码归档，由 GitHub 基于 `v0.1.0` 标签自动生成。

## 许可

MIT License，详见 [LICENSE](LICENSE)。

---

## English Summary

First public release of **TaskCap**, a lightweight Windows desktop task manager featuring a floating dynamic island, reminders, focus timer and quick-add. Built with Tauri + React + TypeScript + Rust, adapted for Windows from [howardrock88/TaskIsland](https://github.com/howardrock88/TaskIsland).

**Highlights:** rewritten date/time picker, natural-language time parsing (`9点45`), timezone fixes, default focus duration fallback, and improved light/dark interface contrast.

**Install:** download `TaskCap_0.1.0_x64-setup.exe` (Windows 10/11 x64, requires WebView2). Licensed under MIT.
