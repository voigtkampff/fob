# Windows 界面自动验证

此前，Fob 的窗口行为只能靠人在 Windows 桌面上观察。`npm run verify:ui` 用 Windows UI Automation 启动 Fob 并检查关键界面，提供一组可重复的冒烟验证；它不是完整的视觉回归测试。

## 前提

1. WebView2 必须以无障碍模式启动。脚本会为新进程设置 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--force-renderer-accessibility`。缺少该参数时，UIA 通常只能看到空白 Pane。
2. 主窗口必须可见。脚本启动后最多等待 30 秒查找窗口；找不到时会发送一次主窗口热键 `Ctrl+Shift+\``，再等 10 秒。设置 `behavior.startInTray = true` 时应用可能先停在托盘，热键会尝试显示主窗口。

## 检查内容

脚本检查无障碍树元素数超过 100、三个中文页签按钮、进程页至少一行数据、状态栏的 `实时 · N 秒` 文本，以及至少一个纯数字或百分比文本。页签切换会依次尝试 Invoke、Toggle 和 SelectionItem 模式，并在都不可用时报告明确错误。

UIA 能读取 WebView 中的 `Document`、`Group`、`Button`、`Text` 和 `DataItem` 等可访问节点；进程表行可由 `DataItem` 识别。它不能验证 canvas 图表中的绘制图形，也不能判断颜色、布局、间距或对齐是否正确。

## 已知限制

- Fob 使用单实例锁。默认检测到已有 `fob` 进程时拒绝运行，避免影响现有窗口状态。只有显式传入 `-Force` 并在确认提示输入 `KILL` 后才会结束现有实例；输出会列出被结束的 PID。
- 页签虽暴露为 `Button`，但当前不支持 Invoke 模式。脚本按 Invoke、Toggle、SelectionItem 的顺序尝试，不会静默假定 Invoke 一定可用。
- `behavior.startInTray = true` 会隐藏主窗口。脚本会尝试热键唤出，但若窗口仍不可见，检查会失败并说明原因。

## 使用

```powershell
npm run verify:ui
# 或指定一个 Windows 可执行文件
powershell -ExecutionPolicy Bypass -File scripts/verify-ui.ps1 -Exe .\dist\fob-v0.8.24\fob.exe
# 调试时保留脚本启动的进程
powershell -ExecutionPolicy Bypass -File scripts/verify-ui.ps1 -Exe .\dist\fob-v0.8.24\fob.exe -KeepOpen
```

不传 `-Exe` 时脚本会从 `dist/` 中选择最后修改的 `fob.exe`。退出码：0 为检查通过，1 为检查失败或因已有实例/未确认而停止，2 为参数或可执行文件错误，3 为非 Windows、缺少 UI Automation 或缺少 WebView2 Runtime。
