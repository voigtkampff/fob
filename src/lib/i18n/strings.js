// Fob UI strings: one key -> string table per locale, no i18n framework.
// Keep both tables in the same key order; tests/i18n.mjs checks that every key
// exists in both and that every key used in src/ exists.

/** @type {Record<string, string>} */
export const en = {
  "dur.days": "{d} day, {h} hr",
  "dur.daysPlural": "{d} days, {h} hr",
  "dur.hours": "{h} hr, {m} min",
  "dur.minutes": "{m} min",
  "uptime.on": "on {d}",
  "uptime.up": "up {d}",
  "uptime.titleStart":
    "Time since Windows last started (power-on or Fast Startup). Kernel uptime, as Task Manager shows it: {d}",
  "uptime.titleKernel": "Time since the system kernel started",
  "steps.legacy":
    "Stop the winperf 0.4 sensor helper and delete its scheduled task “winperf-sensors” and its folder C:\\Program Files\\winperf\\sensors.",
  "steps.pawnio":
    "Install the PawnIO driver from the bundled PawnIO_setup.exe (version 2.2.0, signed by its author namazso, GPL-2.0). Its own installer window opens; Windows needs it so programs can read CPU and motherboard sensors.",
  "steps.replace":
    "Replace the sensor helper in C:\\Program Files\\Fob\\sensors with the one bundled with this Fob.",
  "steps.copy":
    "Copy the read-only sensor helper (fob-sensors.exe, built on LibreHardwareMonitorLib) to C:\\Program Files\\Fob\\sensors.",
  "steps.task":
    "Create the scheduled task “fob-sensors” that starts the helper with administrator rights when you sign in, without asking again, and start it now.",
  "steps.retask":
    "Re-create the scheduled task “fob-sensors” so it starts that helper when you sign in, and start it now.",
  "dash.system": "System",
  "dash.cards": "System cards",
  "dash.threads": "{n} threads",
  "dash.cores": "{n} cores",
  "dash.perCore": "Per-core usage",
  "level.idle": "idle",
  "level.active": "active",
  "level.busy": "busy",
  "dash.gpuNoCounter": "no live counter for this adapter yet",
  "dash.noGpu": "no GPU detected",
  "dash.memory": "Memory",
  "dash.memHigh": "High",
  "dash.memNormal": "Normal",
  "dash.freeSpace": "Free space",
  "dash.used": "{v} used",
  "common.na": "n/a",
  "dash.noVolumes": "no volumes reported",
  "dash.network": "Network",
  "dash.down": "down",
  "dash.up": "up",
  "dash.temp": "Temp & fan",
  "dash.fans": "Fans {v}",
  "dash.fan": "fan {v}",
  "dash.noSensorShort": "No sensor source found. Open for details.",
  "dash.noSensor":
    "No temperature or fan source found. Open this card for details; no readings are faked.",
  "dash.updateHelper": "Update sensor helper",
  "dash.repair": "Repair fan/temperature",
  "dash.enable": "Enable fan/temperature",
  "dash.migrate": "Move sensor helper to Fob",
  "dash.migrateBanner":
    "The winperf 0.4 sensor helper is still installed. Fob uses its own; move to it with one Windows prompt.",
  "dash.migrateOpen": "Review…",
  "dash.topProcs": "Top processes",
  "dash.topByMemory": "Top processes by memory",
  "dash.cpuPct": "% CPU",
  "detail.cpu": "CPU",
  "detail.gpu": "GPU",
  "detail.memory": "Memory",
  "detail.disk": "Drives",
  "detail.network": "Network",
  "detail.temp": "Temperatures & fans",
  "detail.aria": "{title} details",
  "detail.backAria": "Back to cards",
  "detail.back": "Back",
  "detail.coresThreads": "{c} cores / {n} threads",
  "detail.thread": "Thread {i}: {v}%",
  "detail.topProcs": "Top processes",
  "detail.gpuPct": "% GPU",
  "detail.fanPct": "fan {v} %",
  "detail.noGpu": "No GPU detected.",
  "detail.noGpuProcs":
    "No process is using the GPU right now, or this system doesn't report per-process GPU use (Windows: GPU Engine counters).",
  "detail.used": "Used",
  "detail.available": "Available",
  "detail.total": "Total",
  "detail.committed": "Committed",
  "detail.pageFile": "Page file",
  "detail.swap": "Swap",
  "detail.freeOf": "{free} free of {total}",
  "detail.readOnly": "read-only",
  "detail.noDrives": "No drives reported.",
  "detail.download": "Download",
  "detail.upload": "Upload",
  "detail.app": "App",
  "detail.noAppTraffic": "No app has used the network in the last 30 seconds.",
  "detail.perAppNeedsHelper":
    "Per-app speeds come from the Fob sensor helper, which reads Windows network events (ETW) with administrator rights.",
  "detail.waitingHelper": "Waiting for the helper…",
  "detail.adapter": "Adapter",
  "detail.noAdapters": "No active network adapters.",
  "detail.fansOf": "Fans · {hw}",
  "detail.rpm": "RPM",
  "detail.fanStopped": "0 · stopped or unused",
  "detail.duty": "(duty)",
  "detail.untilThen":
    "Until then only NVIDIA GPUs report their temperature and fan here. No numbers are faked.",
  "detail.noReadings":
    "No temperature or fan readings are available on this system. NVIDIA GPUs report their temperature and fan through the driver; other sensors need a supported sensor source. No numbers are faked.",
  "detail.source": "Source: {v}",
  "setup.missingFiles":
    "Fan and temperature readings need the <b>sensors</b> and <b>PawnIO</b> folders that came with Fob. Keep the whole Fob folder together, next to fob.exe.",
  "setup.willDo": "Fob will do the following:",
  "setup.uacNote":
    "Windows shows one permission (UAC) prompt for all of it; Fob itself keeps running without administrator rights. The helper only reads sensors and network byte counts, never changes fan speeds, and works alongside FanControl or other monitoring tools. You can undo it here with Disable.",
  "setup.waiting": "Waiting for Windows…",
  "setup.continue": "Continue",
  "setup.cancel": "Cancel",
  "setup.disableTitle": "Disable fan/temperature",
  "setup.disableNote":
    "Stops the helper, deletes the “fob-sensors” scheduled task and removes C:\\Program Files\\Fob\\sensors. Windows shows one permission prompt. The PawnIO driver stays installed; other tools such as FanControl may use it. You can uninstall it from Settings › Apps.",
  "setup.disable": "Disable",
  "setup.disableMore": "Disable…",
  "setup.repairMore": "Repair…",
  "setup.needHelper":
    "CPU, motherboard and fan sensors and per-app network speeds need Fob's read-only sensor helper. It is set up once, with your permission.",
  "setup.migrate":
    "winperf 0.4's sensor helper (task “winperf-sensors”) is still installed and Fob can't use it. Fob replaces it with its own helper: the old one is stopped and its task and folder are deleted.",
  "setup.newer":
    "This Fob comes with a newer sensor helper than the installed one.",
  "setup.incomplete":
    "The sensor helper setup is incomplete or points somewhere else.",
  "setup.running": "Sensor helper is running.",
  "setup.starting":
    "Sensor helper is installed but not answering yet; Fob asked it to start.",
  "app.name": "Fob",
  "mini.paused": "Paused",
  "mini.lastReading": "Last reading {s}s ago",
  "mini.live": "Live",
  "mini.reading": "Reading…",
  "mini.resume": "Resume updates",
  "mini.pause": "Pause updates",
  "mini.openFull": "Open full window",
  "mini.quit": "Quit Fob",
  "mini.retry": "Retry",
  "mini.readingMachine": "Reading this machine…",
  "title.liveEvery": "Live · {s}s",
  "title.openMini": "Open mini window",
  "title.mini": "Mini window",
  "title.collapse": "Collapse to tray",
  "title.collapseTitle": "Collapse to tray (mini window mode)",
  "title.menu": "Fob menu",
  "title.settings": "Settings…",
  "title.about": "About Fob",
  "title.dismiss": "Dismiss",
  "tabs.views": "Views",
  "tabs.dashboard": "Dashboard",
  "tabs.processes": "Processes",
  "col.name": "Process Name",
  "col.pid": "PID",
  "col.status": "Status",
  "col.user": "User",
  "col.cpu_usage": "CPU %",
  "col.gpu_usage": "GPU %",
  "col.memory_usage": "RAM",
  "col.virtual_memory": "VIRT",
  "col.disk_usage": "Disk I/O (R/W)",
  "col.ppid": "Parent PID",
  "col.root": "Root",
  "col.command": "Command",
  "col.environ": "Environment Variables",
  "col.session_id": "Session ID",
  "col.start_time": "Start Time",
  "col.run_time": "Run Time",
  "col.familyCpu": "Family CPU",
  "col.familyRam": "Family RAM",
  "col.familyCpuTitle":
    "Combined family CPU, as a share of all logical processors (like Task Manager).",
  "col.familyRamTitle":
    "Sum of resident memory for each family; shared pages can be counted more than once.",
  "col.sortBy": "Sort by {v}",
  "col.actions": "Actions",
  "col.viewOptions": "View options",
  "col.view": "View",
  "col.visible": "Visible columns",
  "col.required": "Required",
  "settings.section": "Settings section",
  "settings.general": "General",
  "settings.appearance": "Appearance",
  "settings.generalLead": "Choose how Fob monitors your system.",
  "settings.monitoring": "Monitoring",
  "settings.refresh": "Refresh interval",
  "settings.refreshNote": "Applies to the app and the tray mini window.",
  "settings.refreshMore":
    "Longer intervals reduce background activity. Pause and resume from the app bar.",
  "settings.language": "Language",
  "settings.languageLabel": "Display language",
  "settings.languageNote":
    "Automatic follows the Windows display language. Also in the tray menu.",
  "lang.auto": "Automatic (Windows)",
  "lang.en": "English",
  "lang.zh": "中文",
  "settings.tray": "Tray mini window",
  "settings.showTray": "Show tray icon",
  "settings.showTrayNote":
    "Keep CPU, memory, graphics and network readings close at hand.",
  "settings.startInTray": "Start in tray only (mini window mode)",
  "settings.startInTrayNote":
    "Fob starts with just the tray icon; click it for the mini window. Open the full window from the tray menu or the mini window.",
  "settings.closeNote":
    "With the tray icon on, closing the main window keeps Fob running in the tray. Choose Quit from the tray menu to exit.",
  "settings.appearanceLead": "All your Fob themes, across every window.",
  "settings.retry": "Retry",
  "settings.loading": "Loading settings…",
  "settings.saving": "Saving changes…",
  "settings.saved": "Changes are saved automatically.",
  "about.basedOn": "Based on NeoHtop by {v} (MIT)",
  "about.openSource": "Open source",
  "about.stack": "Stack",
  "about.source": "NeoHtop source on GitHub",
  "about.updates": "Local build. Updates are installed by replacing fob.exe.",
};

/** @type {Record<keyof typeof en, string>} */
export const zh = {
  "dur.days": "{d} 天 {h} 小时",
  "dur.daysPlural": "{d} 天 {h} 小时",
  "dur.hours": "{h} 小时 {m} 分",
  "dur.minutes": "{m} 分钟",
  "uptime.on": "已开机 {d}",
  "uptime.up": "运行 {d}",
  "uptime.titleStart":
    "距 Windows 上次启动（开机或快速启动）的时间。任务管理器显示的内核运行时间：{d}",
  "uptime.titleKernel": "系统内核启动以来的时间",
  "steps.legacy":
    "停止 winperf 0.4 的传感器助手，删除其计划任务“winperf-sensors”和文件夹 C:\\Program Files\\winperf\\sensors。",
  "steps.pawnio":
    "从随附的 PawnIO_setup.exe 安装 PawnIO 驱动（2.2.0 版，由作者 namazso 签名，GPL-2.0）。会打开它自己的安装窗口；Windows 需要它才能让程序读取 CPU 和主板传感器。",
  "steps.replace":
    "用本版 Fob 随附的传感器助手替换 C:\\Program Files\\Fob\\sensors 中的版本。",
  "steps.copy":
    "将只读的传感器助手（fob-sensors.exe，基于 LibreHardwareMonitorLib）复制到 C:\\Program Files\\Fob\\sensors。",
  "steps.task":
    "创建计划任务“fob-sensors”，在你登录时以管理员权限启动助手（之后不再询问），并立即启动。",
  "steps.retask":
    "重新创建计划任务“fob-sensors”，使其在你登录时启动助手，并立即启动。",
  "dash.system": "系统",
  "dash.cards": "系统卡片",
  "dash.threads": "{n} 线程",
  "dash.cores": "{n} 核",
  "dash.perCore": "各核心占用",
  "level.idle": "空闲",
  "level.active": "活跃",
  "level.busy": "繁忙",
  "dash.gpuNoCounter": "此显卡暂无实时计数",
  "dash.noGpu": "未检测到 GPU",
  "dash.memory": "内存",
  "dash.memHigh": "偏高",
  "dash.memNormal": "正常",
  "dash.freeSpace": "可用空间",
  "dash.used": "已用 {v}",
  "common.na": "无",
  "dash.noVolumes": "未报告任何磁盘卷",
  "dash.network": "网络",
  "dash.down": "下载",
  "dash.up": "上传",
  "dash.temp": "温度与风扇",
  "dash.fans": "风扇 {v}",
  "dash.fan": "风扇 {v}",
  "dash.noSensorShort": "未找到传感器来源，点开查看详情。",
  "dash.noSensor":
    "未找到温度或风扇来源。点开此卡片查看详情；不会伪造任何读数。",
  "dash.updateHelper": "更新传感器助手",
  "dash.repair": "修复风扇/温度",
  "dash.enable": "启用风扇/温度",
  "dash.migrate": "将传感器助手迁移到 Fob",
  "dash.migrateBanner":
    "winperf 0.4 的传感器助手仍已安装。Fob 使用自己的助手，只需一次 Windows 提示即可迁移。",
  "dash.migrateOpen": "查看…",
  "dash.topProcs": "进程排行",
  "dash.topByMemory": "按内存排序的进程",
  "dash.cpuPct": "% CPU",
  "detail.cpu": "CPU",
  "detail.gpu": "GPU",
  "detail.memory": "内存",
  "detail.disk": "磁盘",
  "detail.network": "网络",
  "detail.temp": "温度与风扇",
  "detail.aria": "{title}详情",
  "detail.backAria": "返回卡片",
  "detail.back": "返回",
  "detail.coresThreads": "{c} 核 / {n} 线程",
  "detail.thread": "线程 {i}：{v}%",
  "detail.topProcs": "进程排行",
  "detail.gpuPct": "% GPU",
  "detail.fanPct": "风扇 {v} %",
  "detail.noGpu": "未检测到 GPU。",
  "detail.noGpuProcs":
    "当前没有进程在使用 GPU，或此系统不报告每进程 GPU 占用（Windows：GPU Engine 计数器）。",
  "detail.used": "已用",
  "detail.available": "可用",
  "detail.total": "总计",
  "detail.committed": "已提交",
  "detail.pageFile": "页面文件",
  "detail.swap": "交换空间",
  "detail.freeOf": "{total} 中可用 {free}",
  "detail.readOnly": "只读",
  "detail.noDrives": "未报告任何磁盘。",
  "detail.download": "下载",
  "detail.upload": "上传",
  "detail.app": "应用",
  "detail.noAppTraffic": "最近 30 秒内没有应用使用网络。",
  "detail.perAppNeedsHelper":
    "各应用网速来自 Fob 传感器助手，它以管理员权限读取 Windows 网络事件（ETW）。",
  "detail.waitingHelper": "正在等待助手…",
  "detail.adapter": "网卡",
  "detail.noAdapters": "没有活动的网卡。",
  "detail.fansOf": "风扇 · {hw}",
  "detail.rpm": "转/分",
  "detail.fanStopped": "0 · 停转或未使用",
  "detail.duty": "（占空比）",
  "detail.untilThen":
    "在此之前，这里只有 NVIDIA 显卡会报告温度和风扇。不会伪造任何数字。",
  "detail.noReadings":
    "此系统没有可用的温度或风扇读数。NVIDIA 显卡通过驱动报告温度和风扇；其他传感器需要受支持的传感器来源。不会伪造任何数字。",
  "detail.source": "来源：{v}",
  "setup.missingFiles":
    "风扇和温度读数需要 Fob 随附的 <b>sensors</b> 和 <b>PawnIO</b> 文件夹。请保持整个 Fob 文件夹完整，放在 fob.exe 旁边。",
  "setup.willDo": "Fob 将执行以下操作：",
  "setup.uacNote":
    "Windows 会为这些步骤显示一次权限（UAC）提示；Fob 本身始终不以管理员权限运行。助手只读取传感器和网络字节数，从不更改风扇转速，可与 FanControl 等监控工具同时使用。可随时在此处用“停用”撤销。",
  "setup.waiting": "正在等待 Windows…",
  "setup.continue": "继续",
  "setup.cancel": "取消",
  "setup.disableTitle": "停用风扇/温度",
  "setup.disableNote":
    "停止助手，删除计划任务“fob-sensors”，并移除 C:\\Program Files\\Fob\\sensors。Windows 会显示一次权限提示。PawnIO 驱动会保留，FanControl 等其他工具可能在使用它；可在“设置 › 应用”中卸载。",
  "setup.disable": "停用",
  "setup.disableMore": "停用…",
  "setup.repairMore": "修复…",
  "setup.needHelper":
    "CPU、主板、风扇传感器以及各应用网速需要 Fob 的只读传感器助手。只需在你同意后设置一次。",
  "setup.migrate":
    "winperf 0.4 的传感器助手（任务“winperf-sensors”）仍已安装，Fob 无法使用它。Fob 会用自己的助手替换它：停止旧助手并删除其任务和文件夹。",
  "setup.newer": "此版本 Fob 附带的传感器助手比已安装的更新。",
  "setup.incomplete": "传感器助手设置不完整或指向了其他位置。",
  "setup.running": "传感器助手正在运行。",
  "setup.starting": "传感器助手已安装但尚未响应；Fob 已请求启动它。",
  "app.name": "Fob",
  "mini.paused": "已暂停",
  "mini.lastReading": "上次读数 {s} 秒前",
  "mini.live": "实时",
  "mini.reading": "读取中…",
  "mini.resume": "继续更新",
  "mini.pause": "暂停更新",
  "mini.openFull": "打开完整窗口",
  "mini.quit": "退出 Fob",
  "mini.retry": "重试",
  "mini.readingMachine": "正在读取本机…",
  "title.liveEvery": "实时 · {s} 秒",
  "title.openMini": "打开迷你窗口",
  "title.mini": "迷你窗口",
  "title.collapse": "收起到托盘",
  "title.collapseTitle": "收起到托盘（迷你窗口模式）",
  "title.menu": "Fob 菜单",
  "title.settings": "设置…",
  "title.about": "关于 Fob",
  "title.dismiss": "关闭",
  "tabs.views": "视图",
  "tabs.dashboard": "仪表盘",
  "tabs.processes": "进程",
  "col.name": "进程名称",
  "col.pid": "PID",
  "col.status": "状态",
  "col.user": "用户",
  "col.cpu_usage": "CPU %",
  "col.gpu_usage": "GPU %",
  "col.memory_usage": "内存",
  "col.virtual_memory": "虚拟内存",
  "col.disk_usage": "磁盘 I/O（读/写）",
  "col.ppid": "父进程 PID",
  "col.root": "根目录",
  "col.command": "命令行",
  "col.environ": "环境变量",
  "col.session_id": "会话 ID",
  "col.start_time": "启动时间",
  "col.run_time": "运行时长",
  "col.familyCpu": "进程族 CPU",
  "col.familyRam": "进程族内存",
  "col.familyCpuTitle":
    "整个进程族的 CPU 合计，按全部逻辑处理器计算（与任务管理器一致）。",
  "col.familyRamTitle": "每个进程族的常驻内存之和；共享页可能被重复计算。",
  "col.sortBy": "按{v}排序",
  "col.actions": "操作",
  "col.viewOptions": "视图选项",
  "col.view": "视图",
  "col.visible": "显示的列",
  "col.required": "必选",
  "settings.section": "设置分区",
  "settings.general": "常规",
  "settings.appearance": "外观",
  "settings.generalLead": "选择 Fob 如何监控你的系统。",
  "settings.monitoring": "监控",
  "settings.refresh": "刷新间隔",
  "settings.refreshNote": "适用于主程序和托盘迷你窗口。",
  "settings.refreshMore": "间隔越长，后台活动越少。可在标题栏暂停和继续。",
  "settings.language": "语言",
  "settings.languageLabel": "显示语言",
  "settings.languageNote":
    "“自动”跟随 Windows 显示语言。也可在托盘菜单中切换。",
  "lang.auto": "自动（跟随 Windows）",
  "lang.en": "English",
  "lang.zh": "中文",
  "settings.tray": "托盘迷你窗口",
  "settings.showTray": "显示托盘图标",
  "settings.showTrayNote": "让 CPU、内存、显卡和网络读数随手可查。",
  "settings.startInTray": "仅在托盘启动（迷你窗口模式）",
  "settings.startInTrayNote":
    "Fob 启动时只显示托盘图标，点击即可打开迷你窗口。可从托盘菜单或迷你窗口打开完整窗口。",
  "settings.closeNote":
    "开启托盘图标时，关闭主窗口后 Fob 会继续在托盘运行。要退出，请在托盘菜单中选择“退出”。",
  "settings.appearanceLead": "所有 Fob 主题，适用于每个窗口。",
  "settings.retry": "重试",
  "settings.loading": "正在加载设置…",
  "settings.saving": "正在保存更改…",
  "settings.saved": "更改会自动保存。",
  "about.basedOn": "基于 {v} 的 NeoHtop（MIT）",
  "about.openSource": "开源",
  "about.stack": "技术栈",
  "about.source": "GitHub 上的 NeoHtop 源码",
  "about.updates": "本地构建。替换 fob.exe 即可更新。",
};

export const tables = { en, zh };

/** "auto" follows the Windows display language: Chinese (zh*) or English.
 * @param {string | null | undefined} pref "auto" | "en" | "zh"
 * @param {string | null | undefined} system BCP-47 tag such as "zh-CN"
 * @returns {"en" | "zh"}
 */
export function resolveLocale(pref, system) {
  if (pref === "en" || pref === "zh") return pref;
  return /^zh\b/i.test(system ?? "") ? "zh" : "en";
}

/** Look up `key` and fill "{name}" placeholders; falls back to English, then the key.
 * @param {"en" | "zh"} locale
 * @param {string} key
 * @param {Record<string, string | number>} [vars]
 */
export function translate(locale, key, vars) {
  const text = tables[locale]?.[key] ?? en[key] ?? key;
  return vars
    ? text.replace(/\{(\w+)\}/g, (m, name) =>
        name in vars ? String(vars[name]) : m,
      )
    : text;
}

/** English translator, the default for helpers called without one.
 * @param {string} key
 * @param {Record<string, string | number>} [vars]
 */
export const tEn = (key, vars) => translate("en", key, vars);
