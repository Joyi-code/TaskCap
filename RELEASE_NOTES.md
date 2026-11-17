# TaskCap v0.3.0

本版本新增应用内更新：发现新版本后在主界面直接提示，点一下就能在软件内完成升级，不用再去网页下载安装包。

## 新增内容

- 主界面顶部新增版本提示条：发现新版本时显示「发现新版本 vX.Y.Z」，可「立即更新」或「稍后」忽略该版本。
- 软件内更新：自动下载 GitHub Releases 里的安装包，校验 SHA256，静默安装后自动重启，全程不用手动操作。
- 升级只替换程序文件，**任务数据、配置、飞书凭证与同步状态都在 `%APPDATA%\taskcap`，不受影响**。
- 设置页「关于」新增：当前版本、自动检查更新开关、手动「检查更新」、上次检查时间与结果。
- 启动后 12 秒自动检查一次，之后每 6 小时一次；可关闭自动检查，关闭后应用只在手动点「检查更新」时联网。
- 更新失败会保留旧版本并记录原因，下次启动可在设置页看到。

## 升级安全说明

- 下载包来自 `https://github.com/Joyi-code/TaskCap/releases`，并用 GitHub API 返回的 SHA256 校验。
- 校验失败会删除下载文件、保留旧版本，不会执行安装。
- 安装使用 NSIS 静默模式，只覆盖程序目录；`taskcap.db`、`feishu.json`、`feishu-sync.json` 均不在程序目录内。
- 更新助手退出前会写 `%APPDATA%\taskcap\update-result.json`，用于下次启动时告知成功或失败。

## 发版防呆

- release 构建缺少 `custom-protocol` 时直接编译失败，不可能打出会连 `127.0.0.1` 的白屏包。
- `npm run tauri:build` 打包完成后自动运行 `scripts/check-release.ps1`，校验版本号并确认 exe 内嵌了前端资源；不通过则中断，不产出可上传的安装包。

## 发版约定（给维护者）

1. 版本号四处联动：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src/version.ts`。
2. 打 tag 为 `vX.Y.Z`，发布 GitHub Release，上传 `TaskCap_X.Y.Z_x64-setup.exe`。
3. 只需要这一个安装包，应用内更新会自动下载并静默安装；不需要额外上传便携版。
4. 若某个版本只上传了名字含 `portable` 的便携版 exe，而没有安装包，也会识别并直接替换程序文件。
5. 发布后在旧版本里点「检查更新」即可验证。

## 验证结果

- Rust 单元测试 21 项通过（新增版本比较、资产选择、仅替换 exe 的升级测试各 1 项）。
- 联网实测：`cargo test live_fetch -- --ignored` 成功拉到最新 release 并解析。
- 前端生产构建通过，`custom-protocol` 内嵌资源构建通过。
- 本机实机升级到 0.3.0 后，任务库 62 条与飞书配置完好。

---

# TaskCap v0.2.0

本版本新增可选的飞书多维表格同步。默认关闭，关闭时与 v0.1.1 一样完全离线；需要用飞书的人打开开关、填入自己的应用凭证即可接入。

## 新增内容

- 设置页新增「飞书同步（可选）」分区：启用开关、连接配置、立即同步、自动同步间隔。
- 双向同步：本地任务推送为飞书记录；飞书里把「状态」改成「已完成」会回写本地。
- 以「本地ID」为唯一键增量更新，重复同步不产生重复记录。
- 一键创建 `TaskCap任务` 表（20 个字段），无需手抄 Table ID；已有同名表直接复用。
- 设置页显示连接状态：绿色「已连接」/ 黄色「未连接」/ 红色「连接异常」/ 灰色「未启用」，
  已连接时展开明细（数据表名、Table ID、字段数、上次同步时间与摘要）。
- 新增「测试连接」：只读自检凭证、知识库 token 与目标表，不写入数据。
- 粘贴错 Table ID 时，「创建任务表」会按表名找到同名表并回写正确 ID，同时明确提示已覆盖。
- 凭证只写入本机 `%APPDATA%\taskcap\feishu.json`，源码不内置任何租户信息。

## 飞书配置说明（可选）

不配置飞书不影响任何离线功能。想接入的人按下面五步做，全程约 5 分钟。

### 第一步：拿 App ID 与 App Secret

1. 打开[飞书开放平台](https://open.feishu.cn/app)，用企业账号登录。
2. 「创建企业自建应用」，填名称（例如 `TaskCap 同步`）后创建。
3. 左侧「凭证与基础信息」，复制 `App ID` 和 `App Secret`。

### 第二步：开权限并发版

1. 左侧「权限管理」，搜索「多维表格」，勾选 `bitable:app`（多维表格读写）。
2. 左侧「版本管理与发布」→「创建版本」→ 提交发布。企业自建应用一般由管理员审核通过后生效。
3. 权限没生效前调用接口会返回权限不足。

### 第三步：把应用加进目标多维表格

1. 在飞书云文档里新建一个多维表格（名字随意）。
2. 打开该表格，右上角「⋯」→「添加文档应用」（或「协作者」）→ 搜索你的应用名 → 权限选「可编辑」。
3. 不加协作者，应用即使有权限也读不到这个表格。

### 第四步：从链接取 App Token

飞书里点表格右上角「分享 → 复制链接」，或直接复制浏览器地址栏，常见两种形态：

```text
https://xxx.feishu.cn/base/bascnAbCdEfGhIjKlMnOpQr?table=tblXxXxXxXxXxXxXxXx&view=vewxxxx
https://xxx.feishu.cn/wiki/bascnAbCdEfGhIjKlMnOpQr?table=tblXxXxXxXxXxXxXxXx&view=vewxxxx
```

- `/base/` 或 `/wiki/` 后面那段（`bascnAbCd...`）就是 **App Token** 要填的值。
- `?table=` 后面那段（`tbl...`）就是 **Table ID**，可留空，让 TaskCap 自己建表。
- 飞书新版把表格放在知识库里，分享出来是 `/wiki/` 形态。`/wiki/` 后面这串其实是**知识库节点 token**，
  和表格自身的 token 不是同一个；TaskCap 同步时会自动调接口换成真实 token，你照抄链接里的值即可。
- 不想手动拆链接：把整条链接粘到设置页「粘贴链接」，点「从链接解析」，两个值自动填好。

### 第五步：在 TaskCap 里填

1. 主面板右上角齿轮 →「飞书同步（可选）」→ 打开「启用飞书同步」。
2. 展开「连接配置」，填 `App ID`、`App Secret`、`App Token`。
3. 点「创建任务表」，程序会在该多维表格里建好 `TaskCap任务` 表并自动回填 `Table ID`（已有同名表直接复用）。
4. 点「保存配置」→「立即同步」。之后按设置的间隔自动同步（默认 15 分钟）。

### 四个关键字段对照

| 字段 | 从哪里取 | 示例 | 注意 |
| --- | --- | --- | --- |
| App ID | 开放平台 → 应用 → 凭证与基础信息 | `cli_a1b2c3d4e5f6` | `cli_` 开头，不是企业 ID |
| App Secret | 同上「应用密钥」 | 长串字母数字 | 只存本机，界面不回显 |
| App Token | 多维表格链接 `/base/` 或 `/wiki/` 之后 | `bascnAbCdEfGhIjKlMnOpQr` | 知识库链接里是节点 token，程序会自动换算 |
| Table ID | 链接 `?table=` 之后，或点「创建任务表」生成 | `tblAbCdEfGhIjKlMn` | 留空可自动创建 |

### 常见报错怎么排查

| 现象 | 原因 | 处理 |
| --- | --- | --- |
| 提示权限不足 / permission denied | 应用没加为该表格协作者，或 `bitable:app` 未开通/未发布 | 回到第二、三步补齐 |
| 提示 app_id 或 app_secret 无效 | 凭证填错或应用已停用 | 重新复制凭证 |
| 提示字段不存在（FieldNameNotFound） | Table ID 指到了别的表 | 清空 Table ID 后点「创建任务表」 |
| 同步成功但表里没数据 | 看到的是另一个多维表格或在其他视图 | 核对链接里的 App Token 与 Table ID |
| 飞书改了「已完成」本地没变 | 自动同步间隔未到；或该记录没有「本地ID」 | 点「立即同步」；手工新建的记录不回传本地 |

### 安全说明

- 凭证只写入本机 `%APPDATA%\taskcap\feishu.json`，不写入安装包、不进入代码仓库。
- 关闭「启用飞书同步」后应用立即停止所有网络请求。
- 想彻底断开，关掉开关后在飞书开放平台停用该应用即可。

## 兼容与默认行为

- 未开启或未填完配置时，应用不发起任何网络请求，行为和 v0.1.1 一致。
- 升级安装继续使用原有本地数据库与配置。
- 本地删除任务不会删除飞书记录；飞书里手工新建的记录（无本地ID）暂不回传本地。

## 验证结果

- Rust 单元测试通过（含 5 项飞书字段映射与同步判定测试）。
- 前端生产构建通过。
- Windows 实机验证：60 条任务首次全量写入飞书；飞书勾选「已完成」后本地 1 条任务正确回写。

## 安装

1. 运行 `TaskCap_0.2.0_x64-setup.exe`，支持 Windows 10 / 11 x64。
2. 升级安装会继续使用当前用户已有的本地任务数据库和配置。
3. 飞书同步为可选功能，不配置不影响任何离线能力。

---

# TaskCap v0.1.1

本版本修复快速新增任务时可能触发的自然语言解析崩溃问题，并完成 Windows 安装实机验证。

## 修复内容

- 修复输入 `每天10点写日报` 一类内容时，日期、时间和重复规则元数据删除顺序错误导致 UTF-8 中文字符边界 panic 和应用退出的问题。
- 元数据区间按原始位置排序，再从右向左删除，避免删除左侧内容后继续使用失效的字节位置。
- 新增 `parses_daily_time_before_chinese_title_without_panicking` 回归测试，确认标题解析为 `写日报`、重复规则为 `Daily`、提醒时间为 `10:00`。

## 验证结果

- `npm run verify` 通过。
- Rust 测试 11 passed，0 failed。
- 前端生产构建通过。
- 安装包 FileVersion 和 ProductVersion 均为 `0.1.1`。
- Windows 安装及核心功能实机测试通过。

## 安装

1. 下载 `TaskCap_0.1.1_x64-setup.exe`。
2. 双击运行安装，支持 Windows 10 / 11 x64。
3. 升级安装会继续使用当前用户已有的本地任务数据库和配置。

## 下载文件说明

- `TaskCap_0.1.1_x64-setup.exe`：Windows NSIS 安装包。
- SHA256：`6367B55EB78912D47489BA6A8DD46EFD6300591AD6EE088C30E7E3724B23C979`。
- `Source code (zip)` / `Source code (tar.gz)`：GitHub 基于 `v0.1.1` 标签自动生成的源码归档。

## 兼容性说明

部分将 Windows Terminal 设置为默认终端的环境，在 TaskCap 查询开机自启注册表时可能短暂显示 `reg.exe` 终端窗口。该问题不影响任务数据和主要功能，后续版本继续处理。

---

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
