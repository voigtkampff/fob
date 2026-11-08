# fob-perf：Fob 轻量化 A/B 采样工具

只读工具，不安装、不需要管理员、不改 Fob 或系统设置、不联网上传。它附加到正在运行的 Fob，按固定间隔读取 CPU 时间和内存，结果写进你指定的新文件夹。

## 测什么

- 范围：顶层 `fob.exe`，加上它的子进程树（主要是 `msedgewebview2.exe`）。子进程必须父进程号对得上，而且创建时间不早于父进程，所以别的程序的 WebView2 不会算进来
- 不算：传感器助手 `fob-sensors.exe`、`fob-mft.exe`。它们是单独的管理员任务，结果里会列为「排除」
- CPU：累计 CPU 时间的差值 ÷ 实际经过时间 ÷ 全部逻辑处理器数。Windows 11 任务管理器默认按「处理器实用工具」计算，数字可能和这里不同；平均值按时间加权；「最高」是单个采样间隔的最高值，不是瞬时峰值
- 内存：主要看「私有提交内存」（PrivateUsage），这不是任务管理器默认的「内存（活动的专用工作集）」。工作集相加也会给出，但包含共享页，可能重复计算
- 等待阶段（附加后前若干秒）单独报告，不混进稳定段平均值；稳定段还会算私有内存每小时变化趋势
- 不收集命令行、环境变量、文件内容；进程名只记录本次计入或排除的进程

读不到、进程退出、Fob 重启或进程号被复用都会记录次数，并把结果标成「有缺失」或「无效」，不会当成 0

## 使用步骤

1. 解压到任意文件夹，在该文件夹打开「终端」或「命令提示符」
2. 启动 Fob，切到要测的状态，然后运行：

   ```
   fob-perf smoke
   ```

   应只列出 `fob.exe` 和它的 `msedgewebview2.exe`，助手显示为排除，最后一行是「可以开始采样」

3. 先试采一次（约 90 秒），确认范围和数据完整：

   ```
   fob-perf sample --out runs\pilot-tray --label B --app-version 0.8.2 --mode tray --refresh-ms 2000 --helper current --settle-s 30 --steady-s 60
   ```

   `--mode` 只能是 `tray`（只在托盘）、`mini`（迷你窗口）、`statusbar`（状态栏）、`processes`（主窗口进程页）。`--refresh-ms` 填 Fob 里设置的刷新间隔。`--setting 键=值` 可多次填写其他会影响结果的设置。采样时不要切换界面

4. 把 `runs\pilot-tray` 文件夹发回来。里面有 `summary.txt`（可读结果）、`summary.json`、`intervals.csv`、`processes.csv`、原始 `samples.jsonl`

正式 A/B 对比先不要做，等试采结果确认后再定版本和次数

## 对比

```
fob-perf compare --a A --b B --out compare.json runs\A1 runs\B1 runs\A2 ...
```

刷新间隔、机器、采样设置、范围不同的结果会被拒绝。每个状态每边少于 5 次完整结果，或没有交替运行（ABAB），只会显示 inconclusive。两边范围有重叠也只显示 inconclusive
