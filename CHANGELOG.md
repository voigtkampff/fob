# Changelog

## 0.8.27

- 清理 Rust 后端与前端共 117 个文件的冗余叙述性注释（约 1800 行），只删注释、不改代码，类型注解与平台/协议说明保留。
- 修复两个孤儿测试 `tests/copy.mjs` 与 `tests/helper-settings-preview.mjs`：英文文案不再用句号分隔句子；helper 预览测试在 Windows 与新 Node 上可正常关闭 dev server。
- 透明度滑块改为拖动时本地预览、松手才落盘，不再每拖一格就重写 settings.json 并重排状态栏。
- 修复磁盘扫描结果竞态：先写结果再清 running 标记，避免前端在「清标记后、写结果前」的窗口轮询时漏掉新结果。
- 修复 `package-smoke` 冒烟脚本：探测 `pwsh` 并退回 `powershell`，磁盘加载检测改为语言无关的 DOM 判断。
- 删除删注释留下的一个空文档注释。

## 0.8.25

- 新增 `--show-all`，只影响本次运行并强制显示主窗口与状态栏，不改设置。
- 界面自检改用 `--show-all` 启动，不再依赖热键设置。

## 0.8.24

- 新增 Windows 界面自检脚本与文档：`npm run verify:ui` 使用 UI Automation 检查主窗口、页签、进程表和状态栏。
- 版本号 0.8.23 → 0.8.24，六处同步（`package.json`、`package-lock.json` 两处、`tauri.conf.json`、`Cargo.toml`、`Cargo.lock`）。
- 内置传感器助手 unchanged at 0.8.6

## 0.8.23

- 仓库新增 agent 说明文件 `AGENTS.md`，记录项目交付与验证规则。
- `.claude/` 本地 Claude Code 配置不再纳入版本控制。
- 版本号 0.8.22 → 0.8.23，六处同步（`package.json`、`package-lock.json` 两处、`tauri.conf.json`、`Cargo.toml`、`Cargo.lock`）。
- 内置传感器助手 unchanged at 0.8.6

## 0.8.22

- 修回 0.8.19 引入的回归：把 `mft_wire::frame` 标成测试专用导致 `fob-mft` 无法编译，发版打包会失败。现场是 tag `v0.8.21` 的 `Package (windows-x86_64)` 在 `scripts/package-windows.sh` 挂在 `error: could not compile fob-mft (bin "fob-mft") due to 4 previous errors`，note 指向 `src-tauri/src/mft_wire.rs:32` 那条 `#[cfg(test)] pub fn frame`；那个 tag 已删，没有生成草稿，没有发布任何东西
- 0.8.19 当时"一条 allow 都不加"的前提在"同一份源码被两个 crate 用 `#[path]` 复用"这个场景下不成立：`sensors-helper/mft-worker/src/main.rs` 用 `#[path = "../../../src-tauri/src/mft_wire.rs"]` 把本文件当自己的模块，并在生产路径（main.rs 第 43/45/54/55 行）调 `frame`。winperf 二进制里确实没有调用者，但那只是这一个 crate 的视角，不是这份源码的事实
- 修法：`frame` 去掉 `#[cfg(test)]`，恢复成普通 `pub fn`；只给这一个函数一条 `#[allow(dead_code)]`，注释写明 `sensors-helper/mft-worker` 通过 `#[path]` 包含本文件并在生产中使用 `frame`，只有 winperf 二进制看不到调用，并写明不要再把它塞回 `cfg(test)`。没有对模块或文件加 allow。原来那句"Test-only…"的文档注释一并改对
- 反方向的那一半也各自标靶：`MAX_STREAM`、`IDLE_SECS`、`TOTAL_SECS`、`Links`、`Links::push`/`finish`、`receive` 是 winperf 用的，`fob-mft` 只吃 `request`/`frame`，它们在那一边是死代码。6 个 item 各一条 `#[allow(dead_code)]`，注释指向同一段说明，仍然没有模块级 allow。`cargo clippy --manifest-path sensors-helper/mft-worker/Cargo.toml -- -D warnings` 从 6 条 error 变零输出
- 补一道门：`build-check.yml` 的 `Rust lints` job 新增 `cargo build --manifest-path sensors-helper/mft-worker/Cargo.toml`。`Package` 只在 tag 和手动时跑，所以 fob-mft 编坏了要等发版才发现；这一步是宿主直编，不需要 cargo-xwin 和 llvm，复用同一个 `Swatinem/rust-cache`，PR、push `main`、手动都会跑，`release.yml` 通过 `workflow_call` 复用时也会跑
- 这道门真能拦住的实测（本机，先红后绿，命令与 CI 那一步逐字相同）：改代码之前 `cargo build --manifest-path sensors-helper/mft-worker/Cargo.toml` 退出码 101，输出 `error: could not compile fob-mft (bin "fob-mft") due to 4 previous errors`，两处 `cannot find function frame in module mft_wire`（main.rs:54、55）并附 `note: found an item that was configured out --> src-tauri/src/mft_wire.rs:32`，与发版现场同一份报错；`frame` 恢复成 `pub fn` 之后同一条命令退出码 0、`Finished dev profile`
- 其余 clippy 保持零：`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` 零输出
- 实测：`cargo test --manifest-path src-tauri/Cargo.toml` 234 过 0 红 3 ignore；`npm run format:check`、`npm run check`（0 error 0 warning）、`npm run build:site`、`npm run test:release`、`npm run release:check`、`npm run build` 全过
- 本机出包（没有 cargo-xwin/llvm-rc，沿用 0.8.17 起的做法：`cargo build --release` 原生编出 exe，MSVC 的 rc.exe 把清单/图标/VERSIONINFO 嵌进去，复用已提交的 0.8.6 助手产物，再复刻 `package-windows.sh` 的 staging）：`dist/fob-v0.8.22-x86_64-windows-fix.zip`，8 444 674 字节，sha256 `719ab76686aa2ceb36459c49576b212017c6a361a694e2d1eb0dc71c62ac59d5`；包内 `sensors/fob-sensors.exe` sha256 `51e30c2ce1a474e6fcd820c355d3581655f17e44b07cfeaf6aa10c8e022c9b2f`，与 `fob.exe` 内嵌 manifest 里那一行一致（`check-sensors-bundle.py` 报 17 个传感器文件全对）；`fob.exe` 的 FileVersion 与 ProductVersion 都是 0.8.22，CompanyName HCB；`check-windows-exe.py` 对 `fob.exe`、`fob-sensors.exe`、`fob-mft.exe` 三个都通过
- `bash sensors-helper/build.sh` 在本机走不了：第一步 `dotnet run --project tests -c Release` 就报 `error NETSDK1045: The current .NET SDK does not support targeting .NET 8.0`（本机只有 SDK 6.0.410，仓库要 .NET 8）；补上 .NET 8 还会卡在 mft-worker 的 `cargo xwin build`（本机没有 cargo-xwin 和 llvm-rc）。它在 `rm -rf dist` 之前就退出，`sensors-helper/dist` 的已提交产物一个字节没动
- 版本号 0.8.21 → 0.8.22，六处同步（`package.json`、`package-lock.json` 两处、`tauri.conf.json`、`Cargo.toml`、`Cargo.lock`）
- 内置传感器助手 unchanged at 0.8.6

## 0.8.21

- 本机 `npm run test:release` 上那 4 条红在说谎：`doctor rejects a live process without its own visible window (empty)` / `(failed-search)`、`doctor accepts its live process and scoped window with logs absent` / `logs present` 报的失败原因是 `application is not running`，实际进程活得好好的，是本平台没有 X11 能力。现在它们在不具备 X11 时挂 `t.skip` 并写明原因，不再假红也不假绿
- `bin/fob-verify.sh` 的 `doctor` 先做能力检测：`xdotool` 是否在、`DISPLAY` 是否非空、`xdotool getdisplaygeometry` 能不能拿到答复。任何一项不成立就输出 `unsupported on this platform: the verification harness needs X11 and xdotool（原因）` 并退出 3，一次都不去看进程和窗口。退出码从此三档写死在脚本头：0 通过、1 合同失败、2 用法错、3 平台不支持
- 能力检测只靠 bash 内建完成（`command -v`、`[`、函数），`mkdir` 等外部命令都在它之后，所以 PATH 里连 `xdotool` 都没有时它也能把话说清楚，而不是先死在 `command not found` 上
- 进程存活判断改成双路：`kill -0` 仍是第一判据（POSIX 上照旧是真正的检查）；只有当 shell 完全不认得这个 pid 时才退到 `$H/alive`，那是 `launch`/`relaunch` 在看到窗口之后写下的确认，`stop` 会一并删掉。`doctor` 的输出写成 `alive pid N (kill)` 或 `alive pid N (launcher-confirmed)`，用哪条证据写在脸上，失败时仍然是真的失败
- 为什么不选别的：Git Bash 的 `kill -0` 和 `ps -p` 都只能看见 MSYS 自己起过的 pid，explorer 这类系统 pid 一律 `No such process`（本机实测 `kill -0 27376` 失败，与报错一致）；`tasklist` 认的是 Windows pid，而 `$H/pid` 里记的是 MSYS pid，两个号对不上，用它当判据反而会编造结论。剩下一路能真验证的是窗口：拿到 pid 的证据之后，`xdotool search --pid` 还必须找到这个 pid 自己的可见窗口，所以崩溃后残留的 `$H/alive` 过不了 doctor
- 测试分流按能力检测写，没写 `process.platform === 'win32'`：4 条合同测试跑之前用 `xdotool getdisplaygeometry` 探一次真实显示，探不通就 skip 并把原因写进 skip 文案（`needs X11 and xdotool: xdotool could not run (ENOENT)`），将来 Linux 机器缺 `xdotool` 也是同样的 skip，不会变成假绿
- 新增 2 条所有平台都跑的用例：PATH 里没有 `xdotool` 时 `doctor` 必须退出 3、输出那句不支持、一句 `application is not running` 都不能有；`xdotool` 在但 `DISPLAY` 上没有 X server 答复时必须同样退出 3，并且一次窗口搜索都没发生。本机这两条是真绿
- `.cursor/skills/verify-fob/SKILL.md` 补一句：这套工具要 X11 和 `xdotool`，缺了就退出 3；`Doctor` 一节说明 `alive pid N (…)` 括号里的证据含义。其余没动
- 实测：`npm run test:release` 从 240 条 236 过 4 红 0 skip 变成 242 条 238 过 0 红 **4 skip**，新增的 2 条在 Windows 上真绿；`cargo clippy -- -D warnings` 零输出，`npm run format:check`、`check`、`build:site`、`build`、`release:check` 全过
- 应用二进制一行没动，不出安装包
- 版本号 0.8.20 → 0.8.21，六处同步（`package.json`、`package-lock.json` 两处、`tauri.conf.json`、`Cargo.toml`、`Cargo.lock`）
- 内置传感器助手 unchanged at 0.8.6

## 0.8.20

- CI 接上 clippy：`build-check.yml` 新增 `Rust lints` job（`windows-2022`，只跑 clippy，不跑测试，PR、push main、手动都能触发），本地用 `npm run lint:rust` 跑的是同一条命令 `cargo clippy -- -D warnings`。0.8.19 修的 15 条问题当初就是在 main 上静默存活了很久，只因为这条检查只在本机跑
- 放在 Windows 上，没往已有的 ubuntu `Source checks` 里加：那 15 条里有几条是只在 Windows 上才出现的死代码，Linux 编译根本看不见。反过来加了也白加 —— 同一份 main 在 GitHub Actions 的 `ubuntu-22.04` runner 上跑（本 PR 首次 CI，run 44881247806 的 `Source checks`）报了 60 条：`disk_health/parse.rs` 33、`disk_health/mod.rs` 17、`gpu/mod.rs` 3、`autostart.rs` 3、`monitoring/cpu_clock.rs` 2、`space.rs` 1、`monitoring/accounts.rs` 1，全部是只被 `cfg(windows)` 调用的函数、常量、枚举变体被判 `never used`，另加一条 `window-vibrancy` 未使用依赖的 warning。这些不是缺陷，是 Windows-only 应用保留 Linux 可编译外壳的结果，要清掉得把几十个条目改成 `cfg(windows)` 或删 Linux 分支，属于另一个 PR。本机没有 Linux 环境（WSL 没装发行版、没有 Docker），所以这个数是 CI 日志里的，不是本机实测
- `Source checks` job 这次一行没改：`cargo fmt --check` 本来就在 `format:check` 里，格式早有 CI 盖住；clippy 是 `-D warnings`，没有降级成 warn，也没有加 `#[allow]`，main 上零输出
- clippy 步骤前面必须有 `npm run build`：`generate_context!` 在 check 阶段也会读 `frontendDist`（`../build`），缺这个目录直接 proc macro panic，本机实测过
- 实测首次跑（缓存未命中）整轮 3 分 33 秒：`npm ci` 11 秒、`npm run build` 34 秒、clippy 1 分 59 秒。远低于 10 分钟，所以单独开 job，没有并进只在手动和发版时跑的 `windows-tests` —— 并进去等于 PR 上永远不跑这条检查
- 版本号 0.8.19 → 0.8.20，除 CI、`package.json` 的 `lint:rust`、README 测试表一行和本 CHANGELOG 之外没有别的改动，应用代码一行没动
- 已知遗留，本次不动：本地 rustc 是 1.100.0，CI 用 `dtolnay/rust-toolchain@stable`，两者会漂移，新 stable 带来的新 lint 可能让 CI 突然变红。没用 `rust-toolchain.toml` 钉版本，因为 `package` job 靠 cargo-xwin 交叉编译到 windows-msvc，钉住版本会一起改那条链
- 内置传感器助手 unchanged at 0.8.6

## 0.8.19

- 本机 `cargo clippy -- -D warnings` 的 15 条报错全部修掉，这条检查重新有意义（此前长期红着，等于没有检查）。一条 `#[allow]` 都没加，全部改代码本身
- 三个 GPU 来源常量 `linux-amdgpu`、`linux-drm`、`macos-ioreg` 在 Windows 的非测试构建里确实没有消费者，但 `gpu/linux.rs:102/104`、`gpu/macos.rs:137` 在用，跨平台测试也拿它们当反例。所以补 `cfg(any(target_os = "linux", target_os = "macos", test))`，一个没删，Linux 和 macOS 的构建路径不变
- `mft_wire::frame` 同理：生产路径里应答帧由提权助手写，Rust 侧只负责 `receive`，这个函数只在测试里造 fixture。改成 `#[cfg(test)]`，`Write` 的 import 一并收进测试；常量 `MAX_FRAME` 仍由 `receive` 用，没动
- 其余按 clippy 的原意改，没有绕：`boot.rs` 两处多余的 `u32` 转换和点号列表段落缩进、`sensors/setup.rs` 的文档段落、`space.rs:594` 与 `space_mft.rs:93` 各一处多余的 `return`（`#[cfg(windows)]` / `#[cfg(not(windows))]` 两条分支的取值方式不变）、`overlay.rs` 的 `tick % 40 == 0` 换成 `tick.is_multiple_of(40)`、`companion.rs` 任务栏边界的 `ABE_BOTTOM | _` 改为 `_`（`ABE_BOTTOM` 与今后任何新边界都落到下边，行为不变）并删掉随之无用的 import、`hotkey.rs:260` 的槽位类型抽成 `type Slot`
- `overlay.rs` 里那条扫描源码的测试跟着新写法改了匹配串，其余测试一条没动；`cargo test` 234 passed，`cargo clippy -- -D warnings` 零输出
- 已知遗留，本次不动：`cargo clippy --all-targets` 另报 4 条，都在测试代码里（`space.rs:1146` 的 `child`、`companion.rs:1330` 与 `monitoring/system_monitor.rs:311` 的 `items_after_test_module`、`sampling.rs:812` 的 `clone`），待确认后再单开。CI 的 `build-check.yml` 里仍然没有 clippy 步骤，这次只恢复本机信号
- 内置传感器助手 unchanged at 0.8.6

## 0.8.18

- 删掉两个从未被调用的工具函数：`formatPercentage` 和 `getUsageClass`。整个仓库（`src`、`scripts`、`tests`、Rust 侧）一次都没有引用过它们，界面上的百分比和占用颜色分级都由各自的组件直接算；同文件里 `formatBytes`、`formatDate`、`debounce` 等导出逐个确认仍在用，一个没动
- 补上 `打开文件位置` 的测试：为了能直接用 `node --test` 跑，把菜单里那段异步流程抽成 `src/lib/utils/process-menu.js` 里的三个纯函数（查路径结果的落地、过期响应的判定、点下去之后的 reveal），组件只调用它们，交互行为不变。新用例盖住三件事：拿不到路径时这一项置灰并在第二行写明原因、且不会去调 `space_reveal`；菜单已经关掉或换到别的行之后才回来的响应被丢弃，不会把状态写回菜单；reveal 失败时菜单保持打开，同一项变成 `资源管理器没能选中它` 而其余项照旧可用
- 补上 Rust 命令层的测试：`process_location` 的命令体抽成 `location_of`（只剩取快照锁和身份查询两步），可以直接对着真实的 `AppState` 跑。用例覆盖不存在的 pid 得到 `Gone` 而不是 `Err` 或 panic、同一个 pid 配上不匹配的启动时间也得到 `Gone`（PID 被复用不会揭示错的文件），以及自己的进程按启动时间能查到绝对路径
- 内置传感器助手 unchanged at 0.8.6

## 0.8.17

- 进程行的右键菜单多了一项 `打开文件位置`，排在 `复制 PID` 之后、分隔线之前：点它在资源管理器里打开该进程 exe 所在的文件夹并选中那个文件，和系统的“打开文件所在的位置”一致
- 路径由后端按 PID 加启动时间查，前端不传路径也不存路径；读不到路径时这一项置灰，并在第二行直接写明原因：非提权的 Fob 读高权限进程和系统进程会拿到 `权限不足，读不到此路径`，进程已经退出则是 `此进程已结束`
- 因为路径是异步查的，菜单刚打开时这一项先置灰并写 `正在查询路径`，拿到结果后要么启用，要么换成上面两种原因之一；置灰时键盘和鼠标都点不动，不会出现点了没反应
- 点下去之后资源管理器没能选中那个文件（文件已删除、或 Explorer 没起来）时菜单不关闭，同一项变成 `资源管理器没能选中它`，原因就在指针旁边，不用去找弹窗
- 没有第二套实现：取 exe 路径复用图标用的那份进程身份查询（PID 加启动时间），打开资源管理器复用 `space_reveal` 已有的 `explorer.exe /select` 与“不存在 / 拒绝访问 / 启动失败”三种错误处理
- 内置传感器助手 unchanged at 0.8.6

## 0.8.16

- 进程列表的每一行都能右键：在进程名、任意数值单元格或行内空白处按右键都会弹出操作菜单，菜单里只有行上原本就有的动作，没有新增能力 —— `查看详情`、`置顶` / `取消置顶`、`复制进程名称`、`复制 PID`，分隔线之后是红色的 `结束进程`
- 菜单以指针为锚弹出，贴到窗口右边或底部时自动翻到指针的左侧或上方，不越出窗口；进程表本身在滚动容器里，菜单挂在窗口最上层所以不被裁切，滚动表格、缩放窗口或点到别处就收起
- 键盘也能弹出：行有焦点时按 `Shift+F10` 或菜单键打开，`Escape` 关闭并把焦点交回该行，`Tab` 和上下方向键在菜单项之间移动，打开时第一项直接获得焦点
- 右键不再和行上的原有操作互相干扰：不会再顺带打开详情弹窗，也不会选中文字；双击进程名展开/折叠、行末三个图标的行为和 0.8.15 一致
- 菜单文案跟随固定状态：未固定时写 `置顶`，已固定时写 `取消置顶`，与行末图标和详情弹窗共用同一组字符串
- 内置传感器助手 unchanged at 0.8.6

## 0.8.15

- 恢复到 0.8.12 的卡片拖拽判定：向左向右都不再需要把指针上下对齐。落点看指针在目标的哪一半，同一行的卡片按左右分，换行时按目标所在的那一行选，和 0.8.12 逐行一致；0.8.13 引入、0.8.14 靠横向补偿绕过的“纵向偏移也算前后”不再参与卡片
- 原因写进代码：`vertical` 有两种含义被混用了。卡片会换行，所以要跟随指针上下走、算距离时也算纵轴；但“插前还是插后”仍应只看同一行时的左右。0.8.13 为设置页的纵向列表把它改成“只要纵向就一律用纵轴”，卡片也传了 `vertical`，于是被误伤 —— 指针在卡片头部（中线以上）时向左能拖、向右拖不动
- 设置页的纵向块列表改用自己显式声明的 `stacked`，只有它为真才按上下判定；设置页图标槽位和表格表头不再需要额外的横向开关，本来就按 `rowMatch` 走左右；状态栏的指标卡片同样回到 0.8.12 的左右判定
- 内置传感器助手 unchanged at 0.8.6

## 0.8.14

- 卡片拖拽按左右半边决定落点：指针在目标卡右侧就落在它后面，在左侧就落在它前面，不用先把鼠标移到卡片下半部分；最左和最右两格现在和中间一样好放
- 换行的卡片仍按指针所在的那一行选落点，指针在第二行就不会把卡片插到第一行
- 设置页和状态栏的纵向列表仍按上下决定先后，表格表头和设置页图标槽位仍按左右，手感不变
- 内置传感器助手 unchanged at 0.8.6

## 0.8.13

- 状态栏的 CPU 电压前面现在写着来源：主板实测的 Vcore 轨显示 `Vcore 1.28V`，只有 CPU 自报的请求值时显示 `VID 1.44V`，两者都没有就只显示 `—`、不带前缀。单核电压（P-Core #1 之类）和 `CPU Termination` 这类不测量核心供电的通道不再冒充 CPU 供电，也不会再成为标题读数
- 不测量核心供电的通道不再进入界面：`CPU Termination`、VCCIO、VCCSA、VCCIN、VDDQ、VTT、PLL 以及 APU 的 SoC 轨，在主窗口的 CPU 详细分页和迷你窗口的电压卡片里都不再出现；主板实测 Vcore、CPU 的 `CPU Core` 请求值和逐核电压照旧
- 状态栏改成按“家族块”排列：`CPU 100% 188W  98°C  Vcore 1.21V  CLK 4.61GHz | GPU 0% 180W  62°C`。块与块之间按整块拖动；CPU/GPU 块内的占用、功耗、温度、电压、频率五个槽位可以各自排序，默认是占用 → 功耗 → 温度 → 电压 → 频率；名称和值之间只有一个空格，没有冒号
- CPU 有效频率带上 `CLK` 短名：`CLK 5.32GHz`。它等于 Windows 的 `% Processor Performance` 乘以这台机器在注册表里报告的标称频率（本机是 3418 MHz），和任务管理器“速度”同源；拿不到就不显示数字，不用 TDP 或型号表凑一个
- CPU 包功耗是独立槽位，只显示瓦数 `188W`（块内不会歧义），可以和占用、温度、电压、频率任意换位；GPU 功耗同样只显示 `180W`。功耗读不到时显示 `—`，不再绑在占用后面。设置里两个功耗项写全程 `CPU 功耗` / `GPU 功耗`，主窗口的 CPU 详细分页也写 `CPU 封装功耗`，和显卡功耗不会看混；CPU 功耗显示 LHM 的 package 读数（单位 W）
- 设置页按家族分组：`CPU` 一行主项，右侧是 `占用 / 功耗 / 温度 / 电压 / 频率` 五个可拖拽槽位图标，再加一个固定的“占用图表”开关；`GPU` 同构（占用 / 功耗 / 温度 + 图表开关）。内存、显存、硬盘、网络仍是各自一行。块间只靠左侧 `⋮⋮` 整块拖动，上下箭头已去掉，不提供键盘重排
- 图表只跟占用走：CPU/GPU 的“占用图表”挂在各自块里；功耗、电压、频率不再有曲线，原有的内存、硬盘、网络图表保持不变
- 旧配置兼容：0.8.12 及更早只存了扁平的 `overlayMetrics`，升级后启用的字段一个不丢；块顺序从旧字段列表里第一次出现的块推出来，其余块按默认位置补齐，另存为新的 `overlayOrder`，不会把字段顺序和块顺序混在一起
- 拖动设置图标时，原位置改成空出来的虚线框占位，图标幽灵跟随指针，松手后才落到虚线框处；不再在原位重复显示自己的图标
- 状态栏的 `CPU`/`GPU` 家族名固定在块首，永远在字段左边，不参与槽位排序；把占用拖到最后或把电压拖到最前，名称仍在最左
- 图标拖拽不再锁在单行：幽灵可以走到整个“显示项目”列表区域，跨行 drop 仍只落入同家族字段（或本行），离开列表才回弹；幽灵仍不裁切
- `THR` 徽标不再固定在最右边，改为紧跟在 CPU 块的温度字段后面；温度未启用时落在 CPU 块末尾
- 修复 CPU 详情“距 TjMax 无”：后端 wire 的 `distanceToTjMax` 在序列化给前端时曾被写成 `distanceToTjmax`，现在字段名一致，并加了 wire→JSON 的回归测试
- 图标提示改用仓库自带的深色 Tooltip（shadcn/bits-ui）：显示在图标上方，`delayDuration` 150ms、`skipDelayDuration` 500ms，Trigger 包住图标、Content 走 portal，所以不会被列表滚动容器裁切；A→B→C 连续滑过每个都会弹自己的提示，键盘 focus 也会弹。提示文案只留名字（占用 / 功耗 / 温度 / 电压 / 频率 / 图表），不再写“已显示/未显示/未启用”，状态只由图标本身的亮/灰表示；图标继续用 `aria-disabled` + 点击拦截，不再带原生 `title`。拖拽中提示由 CSS 隐藏，拖拽优先
- 设置页的顺序和启用状态现在是三件独立的事：块是否显示（父勾选）、每个字段的开关、块间/块内顺序；所有块始终留在列表里，取消父勾选只隐藏块并把子开关变灰不可操作，**子开关的值保留**，再勾上恢复原来的选择；取消单个字段只改它的开关，不会让它从顺序里消失；未启用的块也可以拖动
- 旧配置没有“块是否显示”这一层：升级后凡是至少有一个字段启用的块保持显示，字段开关和顺序一个不丢；单个读数块（内存/显存/硬盘/网络）重新显示时会自动打开它的字段
- 每个家族单独保存槽位顺序（CPU/GPU 各一份），不塞进块间顺序；图表开关不参与排序；只能在同行图标之间拖，跨行会拒绝并回弹。旧配置没有这份顺序，升级后按默认顺序归一
- 设置页不再把每个字段排成一行勾选框：CPU/GPU 家族各占一行，字段是横向的图标开关兼排序手柄，开启时着色，关闭时变灰，悬停显示原生提示；图表图标固定在最右，只是开关
- 拖动设置项的手感和卡片一致：拖动时其它块/槽位实时让位，被拖的项抬起（顶层、轻阴影、`grabbing` 光标），松手才写入顺序；上下箭头已去掉，键盘用户无法重排。拖动还修正了滚动设置页里的坐标偏移（之前按行偏移算错会让被拖的行“飞走”），拖到列表上下边缘会自动滚动
- 拖动的幽灵行现在被限制在“显示项目”列表的可视区域内：超出左右/上下就贴边，不会盖到上面的开关区或跑到窗口其它地方；拖到列表上方按第一位、下方按最后一位，左右拖出不影响结果。幽灵行通过 portal 放到 `body` 的 fixed 层，保留完整宽度和图标组，不被滚动容器或祖先的 `transform`/`filter` 裁切
- 传感器助手：安装包校验同时接受 `sha256sum` 的两种分隔格式（Linux 的两个空格和 Windows/Git Bash 的 `hash *path`）。0.8.13 早期本地包因为只认前者，把所有随附文件都当成缺失，停用后也点不到安装；现在停用后显示“未安装”，可以直接重新安装，升级旧助手到 0.8.6 仍是一次 UAC 更新，不会静默自升级
- 出包自检：打包前会从待打的 `fob.exe` 里确认嵌入的助手清单就是当前这份，再逐个核对包内 `sensors/`、`PawnIO/` 的字节；exe 与助手不是同一次构建、某个文件缺失或哈希不符都会拒绝出包并指出具体文件
- 随附文件有问题时不再只说“安装文件缺失或不匹配”：设置页会列出具体是哪个文件缺失或哈希不符，并提示用同一个压缩包完整解压、不要混用旧包目录；未安装时即使随附文件有问题也保留可点的安装入口，点下去会直接给出这条具体原因，不再卡死
- 计划任务指向旧目录（任务跑的不是已安装的助手）单独显示“任务仍指向旧目录”，按钮变成“重建任务”，不再和随附文件损坏混为一谈
- 节流提示改成真实 MSR 判断，不再用温度/频率推断：采样时读取 Intel `0x19C`/`0x1B1` 的热状态位和 `0x613` 的封装节流计时器；`过热降频` 和 `封装受限（非热墙）` 都是硬件的是/否，`THR` 只在“是”时亮，并附最近 120 个采样的触发百分比。没有新助手（低于 0.8.5）或 MSR 读不到时 `THR` 不显示，CPU 详情写“此机器或此助手版本不支持节流检测”。`0x64B`/`0x64F` 在本机全 0，所以不标“功耗/电流受限”；`0x606` 是 RAPL 单位，不是 limit reasons。98°C 时 thermal 位是否置位尚未实测（本次 10 秒满载约 87–90°C，未置位）
- CPU 详细页和迷你窗 CPU 详情新增 `距 TjMax`：来自 `0x19C`/`0x1B1` 的 bit 16–22，是硬件距离值，不是推断；状态栏不加这一项
- 合并脚本的错误文案写清了成功标准：合并后 commit 的 tree、parent、author 三项通过即视为成功，committer 是 `-0800` 是 GitHub 自己的已知行为，不是提交被改写；断言本身没有放宽成永远通过
- 传感器助手 (Sensor helper): 内置版本从 0.8.5 升到 0.8.6。0.8.5 虽然读到了 MSR，但组装 wire 的调用链没把节流状态传进去，所以管道里一直没有 throttle 字段；0.8.6 去掉默认参数、强制调用点传值，MSR 不可用时省略字段而不是发空状态。0.8.4 新增的 CPU package 功耗和电压筛选保持不变。设置 → 常规 → 传感器助手 会显示“有可用更新”，按 更新 → 继续，Windows 询问一次权限即可，无需重启 Fob

## 0.8.12

- 电压指标的悬停提示改为与全应用一致的原生提示：迷你窗口 (mini window) 电压卡片里的 Vcore、CPU Core、P 核平均、E 核平均四行，以及主窗口 (main window) CPU 详细分页里同名的 Vcore、CPU Core 两行，鼠标停在左侧指标名上时显示的是应用里其它地方一直在用的那个提示框（WebView2 画成浅黄色），提示文案不变。0.8.11 里给这几行加的深色气泡到此取消
- 提示只挂在这几个汇总标签上：逐核行（P-Core #1、E-Core #2 等单核读数）不挂，因为均值那句放在单核上是错的；卡片标题和卡片容器也不挂。主窗口逐核列表里没有对应说明的项（如 CPU Termination）保持和其它行一样的原生提示
- 读数、排序、卡片与详情页的版式都没有变；内置的传感器助手 (sensor helper) 仍是 0.8.3，这次更新不涉及它

## 0.8.11

- CPU 电压 (CPU voltage): 主窗口 (main window) 的 CPU 详细分页列出这台机器读到的全部 CPU 电压，迷你窗口 (mini window) 的 CPU 详情固定在进程排行上方，列出 Vcore、CPU Core 和 P/E 核平均；状态栏 (status bar) 有自己的 CPU 电压指标。读数保留三位小数（1.240 V），读不到时显示 —，不显示 0；温度与风扇卡片不混入电压，仍只列温度、风扇转速和占空比
- 逐核电压只在主窗口的 CPU 详细分页里，按核心名字列出；迷你窗口写平均，是因为几行位置里单列一个核心既不代表全体又占地方。卡片只列这台机器真有的读数：没有 P 核或 E 核读数的 CPU（如非混合架构的 Intel、没有 P/E 命名的 AMD）就少一行，不留空标签也不写 —，只剩 Vcore、CPU Core 时卡片就两行。判断看的是有没有 P-Core/E-Core 读数，不看厂商或型号；只有部分核心上报时用上报的那些算平均
- CPU 与 GPU 详情共用同一个头部：型号、规格行（CPU 是“16 核 / 24 线程”，GPU 是显存 / 温度 / 风扇）、然后是使用率百分比。百分比属于左边这块文字，左对齐、大号加粗，不再浮在右上角，两个面板的字号、字重、颜色、行高完全一致；型号宽度不够时省略号收尾（悬停可看全名），下面那行始终完整
- 电压卡片里 Vcore、CPU Core 两行，以及主窗口逐核列表里的同名行，悬停（或按 Tab 聚焦）左侧指标名会有一句说明：Vcore 是主板实测的供电电压，CPU Core 是 CPU 请求的电压（VID），P/E 核平均是两簇的均值。逐核行（P-Core #1、E-Core #2 等单核读数）不挂这句，因为均值那句放在单核上是错的；没有对应说明的项（如 CPU Termination）同样不提，也不用浏览器自带的 title 兜底
- 只显示 CPU 电压：CPU 自己的通道（CPU Core 和各核心）以及主板给 CPU 供电的那条（Vcore、CPU Termination）。整板轨道（AVCC、+3.3V、+5V、+12V、+3V 待机、未命名的 Voltage #N）既不上传也不显示，显卡电压同样不显示
- 状态栏的 CPU 电压指标默认打开，可在 设置 → 状态栏 里关掉，也可和其它指标一样调顺序、开关图表；图表对它不可用，与显存占用、显卡功耗一致
- 没有电压可读的机器（不少笔记本和不被 Super I/O 支持的芯片组）不显示假数据：详情页与状态栏都只显示 —，其余指标照旧
- 传感器助手 (Sensor helper): 内置版本现在是 0.8.3，0.8.10 里是 0.8.2。0.8.3 是第一个上报 CPU 电压的助手，装过 0.8.2 的机器要更新一次才能看到电压：设置 → 常规 → 传感器助手 会显示“有可用更新”，按 更新 → 继续，Windows 询问一次权限即可。无需重启 Fob，助手重连后电压自己出现；计划任务在这一步重新注册并立即启动，不用重新登录

## 0.8.10

- 主窗口 (Main window): chart history is sent in full on the first read, then only new samples are transferred. Returning after a long hidden period resynchronizes the full retained history when needed; chart cadence and the independent mini window/status bar readings are unchanged
- 进程 CPU (Process CPU): unavailable Windows CPU readings and samples without a valid baseline now show — rather than 0.0%. Measured idle still shows 0.0%, and readings recover after two successful samples
- A process family's CPU total shows — if any member has no reading. Unknown CPU values sort after measured values in either direction and do not match numeric CPU filters; system-wide CPU readings and other process metrics are unchanged
- Skip the unused legacy Windows process CPU calculation while retaining system CPU and other process readings; Settings → General now has one owner for its initial helper status request, with manual and action refreshes unchanged
- The bundled sensor helper is unchanged at 0.8.2

## 0.8.9

- 空间分布 (Space map): when right-click Show in File Explorer finds the item gone (Windows error 2 or 3), Fob now says the item no longer exists and that the scan may be out of date, with a hint to press 重新扫描 (Rescan). This replaces an English-only message that also showed in the Chinese interface
- When Windows denies access to the item (error 5) or the check fails for another reason, Fob says so instead of claiming the item is gone; other errors show their Windows error number. A File Explorer that does not start has its own message in both languages
- A message on the scan's status line now wraps onto a second line when the line is full, instead of being cut off
- 进程 (Processes): bringing the main window back from the tray or the taskbar now reads the process list at once, instead of showing the list from before it was hidden until the next refresh (up to the refresh interval). While the main window is hidden or minimized it still reads no process list, as before
- 状态栏 (Status bar): turning it off now closes its window, and turning it on builds a new one. If the bar went blank (for example while a game was running), switching it off and on brings it back without restarting Fob. Its position, dock and settings are kept. While shown, the bar also puts itself back above other always-on-top windows every 2 seconds, without taking focus, so a borderless game that brings itself to the top no longer covers it for good
- Scanning itself is unchanged. The bundled sensor helper is unchanged at 0.8.2

## 0.8.8

- In every theme except 玻璃 (Glassy), the main window's backdrop is now Mica instead of Acrylic. Only Glassy lets the backdrop show through; the other themes cover it with an opaque page, and Acrylic's live blur of what is behind the window is GPU work Windows does while the window is active. Glassy keeps Acrylic. On Windows 11 the title bar may look slightly different in the other themes. Windows versions without Mica keep Acrylic as before
- Whether this lowers the desktop window manager's GPU use with Fob in front has not been measured yet; a test on Windows is pending
- Only one Fob runs at a time. Starting Fob again, from a shortcut, the Start menu or the login item, brings the running Fob's main window forward instead: from the tray, from the taskbar when minimized, or simply to the front, keeping its size and maximized state. Two launches at the same moment leave one Fob. After Fob quits it starts normally again. This holds per Windows sign-in session, and Fob still never asks for administrator rights to run
- After Windows' display scale changes while Fob runs (for example 100% to 150% and back), the main window now redraws its page at the new scale by itself instead of staying blank or garbled until its edges were dragged. This also happens when the window comes back from the tray after a change. Saved size, theme and layout are kept; the 迷你窗口 (mini window) and the status bar are unchanged
- The display scale fix is a candidate: it has not yet been tried on Windows with a real scale change, so it is not confirmed
- An NVIDIA card's GPU % now comes, everywhere Fob shows it (the 迷你窗口 (mini window) included), from the same Windows reading as the GPU % column in 进程 (Processes): the card's busiest engine (3D, Copy, Video and so on) over the same refresh. Before, it came from the NVIDIA driver, which counts only the graphics and compute pipe over its last second or less, so the card could read well above every process. The card now also lists its engine loads like other GPUs. Temperature, power, fan, clock and memory still come from the NVIDIA driver
- The driver's own GPU % is used only when the Windows reading is unavailable, and the GPU detail's note says which source the number comes from. Two cards with the same name keep the driver's number, since Windows' reading cannot be matched to one of them by name
- A process's GPU % is still its own busiest engine, and the card's GPU % is the busiest engine of all processes together, so process values are not meant to add up to the card's value
- Windows engine readings that Windows marks as new since the last refresh are no longer skipped
- The GPU changes were checked by unit tests with made-up readings; a check on Windows with a real NVIDIA card is still pending
- The GPU detail's engine list now uses official Chinese names where they exist (三维引擎, 复制引擎, 光流加速器, 视频解码, 视频编码); other names, including numbered ones such as Security_1, stay as the driver reports them. English keeps the driver names. Pointing at an engine shows only the short description, not the name again
- The 关于这些读数 (About these readings) note is now fully in Chinese or English, instead of mixing in an English source sentence
- Process column headers no longer show a hover explanation, including sort, resize and drag hints; sorting, dragging a column's edge, reordering and Esc to cancel are unchanged, and summary cards keep their grips without a drag hint
- Other hover text that repeated a visible name now shows only the extra explanation, or is omitted when it would only repeat the label
- The engine names and notes were checked in a browser preview in Chinese and English with mocked readings; a check in the Windows app is still pending
- The bundled sensor helper is unchanged at 0.8.2

## 0.8.7

- SATA disks attached directly to the motherboard now show what the drive reports in its standard ATA statistics: temperature, life used (the drive's estimate, which can pass 100%), data written and read, power-on time and power cycles. Fob reads them without administrator rights and does not read vendor SMART attributes
- Only items the drive marks as valid are shown. A hard disk without SSD life used, a driver that refuses the query or a drive without these statistics says so instead of showing 0 or looking healthy
- When Windows reports a SATA disk asleep, Fob skips its statistics for that reading instead of asking the disk. Windows does not always know a hard disk has spun down, so this cannot promise the disk never wakes
- The 存储 (Storage) side panel shows the same temperature and life used for SATA disks as the Disk detail
- USB, RAID and other disks still show only the status Windows reports; NVMe readings are unchanged
- Checked with mocked readings in a browser preview in Chinese and English; readings on real Windows SATA disks are still pending
- Processes columns can be dragged by their header into a new order; 进程名称 (Process Name) stays first. The column lifts and follows the pointer while the other columns, values included, slide aside to show where it will land. A short click still sorts, and Esc cancels a drag and puts the columns back
- A small dot grip in each summary card's title shows the card can be dragged. It lines up with the values below and darkens on hover. Card titles and column headers both carry the tooltip 拖动调整顺序 (Drag to reorder); column headers show no grip and no lines between them. A drag starts only from a card's title or a column's title, never from values, charts or a column's edge. The 迷你窗口 (mini window) cards have none
- A dragged summary card is drawn above everything else in the window, so the card area, the tab bar and the scrolling 仪表盘 (Dashboard) no longer cut it off; it stays inside the window
- The 用户 (User) column shows the account that owns each process the way Task Manager does, such as SYSTEM, LOCAL SERVICE, NETWORK SERVICE or a user name, instead of its SID. Each account is looked up once in the background, so a refresh never waits for it: the SID shows until the name is known, an owner with no account keeps its SID, and a process whose owner Windows will not tell shows a dash
- The summary cards on the 仪表盘 (Dashboard) and 进程 (Processes) tabs can be dragged by their title into a new order the same way; a drag does not open the card. Reordering has no keyboard shortcut. With reduced motion turned on in Windows, items move without sliding
- Column and card orders are saved with the other settings and come back after a restart; a column shown later returns to its saved place
- Processes columns are as wide as their values and header need, so short values such as PID, CPU % and GPU % sit close together instead of being spread across the window. On a wide window 进程名称 (Process Name) grows to fit long names, then 命令行 (Command) when shown, and any room left over stays empty before the actions column. With the default columns the table fits without scrolling sideways from about 1262 px, so a 1920 px screen at 125% scaling no longer scrolls
- A column's width can be set by dragging the right edge of its header. The edge shows a thin mark only on hover and while dragging, Esc cancels a drag, and a double-click on the edge returns the column to its automatic width. Resizing never sorts or moves a column, and the widths are saved with the other settings
- The ordering and width changes were checked in a browser preview at 1024 and 1920 px in Chinese and English; dragging in the Windows app, display scaling and saving in the installed app are still pending
- The main window opens again at the size and position it had when Fob last closed, and maximized if it was. Minimizing or hiding it to the tray keeps the last normal size, and a size is also saved shortly after each resize, so shutting Windows down with Fob in the tray keeps it. A position on a screen that is no longer connected is left to Windows. Before, the main window always opened at its default size. Checked by unit tests only; a restart of the Windows app is still pending
- The bundled sensor helper is unchanged at 0.8.2

## 0.8.6

- The 硬盘 (Disk) detail has a Physical disks section with one entry per physical disk: its number, bus, model and the volumes on it, including disks and partitions without a drive letter
- NVMe disks show the drive's own health log, read without administrator rights: temperature, life used (the drive's estimate, which can pass 100%), available spare and its threshold, data written and read, power-on time, power cycles, unsafe shutdowns, media errors, error log entries and time spent above the drive's temperature thresholds
- SATA and other disks show the health and operational status Windows reports; Fob does not read ATA SMART attributes
- The 存储 (Storage) side panel on the Processes tab lists each physical disk once, however many volumes it has, with its status and, when the drive reports them, temperature and life used. 查看健康详情 (View health details) closes the panel and opens the full Disk detail at its health section
- A warning appears only when the drive sets a critical warning flag or Windows reports a warning or failure. A value the disk does not report shows a dash, and a disk that refuses or does not answer the query says so instead of looking healthy
- Fob reads the disks when the Disk detail or the Storage panel opens and at most every 5 minutes while one stays open, never in the background; both share one cached reading, and asks for no extra permission. The mini window does not show this section
- Checked with mocked readings in a browser preview in Chinese and English; readings on real Windows disks are still pending
- The bundled sensor helper is unchanged at 0.8.2

## 0.8.5

- Processes columns that show the same kind of value now have the same width: IDs, percentages, speeds, sizes and times each share one width
- Column headers fit with their sort arrow and line up with the values below them, including GPU %, 硬盘读取 / 硬盘写入 (Disk read / Disk write), 专用显存 (Dedicated VRAM), 父进程 PID (Parent PID), 用户 (User) and 进程族 CPU (Family CPU) in tree view
- Run time no longer runs into the actions buttons: when the window is a little narrower than the table, the process name column narrows instead of the table scrolling the end of 运行时长 (Run Time) under the buttons, and 运行时长 now reads like 0h 37m 45s in Chinese too, the same as in English
- Checked in a browser preview at 1024 and 1600 px wide in Chinese and English; a check on Windows at 125% and 150% display scaling is still pending
- The bundled sensor helper is unchanged at 0.8.2

## 0.8.4

- On hybrid CPUs the CPU views split the load into performance and efficiency cores, read from the efficiency class Windows reports for each core. Intel shows P 核 / E 核 (P-cores / E-cores); AMD and other vendors show 性能核 / 能效核 (Performance cores / Efficiency cores)
- Each group is the average of its own threads, like the total CPU %, so a P-core's two threads count once each and no group goes past 100%
- The dashboard CPU card groups its bars and lists each group's load; the Processes CPU card, the CPU detail and the CPU details sheet show every group with its load and its cores and threads
- CPUs with one kind of core, and machines where the layout cannot be read, look as before. A group without a reading shows a dash, not 0%
- The mini window card is unchanged; its CPU detail shows the split
- The bundled sensor helper is unchanged at 0.8.2

## 0.8.3

- Processes shows each process's disk read and write speed in bytes per second over the last refresh, in two sortable columns (硬盘读取 / 硬盘写入). A process without two readings, or whose counters cannot be read, shows a dash instead of 0 B/s
- GPU % is visible by default, once, for settings saved by earlier versions. On Windows it is the busiest GPU engine the process used, like Task Manager; a process with no GPU engine reads 0.0%, and a dash means Windows gave no GPU reading
- Optional Dedicated VRAM column (专用显存) on Windows from the same GPU counters
- No reading sorts after every real value in both directions
- The sensor helper's Disable note says PawnIO is uninstalled in Windows Settings, not in Fob
- The bundled sensor helper is unchanged at 0.8.2, so an installed 0.8.2 helper needs no update

## 0.8.2

- Cache command and environment details by process identity, share environment lists between frames, and discard exited-process cache entries without changing JSON shape

- Add persistent sensor helper controls in Settings › General, with installed and bundled versions, read-only status refresh, and explicit Install, Update, Repair, and Disable consent
- Reuse the existing verified helper setup flow and disclose whole-Disk metadata access without elevating Fob
- Detect mixed helper bundles and missing task or sensor response without claiming the helper is working, and prevent concurrent setup prompts across app windows

## 0.8.1

- Normally launched Fob uses the updated elevated sensor helper for NTFS MFT space scans. The helper update uses the existing explicit one-time UAC flow.
- Independent, current-user scan IPC keeps sensor updates separate. Requests only authorize a fixed NTFS drive root and return bounded read-only metadata. Cancellation, disconnects, and deadlines stop the worker.
- Old or unavailable helpers retain direct-MFT and SSD-aware folder-scan fallback. Scan results identify the method used.

## [1.3.0] — 2026-10-29

### A refreshed desktop, with the themes you know

- Migrate the interface to shared shadcn-svelte components and consistent spacing, controls and semantic theme tokens; retain all 24 original themes.
- Add native process/app icons on macOS, Linux and Windows, with the familiar ghost when an icon cannot be resolved.
- Add keyboard/pointer inspection and faded area fills to CPU, memory, GPU and network charts, with approximately two minutes of in-memory history and honest gaps for paused or missing readings.
- Add detailed CPU, memory, graphics, storage, system and network sheets, including per-core activity, volumes and interface search.
- Simplify the app bar and process toolbar; add separate Settings and About windows, automatic preference migration, cross-window synchronization, and native desktop shortcuts.

### Processes and graphics

- Add expandable process families with combined CPU/RAM, matching-parent context, stable family pagination and saved List/Tree choice.
- Search local TCP listeners and bound UDP sockets with `:3000` or `port:443`. Socket scans run only during explicit port queries; bare numbers continue to match PIDs. Thanks to [@felipe-vicente](https://github.com/felipe-vicente) in [#201](https://github.com/Abdenasser/neohtop/pull/201).
- Add driver-dependent GPU utilization, memory, temperature, core count, power and clocks. Use macOS IORegistry, Linux AMD sysfs/NVIDIA NVML, and Windows NVML/DXGI inventory; unsupported readings remain unavailable.
- Integrate optional per-process GPU engine accounting and filters for supported Linux DRM drivers. Readings need a baseline, may exceed 100% across engines and never treat inaccessible data as zero. Thanks to [@nezmaka](https://github.com/nezmaka) in [#202](https://github.com/Abdenasser/neohtop/pull/202).

### Menu-bar companion

- Add a rounded overview with CPU, RAM, GPU, network, free storage and the five busiest processes, with real app icons and inspector shortcuts.
- Share the sample, interval, pause and theme across windows; stop companion polling while hidden.
- Offer tray visibility and opt-in monitoring after closing the main window. Closing Settings or About leaves the monitor running; Quit always exits.

### Fixes and delivery

- Handle zero/unknown storage capacity without NaN percentages. Thanks to [@dajiaohuang](https://github.com/dajiaohuang) in [#204](https://github.com/Abdenasser/neohtop/pull/204).
- Fix process-inspector clipping and bottom spacing, dialog/empty-state consistency, theme contrast and focus recovery.
- Correct network-rate baselines for new/reset interfaces and report missing driver counters honestly.
- Repair cross-platform packaging and coordinate release publication only after all five platform jobs pass.
- Ship Apple Silicon/Intel macOS DMGs, Linux x86_64/ARM64 deb/rpm/AppImage packages, and a Windows x64 NSIS installer, with SHA-256 checksums and a build manifest. macOS release builds require signing and notarization.
- Update the website, screenshots, installation guide, usage documentation and machine-readable product references.

### Privacy

- Add a disclosed basic launch counter (app version, no persistent identifier) through a Cloudflare relay. Detailed feature usage remains off by default in Settings → Privacy. Process names, commands, queries, paths, port numbers and system readings are never sent.
- Keep Google Analytics credentials on the server. Builds without the analytics endpoint send no desktop events; monitoring works offline.

### Compatibility and scope

- Source builds now require Rust 1.88 or newer and Node.js 22. GPU/socket detail visibility depends on drivers and operating-system permissions; Linux tray behavior varies by desktop.
- GPU shared memory on Apple silicon is part of system RAM. Family RAM totals can double-count shared pages. Short resource histories are in-memory and reset on restart.
- NeoHtop Pro is postponed. This is the free, MIT-licensed desktop app; no account, paid plan or cloud subscription is required.

[1.3.0]: https://github.com/Abdenasser/neohtop/compare/v1.2.0...v1.3.0

Older release notes are available on [GitHub Releases](https://github.com/Abdenasser/neohtop/releases).
