# Defender "Behavior:Win32/Execution.A!ml" on winperf 0.4 (now Fob)

## What happened

Microsoft Defender flagged winperf 0.4.0 on the owner's PC as
`Behavior:Win32/Execution.A!ml` (severe, suspicious behaviour). The alert did
not name the file or process.

## Most likely trigger (hypothesis, not confirmed)

`Behavior:` detections come from Defender's behaviour monitoring, not from a
file signature. `!ml` means a cloud machine-learning model scored a chain of
actions. winperf 0.4 produced this chain:

1. An unsigned exe with little or no reputation, started from a user folder
   (Downloads or Desktop), relaunched itself elevated (`runas`).
2. The elevated copy spawned `schtasks.exe /Create /XML …` to register a
   logon task with `HighestAvailable` rights (MITRE ATT&CK T1053.005, a
   common persistence technique), ran `PawnIO_setup.exe -install -silent`
   (a kernel driver installer, run silently), and copied an exe into
   Program Files.
3. After that, the unelevated app kept spawning `schtasks.exe /Run` (every 60
   s while the helper was away) and `schtasks.exe /Query /XML` (on every status
   refresh).

The model most likely saw "unknown binary → elevation → schtasks persistence →
silent driver install → repeated LOLBin child processes". Each step is legitimate
on its own; together they look like a dropper. The repeated `schtasks.exe` child
processes from an unsigned exe are the strongest single signal.

## What Fob 0.5.0 changes

- No `schtasks.exe`: the task is created, run, stopped, read and deleted
  through the Task Scheduler COM API (ITaskService). Fob starts no child
  processes, except PawnIO's own installer, and never starts cmd or PowerShell.
- The PawnIO installer runs with its own visible window (no `-silent`).
- The background "start the helper" call backs off from 1 minute to 30
  minutes, and it is an in-process API call.
- Full VERSIONINFO on both exes (CompanyName HCB (a personal name before 0.7.14), ProductName Fob,
  FileDescription, LegalCopyright, OriginalFilename, InternalName, versions) and
  application manifests (asInvoker, Windows 10/11 supportedOS).
- Every shipped exe and dll is listed with its SHA-256 in `SHA256SUMS.txt`
  inside the zip.

This lowers the risk. It doesn't guarantee that the model won't flag the new
build: Fob still elevates once and registers a logon task, which is its job.

## Submitting a false positive (the owner does this, nothing was submitted)

Site: https://www.microsoft.com/wdsi/filesubmission. Choose "Home customer",
then "Incorrectly detected as malware/malicious". Upload the zip, or `fob.exe`
and `sensors\fob-sensors.exe`.

Detection name: `Behavior:Win32/Execution.A!ml`

Text:

> Fob (formerly winperf) is a portable, open-source Windows performance monitor
> that I build myself (fork of NeoHtop, MIT). Version 0.4.0 was flagged as
> Behavior:Win32/Execution.A!ml when its optional sensor setup ran. That setup
> asks for UAC once, installs the signed PawnIO driver (namazso), copies a
> read-only sensor helper (LibreHardwareMonitorLib) to Program Files and
> registers a logon scheduled task so that the helper can read temperatures
> without further prompts. Nothing is downloaded and no data is sent anywhere.
> Version 0.6.3 (attached) uses the Task Scheduler API instead of schtasks.exe
> and runs the driver installer visibly. SHA-256 of the files: see the list
> below. Please review the detection and clear it for these files.

Paste the lines of `SHA256SUMS.txt` (at least `fob.exe` and
`sensors/fob-sensors.exe`) under the text.

## Code signing (option, nothing bought)

A signature gives the files a publisher identity and builds SmartScreen and
Defender reputation over time. Prices are approximate and must be checked
before buying:

- Microsoft Azure Trusted Signing (Artifact Signing): about US$10 per month
  (Basic). It needs an Azure subscription and identity validation, and it is
  limited to some countries and to organisations or individuals that Microsoft
  accepts.
- OV code-signing certificate from a commercial CA (for example Sectigo,
  SSL.com or Certum): about US$200–400 per year. Since 2023 the private key
  must sit on a hardware token or in a cloud HSM.
- EV certificate: about US$300–600 per year. It no longer bypasses
  SmartScreen reputation on its own.
- Certum Open Source code signing: about €25–60 per year, for open-source
  developers.
