# Fob

Fob 是面向 Windows 10 和 11（x64）的性能监视器。它是 Abdenasser 的
[NeoHtop](https://github.com/Abdenasser/neohtop) 的分支（MIT，见
[LICENSE](LICENSE)）。Fob 在 NeoHtop 的进程监视器之上，增加了卡片仪表盘、
托盘迷你窗口、状态栏、硬盘空间分布图，以及可选的传感器助手。

Fob 此前名为 winperf。Rust crate 和部分内部名称仍使用 `winperf`。
随包发布的程序都叫 Fob：`fob.exe`、`fob-sensors.exe` 和 `fob-mft.exe`。

当前版本：0.8.4（见 [CHANGELOG.md](CHANGELOG.md)）。Windows 压缩包
`fob-v0.8.4-x86_64-windows.zip` 在
[v0.8.4 发布页](https://github.com/voigtkampff/fob/releases/tag/v0.8.4)。
要从源码构建，见[构建 Windows 压缩包](#构建-windows-压缩包)。

> 截图：待补。本仓库中的 `screenshot.png`、`screenshot-light.png` 和
> `docs/screenshots/` 里的图片是 NeoHtop 的，不是 Fob 的。

## Fob 显示什么

主窗口有三个标签页：**仪表盘**、**进程**和**空间分布**。

- **仪表盘。** 卡片包括 CPU、GPU、内存、系统盘可用空间、网络、温度与风扇，
  以及按内存排序的进程。点击卡片打开对应的详情：
  - CPU：各线程占用和进程排行。
  - GPU：各引擎占用和 GPU 进程排行。
  - 内存：占用和已提交（commit charge）。
  - 硬盘：全部硬盘。
  - 网络：各应用网速和网卡。
  - 温度与风扇：全部传感器。
- **CPU 核心分组（0.8.4）。** 在混合架构 CPU 上，CPU 视图会按 Windows
  为每个核心报告的效率等级，把占用拆成性能核和能效核。Intel CPU 显示
  P 核和 E 核。AMD 和其他厂商显示性能核和能效核。每组是组内线程的平均占用，
  所以一组不会超过 100%。只有一种核心的 CPU 看起来和以前一样。
- **GPU 占用。** Fob 按适配器读取 Windows PDH 计数器
  `\GPU Engine(*)\Utilization Percentage`，并像任务管理器一样显示最忙的引擎。
  NVIDIA 显卡还会通过 NVML 读取。
- **进程。** 即 NeoHtop 的进程表，另有以字节/秒计的**硬盘读取**和
  **硬盘写入**列、**GPU %**，以及可选的**专用显存**列（0.8.3）。
  「—」表示 Fob 没有读数，绝不表示 0。各列的来源和限制见
  [docs/process-disk-gpu.md](docs/process-disk-gpu.md)。
- **空间分布。** 用矩形树图显示硬盘里被什么占满。选择一块硬盘，按**扫描**。
  点击文件夹进入其中。右键一项并选择**在资源管理器中显示**。Alt+Left 和
  Alt+Right 可后退和前进。只有在你按**扫描**时，Fob 才会扫描硬盘。
- **托盘迷你窗口。** 左键单击托盘图标，打开宽 360 px、带有相同卡片的面板。
  失去焦点或按下 Esc 时会收起，除非你钉住它。拖动标题栏可移动位置。
  双击标题栏会回到托盘旁。
- **状态栏。** 一条始终置顶的读数条，默认关闭。鼠标点击会穿透。按住 Ctrl
  可拖动。你可以选择显示哪些读数及其顺序，停靠在顶部、底部或自由位置，
  并设置不透明度和主题。它不挂钩到游戏，因此不能显示帧率。
- **全局快捷键。** 可以为迷你窗口、状态栏和完整窗口分别设置显示或隐藏的
  快捷键。三项都默认关闭，直到你设置为止，且每项都需要带 Ctrl、Alt 或 Win。
- **中文和英文。** Fob 跟随 Windows 显示语言。你可以在设置或托盘菜单中更改。
- **无遥测。** 已移除 NeoHtop 的用量上报。Fob 不会把你的系统信息发到本机以外。

尚未实现：电池、健康评分和网速测试。

## 运行 Fob

Fob 是一个便携文件夹，没有安装程序。

1. 解压 `fob-v<version>-x86_64-windows.zip`。
2. 将 `sensors` 和 `PawnIO` 文件夹保留在 `fob.exe` 旁边。
3. 运行 `fob.exe`。

默认情况下，Fob 在托盘中启动。关闭主窗口后，Fob 仍在托盘中运行。
要退出，请用托盘菜单里的**退出**。Fob 没有原生菜单栏。标题栏菜单里有
**设置…**、**关于 Fob**和**退出 Fob**。Ctrl+, 打开设置。

`fob.exe` 使用 Windows 10 和 11 自带的 WebView2 运行时。它以普通用户权限运行。

## 启用温度、风扇和各应用网速读数

没有传感器助手时，只有 NVIDIA 显卡会报告温度和风扇转速。可选的只读传感器助手
会在不提升 Fob 权限的情况下，增加 CPU 和主板传感器、各应用网速，以及
**空间分布**的 MFT 扫描。

要安装它，请在**温度与风扇**卡片上点击**启用完整温度检测**，或打开
**设置 › 常规 › 传感器助手**并点击**安装**。Fob 会先列出步骤。
在你按下**继续**并批准一次 Windows 权限（UAC）提示之前，Fob 不会安装任何东西。
随后 Fob 会执行以下操作：

1. 安装 PawnIO 驱动（v2.2.0，由 namazso 签名），只在尚未安装时才会安装。
   PawnIO 会打开自己的安装窗口。
2. 把助手复制到 `C:\Program Files\Fob\sensors`。复制前，Fob 会用编译进
   `fob.exe` 的 SHA-256 哈希校验每个文件。
3. 通过任务计划程序 API 创建计划任务 `fob-sensors`。该任务会立即以管理员
   身份启动助手，并在你每次登录时自动启动，不再询问。

`fob.exe` 本身仍以普通权限运行。助手通过命名管道 `\\.\pipe\fob-sensors`
提供传感器读数。它用 LibreHardwareMonitorLib 0.9.6 读取温度和风扇转速，
从不改变风扇转速。

**设置 › 常规 › 传感器助手**会显示已安装版本和随附版本，并提供**安装**、
**更新**、**修复**或**刷新状态**。**刷新状态**不会请求权限。要移除助手，
选择**停用…**。它会在一次权限提示后停止并移除助手。PawnIO 驱动会留在系统中。
如果没有其他程序需要它，可在 Windows 设置中卸载。

> **隐私说明。** 助手运行期间，同一 Windows 用户会话中的程序可以查询整块
> 固定 NTFS 硬盘的文件名、大小和文件夹结构，包括你无权打开的文件夹。
> 助手不返回文件内容，也不写入文件。

### 空间分布如何扫描硬盘

1. 如果助手已安装且为当前版本，助手读取 NTFS 主文件表（MFT）。
   扫描不会请求权限。
2. 如果助手缺失、过旧或不兼容，在 Windows 允许时，Fob 直接读取 MFT。
3. 否则 Fob 遍历文件夹。

结果会标明所用方法，例如 **MFT 助手扫描** 或 **文件夹扫描**。
详情见 [docs/mft-helper.md](docs/mft-helper.md)。

## 设置

Fob 会自动把设置保存到应用配置文件夹中的 `settings.json`
（Tauri 应用标识符 `com.hcb.fob`）。默认值来自
[src/lib/definitions/default-config.json](src/lib/definitions/default-config.json)。
在设置窗口中更改设置。以下 `behavior` 默认值是你最可能改动的：

| 键 | 默认值 | 含义 |
| --- | --- | --- |
| `refreshRate` | `3000` | 刷新间隔，单位毫秒，适用于主窗口和迷你窗口 |
| `language` | `"auto"` | `auto` 跟随 Windows |
| `startInTray` | `true` | 仅以托盘图标启动 |
| `keepRunning` | `true` | 关闭主窗口后仍在托盘中运行 |
| `launchAtLogin` | `false` | 登录 Windows 时打开 Fob |
| `miniOpacity` | `100` | 迷你窗口背景不透明度，单位为百分比 |
| `overlayEnabled` | `false` | 显示状态栏 |
| `overlayMetrics` | `["cpu", "cpuTemp", "gpu", "gpuTemp", "ram", "net"]` | 状态栏读数，从左到右 |
| `overlayDock` | `"top"` | 状态栏位置：`top`、`bottom` 或 `free` |
| `overlayOpacity` | `60` | 状态栏不透明度，单位为百分比 |
| `miniHotkey`, `overlayHotkey`, `mainHotkey` | `""` | 全局快捷键，未设置 |

状态栏还可以显示 `vram`、`disk` 和 `power`（GPU 功耗）。

## 构建 Windows 压缩包

Fob 在 Linux 上用 `cargo xwin` 交叉编译到 Windows。构建过程不生成安装程序，
只生成一个独立的 exe。VC 运行时和 WebView2Loader 是静态链接的
（`.cargo/config.toml` 设置了 `STATIC_VCRUNTIME`）。

### 环境要求

- Node.js 22（`.nvmrc`）和 npm。
- Rust 1.88 或更新（`src-tauri/Cargo.toml` 中的 `rust-version`），以及
  `cargo-xwin` 和 `x86_64-pc-windows-msvc` 目标。
- [llvm-mingw](https://github.com/mstorsjo/llvm-mingw)，用于 `llvm-rc`。
  没有 `llvm-rc` 时，构建出的 `fob.exe` 没有清单、图标和版本资源。
  Windows 会拒绝启动它。
- .NET 8 SDK，用于构建传感器助手。
- 带 `pefile` 的 Python 3（`pip install pefile`）。
- `curl`，用于一次性下载固定版本的 PawnIO 安装程序。

### 构建完整压缩包

```bash
LLVM_MINGW=/path/to/llvm-mingw DOTNET=/path/to/dotnet scripts/package-windows.sh
```

`LLVM_MINGW` 的默认值是
`/workspace/tools/llvm-mingw-20261020-ucrt-ubuntu-22.04-x86_64`。`DOTNET`
的默认值是 `PATH` 上的 `dotnet`，然后是 `/workspace/tools/dotnet/dotnet`。

脚本会执行以下操作：

1. 用 `sensors-helper/build.sh` 构建传感器助手。该脚本会运行助手的线格式测试，
   构建 `fob-mft.exe` 和 `fob-sensors.exe`，下载并校验 `PawnIO_setup.exe`，
   并写入嵌入 `fob.exe` 的哈希清单。
2. 用 `npm run build` 构建前端。
3. 用 `cargo xwin build` 构建 exe。
4. 检查每个 exe 的清单、版本资源和图标。
5. 写出 `dist/fob-v<version>-x86_64-windows.zip` 和
   `dist/fob-v<version>-SHA256SUMS.txt`。

压缩包里是一个文件夹，包含 `fob.exe`、`sensors/`、`PawnIO/`、
`licenses/`、`LICENSE.txt`、`README.txt` 和 `SHA256SUMS.txt`。

### 只构建 exe

```bash
npm ci && npm run build    # frontend -> build/, embedded into the exe
cd src-tauri && cargo xwin build --target x86_64-pc-windows-msvc --release
# -> src-tauri/target/x86_64-pc-windows-msvc/release/winperf.exe (shipped as fob.exe)
```

请先把 `llvm-rc` 加入 `PATH`。

### 运行测试

| 命令 | 覆盖范围 |
| --- | --- |
| `npm run check` | Svelte 和类型检查 |
| `npm run test:release` | `node --test scripts/*.test.mjs` 和 metrics 测试 |
| `for t in tests/*.mjs; do node "$t" \|\| exit; done` | 前端脚本：仪表盘、i18n、文案、状态栏、矩形树图等 |
| `cd src-tauri && cargo test` | Rust 后端。在 Linux 上需要 webkit2gtk-4.1 和 gtk3 开发包。 |
| `cargo test --manifest-path tools/monitoring-harness/Cargo.toml` | 监控模块，不需要 GTK 和 WebKit 开发包 |
| `cargo test --manifest-path sensors-helper/mft-worker/Cargo.toml` | MFT worker |
| `npm run format:check` | Prettier 和 `cargo fmt` |

`sensors-helper/build.sh` 也会运行助手的 C# 线格式测试。

## 已知限制

- Microsoft Defender 曾在传感器安装过程中把 winperf 0.4.0 标记为
  `Behavior:Win32/Execution.A!ml`。Fob 0.5.0 及之后改了安装步骤，但 exe
  仍未签名，仍会提升一次权限并注册登录任务。见
  [docs-fob/defender-false-positive.md](docs-fob/defender-false-positive.md)。
- 各进程的**硬盘读取**和**硬盘写入**来自 `GetProcessIoCounters`。
  它们计入每一次读写调用，包括缓存读取，因此可能超过物理硬盘的实际读写量。
- 使用默认列时，进程表大约宽 1306 px，所以默认的 1280 px 窗口需要少量横向滚动。
- 部分 0.8.x 行为尚未在 Windows 上检查：各进程的 PDH GPU 数值，以及设置里
  助手的安装、更新和修复流程。见
  [docs/process-disk-gpu.md](docs/process-disk-gpu.md) 和
  [docs/helper-settings.md](docs/helper-settings.md)。

## 更多文档

- [CHANGELOG.md](CHANGELOG.md)：Fob 版本说明（0.8.x），以及它所基于的
  NeoHtop 1.3.0 说明。
- [docs/process-disk-gpu.md](docs/process-disk-gpu.md)：各进程的硬盘和 GPU 列。
- [docs/helper-settings.md](docs/helper-settings.md)：设置中的助手控件。
- [docs/mft-helper.md](docs/mft-helper.md)：通过助手的 MFT 扫描，
  以及安全边界。
- [tools/fob-perf/README.md](tools/fob-perf/README.md)：在 Windows 上采样
  Fob 的 CPU 和内存占用的只读工具（中文）。
- [.github/DEVELOPMENT.md](.github/DEVELOPMENT.md)：NeoHtop 的开发和发布指南。
  其中的发布步骤描述的是 NeoHtop 的多平台工作流，不是 Fob 的打包流程。

## 致谢与许可

Fob 基于 Abdenasser 的 [NeoHtop](https://github.com/Abdenasser/neohtop)，
以 MIT License 发布。见 [LICENSE](LICENSE)。

传感器助手捆绑了采用各自许可的第三方组件：LibreHardwareMonitorLib（MPL-2.0）、
PawnIO（GPL-2.0）、HidSharp 和 ntfs-reader。它们的许可文本在
[sensors-helper/licenses/](sensors-helper/licenses/)。其他第三方许可文本在
[licenses/](licenses/)。
