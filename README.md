# TaskCap

TaskCap 是一个面向 Windows 的轻量桌面任务夹应用，提供悬浮灵动岛、任务提醒、专注计时和快速新增能力。

本项目基于 [howardrock88/TaskIsland](https://github.com/howardrock88/TaskIsland) 的开源项目思路做 Windows 系统适配，**感谢原作者 howardrock88 的开源工作**。原项目为 macOS 原生应用，本项目将其核心交互逻辑移植到 Windows 平台，技术栈全面替换为 Tauri + React + TypeScript + Rust，并加入 Windows 专属适配。

## About

TaskCap: Windows desktop adaptation of howardrock88/TaskIsland, built with Tauri, React, TypeScript and Rust as a lightweight floating task manager.

## 页面截图

![TaskCap 产品预览](screenshots/overview.png)

## 联系方式

### 微信交流

扫码添加微信（备注 GitHub）：

<img src="screenshots/wechat-qrcode.png" alt="微信二维码" width="240" height="290">

微信号：`pixel-cafetime`

微信公众号：像素与咖啡时光

抖音号：像素与咖啡时光

## 当前能力

- 悬浮灵动岛：三态设计（收起 / 专注 / 展开），任务优先级一眼可见，拖动自由定位。
- 任务提醒：截止到点、指定提醒时间、喝水和久坐提醒，统一排队展示，任务提醒优先插队。
- 专注计时：单任务专注倒计时，支持暂停与停止。
- 快速新增：全局快捷键（默认 `Ctrl+Alt+N`）呼出，支持自然语言解析时间和优先级。
- 任务面板：按优先级分组，支持今日队列、标签、截止时间和预计时长。
- 系统托盘：开机自启、最小化到托盘，主面板默认不进入任务栏。
- **数据本地化：任务和配置全部存储在本地 SQLite，无云同步**。

## 与原项目的关系

| 项目 | 原项目 TaskIsland | 本项目 TaskCap |
| --- | --- | --- |
| 目标平台 | macOS | Windows 桌面端 |
| 核心技术 | macOS 原生 Swift / SwiftUI | Tauri 2, React 18, TypeScript, Rust |
| 主要功能 | 浮动任务岛、提醒、专注计时 | 同上，并加入 Windows 专属适配 |
| 启动方式 | macOS 原生应用 | Windows 桌面应用（NSIS 安装包） |
| 与原项目关系 | 原版 | 移植适配，业务逻辑与交互模型对齐原版 |

## 系统要求

- Windows 10 或 Windows 11。
- Microsoft Edge WebView2 Runtime。Windows 11 通常已内置，Windows 10 如缺失需单独安装。
- Node.js 18+ 和 npm（开发环境）。
- Rust 1.77.2+，建议使用 MSVC 工具链（开发环境）。
- Visual Studio Build Tools，需包含 Desktop development with C++ 相关组件（开发环境）。

## 使用说明

### 快速新增任务

全局快捷键（默认 `Ctrl+Alt+N`）呼出输入框，支持自然语言：

```
明天10点开会 !高 #工作 /30m
后天下午3点 提交报告 #项目
每周五 18:00 发周报
今晚写日报 !低
```

**日期关键词**：`今天` `今晚` `明天` `明晚`

**时间格式**

| 输入示例 | 解析结果 |
| --- | --- |
| 10点 | 上午 10:00 |
| 9点45 / 9点45分 | 09:45 |
| 10:30 / 10：30 | 10:30 |
| 9点半 | 09:30 |
| 下午3点 / 晚上8点 | 15:00 / 20:00 |

不写上午/下午时，一律按上午（24 小时制 < 12）处理。需要下午请明确写"下午"或"晚上"。

**优先级标记**

`!高` 或 `!p1`（高）、`!中` 或 `!p2`（中）、`!低` 或 `!p3`（低）。不填默认中优先级。

**标签与项目**

`#标签名` 打标签，`/30m` 或 `/2h` 设置预计时长。

**专注时长**

- 创建任务时填写 `/30m`、`/2h` 等时长，任务详情和专注倒计时使用该任务的自定义时长。
- 未填写时长时，使用设置页中专注区域的默认时长；任务详情会显示该默认值。
- 修改默认时长后，所有未单独设置时长的任务同步使用新值，已经单独设置的任务不受影响。

---

### 灵动岛数字说明

灵动岛显示三个数字，分别对应高 / 中 / 低优先级未完成任务数量。

- **有今日队列任务时**：**只统计今日队列中的任务**
- **今日队列为空时**：**统计全部未完成任务**

因此，当你手动将一条任务加入今日队列后，灵动岛数字会从"全部任务统计"切换为"今日队列统计"，数字可能变小，属于正常行为。

---

### 今日队列

任务的截止日期设为今天，**不会**自动进入今日队列。需要在任务面板中手动点击"**加入今日**"，任务才会出现在今日队列并影响灵动岛计数。

---

### 提醒优先级

**任务截止提醒优先级高于喝水 / 久坐提醒。**若灵动岛正在显示喝水提醒，任务到点后会立即插队显示。任务提醒 60 秒后自动消失；喝水 / 久坐提醒不自动消失，需手动点击关闭。

---

### 设置页面

点击主面板右上角的设置按钮进入设置页面，主要选项如下：

| 设置项 | 说明 |
| --- | --- |
| 显示 | 控制灵动岛、菜单栏标题和暗夜模式。 |
| 灵动岛 | 调整顶部间距和界面透明度。 |
| 开机自启 | 控制登录 Windows 后是否自动启动 TaskCap。 |
| 提醒 | 分别启用喝水、久坐提醒，并设置 1 至 120 分钟的提醒间隔。 |
| 显示模式 | 设置提醒弹出时使用标准宽度 172px 或宽大模式 340px。 |
| 专注 | 设置 5 至 120 分钟的默认专注时长；任务单独设置的时长优先。 |
| 快捷键 | 点击当前组合键即可修改快速新增快捷键，按 Esc 取消录制。 |
| 数据 | 控制主面板后台预加载，并设置 15 至 300 秒的自动刷新间隔。 |
| 操作 | 支持刷新数据，导出 JSON、Markdown、CSV，导入 JSON 或 CSV。 |

---

## 安装说明

从 GitHub Releases 下载 `TaskCap_x.x.x_x64-setup.exe` 后直接运行安装即可。

安装时若 360 安全卫士或 Windows Defender 弹出拦截提示，选择"信任"或"允许本次运行"。**本软件无云同步、无网络请求，所有数据仅存储在本地。**

## 安装与开发

Windows 源码开发需要安装 Visual Studio Build Tools 2022，并勾选 `Desktop development with C++`。项目脚本会自动探测本机 VS Build Tools 安装位置，无需手动配置固定路径。

```powershell
git clone <your-repo-url>
cd taskcap
npm install
npm run tauri:dev
```

开发检查：

```powershell
npm run tauri:check
```

构建安装包：

```powershell
npm run tauri:build
```

Tauri 打包目标当前配置为 NSIS 安装包，产物位于项目平级的 `.build\taskcap-target\release\bundle\nsis\` 目录。

如果出现 `Visual Studio Build Tools not found`，请安装 Visual Studio Build Tools 2022，并确认已勾选 `Desktop development with C++` 组件。

## 数据存储

应用数据默认存储在：

```text
%APPDATA%\taskcap\taskcap.db    # 任务数据库（SQLite）
%APPDATA%\taskcap\config.json   # 应用配置
```

首次启动时，程序会自动创建目录、配置文件和 SQLite 数据库，无需手动初始化。安装包不包含开发者本机数据；同一台电脑升级或重新安装时，会继续读取该用户原有的数据。

**请不要提交上述文件，也不要把截图、日志或配置文件中的敏感内容公开。**

## 项目结构

```text
taskcap/
├── src/                         # React + TypeScript 前端
│   ├── lib/                     # 工具函数与 hooks
│   ├── styles/                  # CSS 样式
│   ├── windows/                 # 各窗口组件
│   │   ├── island/              # 灵动岛窗口
│   │   ├── panel/               # 任务面板窗口
│   │   └── quickadd/            # 快速新增窗口
│   ├── main.tsx                 # 应用入口
│   └── version.ts               # 版本号
├── src-tauri/                   # Tauri + Rust 后端
│   ├── src/                     # Rust 源码
│   ├── icons/                   # 应用图标
│   ├── capabilities/            # Tauri 权限配置
│   ├── tauri.conf.json          # Tauri 配置
│   └── Cargo.toml               # Rust 依赖
├── public/                      # 静态资源（图标、加载动画）
├── scripts/                     # Windows 开发脚本
├── screenshots/                 # 产品截图
├── package.json                 # 前端依赖与脚本
└── README.md                    # 项目说明
```

## 不应提交的文件

仓库已通过 `.gitignore` 忽略以下内容：

- `node_modules/`
- `dist/`
- `src-tauri/target/` 和构建输出目录
- `.env`, `.env.local`, `.env.*.local`
- `*.log`, `*.err.log`, `*.out.log`
- 根目录开发截图和调试文件
- 本地启动验证截图 `verify-startup/`
- IDE 配置和系统临时文件

## 依赖

前端运行依赖：

- React 18
- React DOM 18
- Tauri JavaScript API 2
- lucide-react

前端开发依赖：

- Vite 5
- TypeScript 5
- Tauri CLI 2
- React 类型定义

Rust 后端依赖：

- tauri 2.11，启用 tray-icon, image-png
- tauri-plugin-log
- tauri-plugin-single-instance，单实例守卫，防止应用重复多开
- tauri-plugin-global-shortcut，全局快捷键
- tauri-plugin-notification，系统通知
- tauri-plugin-dialog，文件对话框
- rusqlite（bundled），本地 SQLite 存储
- serde / serde_json
- chrono / uuid / regex / log

## 更新日志

完整发布记录见 GitHub Releases。

### v0.1.1

- 修复快速新增输入 `每天10点写日报` 一类内容时，元数据删除顺序错误导致 UTF-8 中文字符边界 panic 和应用退出的问题。
- 日期、时间、重复规则等元数据区间统一按原始位置排序，再从右向左删除，并新增对应 Rust 回归测试。
- 应用、前端、Cargo 与 Tauri 配置版本号统一更新为 `0.1.1`。
- 已通过 `npm run verify`、Rust 11 项测试、前端生产构建和 Windows 安装实机验证。
- 安装包：`TaskCap_0.1.1_x64-setup.exe`。

### v0.1.0

- 首个正式发布版本，提供悬浮灵动岛、任务提醒、专注计时、快速新增、任务面板、系统托盘和本地 SQLite 存储等完整能力。
- 灵动岛三态（收起 / 专注 / 展开），任务提醒统一队列，任务提醒优先级高于喝水 / 久坐软提醒。
- 任务未指定预计时长时自动采用设置页默认专注时长，任务自定义时长优先。
- 优化浅色模式任务明细边界和暗夜模式快速新增输入文字对比度。
- 安装包：`TaskCap_0.1.0_x64-setup.exe`。

## 许可证

本项目使用 MIT License，与原项目 README 中声明的许可证保持一致。详见 [LICENSE](LICENSE)。

## 免责声明

本项目仅用于学习和研究目的。原版产品功能和交互设计版权归原作者所有。本项目仅做平台移植，不用于商业用途。
