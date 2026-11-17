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
| 飞书同步（可选） | 默认关闭。开启并填入飞书应用凭证后，任务同步到飞书多维表格，飞书里勾选完成会回写本地。 |
| 关于 | 显示当前版本，提供「检查更新」与「自动检查更新」开关。 |
| 操作 | 支持刷新数据，导出 JSON、Markdown、CSV，导入 JSON 或 CSV。 |

---

### 应用内更新

发现新版本时，主面板顶部会出现一条提示：「发现新版本 vX.Y.Z」。点「立即更新」后软件自己完成下载、校验、安装和重启，不需要再去网页下载安装包。

- 检查来源：`https://api.github.com/repos/Joyi-code/TaskCap/releases/latest`，只读取版本与资产信息。
- 下载来源：GitHub Releases 的 `TaskCap_X.Y.Z_x64-setup.exe`，直接静默安装；不需要额外上传便携版（只有没传安装包、只传了名字含 `portable` 的 exe 时才走便携替换）。
- 完整性校验：使用 GitHub API 返回的 SHA256 摘要校验安装包，不匹配就删除并保留旧版本。
- 自动检查：启动 12 秒后检查一次，之后每 6 小时一次；设置页「关于」可关闭，关闭后只有手动点「检查更新」才联网。
- 镜像源：设置环境变量 `TASKCAP_UPDATE_API` 指向自建 JSON 接口即可换源（企业内网用）。
- 限流：GitHub 未登录接口每小时 60 次。正常情况下（缓存 10 分钟、自动检查 6 小时一次）不会触发；遇到了会提示大约几分钟后自动重试。需要更高额度时设置环境变量 `TASKCAP_UPDATE_TOKEN`（GitHub token），额度提升到每小时 5000 次。
- 忽略某个版本：点提示条右侧的 ×，该版本不再提示；更高版本仍会提示。

**升级不会丢数据**：任务库、配置、飞书凭证、同步状态全部在 `%APPDATA%\taskcap\`，不在程序目录，安装过程只覆盖程序文件。更新结果写在 `%APPDATA%\taskcap\update-result.json`，下次启动在设置页可见。

发版约定：

1. 四处版本号保持一致：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src/version.ts`。
2. Git tag 用 `vX.Y.Z`，Release 里上传 `TaskCap_X.Y.Z_x64-setup.exe` 即可，不需要额外传便携版。

---

### 飞书同步（可选）

不用飞书的用户不需要做任何事：开关默认关闭，应用不联网、不上传数据。开启后才访问飞书开放平台 API。

接入步骤：

1. 在[飞书开放平台](https://open.feishu.cn/app)创建企业自建应用，在「权限管理」开通 `bitable:app`（多维表格读写），再在「版本管理与发布」创建版本并发布，审核通过后权限才生效。
2. 在飞书里新建一个多维表格，右上角「⋯」→「添加文档应用」，把该应用添加为「可编辑」协作者。
3. 打开 TaskCap 设置页 →「飞书同步（可选）」→ 打开开关，展开「连接配置」，填入 `App ID`、`App Secret`、`App Token`。
4. 点「创建任务表」，应用会自动建好 `TaskCap任务` 表并回填 `Table ID`；也可手动粘贴已有表的 `Table ID`。
5. 点「立即同步」。之后按设置的间隔自动同步（默认 15 分钟）。

四个关键字段怎么填：

| 字段 | 从哪里取 | 示例 | 注意 |
| --- | --- | --- | --- |
| App ID | 开放平台 → 应用 →「凭证与基础信息」 | `cli_a1b2c3d4e5f6` | `cli_` 开头，不是企业 ID |
| App Secret | 同上「应用密钥」 | 长串字母数字 | 只存本机，界面不回显 |
| App Token | 多维表格链接 `/base/` 或 `/wiki/` 之后 | `bascnAbCdEfGhIjKlMnOpQr` | 知识库链接里是节点 token，程序会自动换算 |
| Table ID | 链接 `?table=` 之后，或点「创建任务表」生成 | `tblAbCdEfGhIjKlMn` | 留空可自动创建 |

链接有两种形态，都能用：

```text
https://xxx.feishu.cn/base/bascnAbCdEfGhIjKlMnOpQr?table=tblAbCdEfGhIjKlMn&view=vewxxxx
https://xxx.feishu.cn/wiki/bascnAbCdEfGhIjKlMnOpQr?table=tblAbCdEfGhIjKlMn&view=vewxxxx
```

`/base/` 或 `/wiki/` 后是 App Token，`?table=` 后是 Table ID。飞书客户端「分享 → 复制链接」给的是 `/wiki/` 形态；
这种链接里的 token 是知识库节点 token，TaskCap 同步时会自动换成底层表格的真实 token，不用你手动处理。

也可以直接把整条链接粘进设置页的「粘贴链接」，点「从链接解析」自动填两个值。

常见报错：

| 现象 | 原因 | 处理 |
| --- | --- | --- |
| 权限不足 / permission denied | 应用没加为表格协作者，或权限未开通/未发布 | 补第 1、2 步 |
| app_id 或 app_secret 无效 | 凭证填错或应用已停用 | 重新复制凭证 |
| 字段不存在（FieldNameNotFound） | Table ID 指到了别的表 | 清空 Table ID 后重新「创建任务表」 |
| 同步成功但看不到数据 | App Token 或 Table ID 对不上 | 按上面链接示意核对两个值 |

连接状态标识（设置页顶部）：

| 颜色 | 含义 |
| --- | --- |
| 绿色「已连接」 | 最近一次同步或「测试连接」成功；下方明细显示表名、Table ID、字段数、上次同步时间 |
| 黄色「未连接」 | 已启用但配置不全，或还没跑过首次同步 |
| 红色「连接异常」 | 最近一次同步/自检失败，具体原因在状态下方 |
| 灰色「未启用」 | 等同于完全离线，不发任何请求 |

「测试连接」只做只读自检（取 token、解析知识库 token、确认表存在、读字段数），不会写入或改动数据。

同步规则：

| 方向 | 说明 |
| --- | --- |
| 本地 → 飞书 | 新增任务自动建记录，任务字段变化自动更新记录，已完成任务显示为「已完成」（历史存档）。 |
| 飞书 → 本地 | 在飞书里把「状态」改成「已完成」，下次同步会把本地对应任务标记完成。 |

说明与限制：

- 飞书表字段：任务标题、本地ID、状态、优先级、项目、标签、截止日期、提醒时间、完成时间、创建时间、更新时间、预计分钟、专注分钟、推迟次数、子任务、备注、重复规则、今日清单、当前任务、同步时间。
- 以「本地ID」为唯一键做增量更新，重复同步不会产生重复记录。
- 本地删除任务不会删除飞书记录，避免误删；需要时在飞书里手动删除。
- 在飞书里新建的记录没有「本地ID」，暂不回传本地。
- 凭证只写入本机 `%APPDATA%\taskcap\feishu.json`，不进入仓库。

命令行辅助脚本（可选，不影响应用内同步）：

```powershell
python scripts/feishu/create_table.py      # 在指定 base 里建表
python scripts/feishu/verify.py            # 核对同步结果
python scripts/feishu/seed_config.py       # 从本机已有凭证写入 feishu.json
```

---

## 安装说明

从 GitHub Releases 下载 `TaskCap_x.x.x_x64-setup.exe` 后直接运行安装即可。

安装时若 360 安全卫士或 Windows Defender 弹出拦截提示，选择"信任"或"允许本次运行"。**应用默认无云同步、无网络请求，所有数据仅存储在本地。**只有你主动开启「飞书同步」并填入凭证后，才会访问飞书开放平台接口；关闭开关即恢复完全离线。

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

> 不要用裸 `cargo build --release` 代替：Tauri 2 只有在启用 `custom-protocol` 特性时才把前端资源内嵌进 exe，否则窗口会按开发模式去连 `http://127.0.0.1:5181` 并显示「拒绝连接」。项目已在 `Cargo.toml` 声明 `custom-protocol` 特性，等价命令是 `cargo build --release --features custom-protocol`。

发版防呆（两道，任一不满足都发不出去）：

1. **编译期拦截**：`src-tauri/src/lib.rs` 里有一条 `compile_error!`。release 构建缺少 `custom-protocol` 时直接编译失败，提示改用 `npm run tauri:build`，不可能产出白屏包。
2. **产物校验**：`scripts/check-release.ps1` 检查 exe 的版本号与 `package.json` 一致，并扫描二进制里是否真的内嵌了前端资源。`npm run tauri:build` 打包完成后自动调用它，`npm run verify` 也会调用；任一项不通过就中断，不会留下可上传的安装包。

单独校验某个产物：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/check-release.ps1 -Exe D:\TaskCap\taskcap.exe
```

Tauri 打包目标当前配置为 NSIS 安装包，产物位于项目平级的 `.build\taskcap-target\release\bundle\nsis\` 目录。

如果出现 `Visual Studio Build Tools not found`，请安装 Visual Studio Build Tools 2022，并确认已勾选 `Desktop development with C++` 组件。

## 数据存储

应用数据默认存储在：

```text
%APPDATA%\taskcap\taskcap.db        # 任务数据库（SQLite）
%APPDATA%\taskcap\config.json       # 应用配置
%APPDATA%\taskcap\feishu.json       # 飞书同步配置（含应用凭证，仅开启时生成）
%APPDATA%\taskcap\feishu-sync.json  # 飞书同步去重状态
%APPDATA%\taskcap\feishu.log        # 飞书同步日志（便于排查）
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
│   ├── src/                     # Rust 源码（feishu.rs 为可选飞书同步）
│   ├── icons/                   # 应用图标
│   ├── capabilities/            # Tauri 权限配置
│   ├── tauri.conf.json          # Tauri 配置
│   └── Cargo.toml               # Rust 依赖
├── public/                      # 静态资源（图标、加载动画）
├── scripts/                     # Windows 开发脚本（feishu/ 为飞书辅助脚本）
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
- 飞书同步本地凭证与状态（`feishu.json`、`feishu-sync.json`、`scripts/feishu/*.local.json`）
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
- ureq（仅飞书同步使用，默认关闭时不会发起请求）

## 更新日志

完整发布记录见 GitHub Releases。

### v0.3.0

- 新增应用内更新：主界面提示新版本，点「立即更新」自动下载、校验、静默安装并重启，无需手动下载安装包。
- 更新只替换程序文件，任务库与飞书配置位于 `%APPDATA%\taskcap`，升级不丢失数据。
- 设置页「关于」新增当前版本、检查更新、自动检查开关与上次检查结果。
- 新增 4 项更新相关单元测试（版本比较、资产选择、仅替换 exe、联网拉取）。

### v0.2.0

- 新增可选「飞书同步」：默认关闭，关闭时完全离线；开启后填写飞书应用凭证即可把任务同步到多维表格。
- 支持双向同步：本地任务推送为飞书记录，飞书里勾选「已完成」回写本地；以本地 ID 增量更新，不产生重复记录。
- 支持一键创建 `TaskCap任务` 表，无需手抄 Table ID；同步间隔 5 至 120 分钟可调。
- 凭证只存本机 `%APPDATA%\taskcap\feishu.json`，不写入仓库、不内置任何租户信息。
- 新增 5 项飞书字段映射与同步判定单元测试。

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
