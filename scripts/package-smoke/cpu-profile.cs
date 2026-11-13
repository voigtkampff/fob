using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.IO;
using System.Text;
using System.Threading;

namespace FobStartup {
    public class Row {
        public uint Pid, Parent;
        public string Name;
    }
    public class Tracked {
        public uint Pid, Parent;
        public long Created, ExitedAt;
        public string Name, Group, Key, HelperSha256;
        public double BaselineCpu, Cpu, FirstSeen;
        public bool Exited, Valid = true;
        public uint? ExitCode;
        public IntPtr Handle;
        public double Total { get { return Math.Max(0, Cpu - BaselineCpu); } }
    }
    public class Point {
        public string Key;
        public double CpuSeconds;
        public bool Exited, Valid;
    }
    public class Sample {
        public double WallSeconds, SystemBusyCpuSeconds, ProfilerCpuSeconds;
        public List<Point> Processes;
    }
    public class Usage {
        public string Group;
        public double CpuSeconds, OneCorePercent, MachinePercent;
        public bool Complete;
    }
    public class Phase {
        public double NominalStart, NominalEnd, WallStart, WallEnd, WallSeconds;
        public double SystemBusyCpuSeconds, ProfilerCpuSeconds;
        public List<Usage> Groups;
        public Dictionary<string, double> ProcessCpuSeconds;
    }
    public class Cleanup {
        public string Key, Reason;
        public bool TerminationRequested, Gone;
    }
    public class RunResult {
        public double LaunchApiSeconds, DurationSeconds, MaxSampleGapSeconds, DiscoveryWallSeconds;
        public double? MainWindowVisibleSeconds;
        public uint RootPid;
        public int LogicalProcessors, SamplesCount, FailedCandidateCount;
        public bool RootExitedDuringMeasurement, RootAliveAtEnd;
        public string Screenshot;
        public double CleanupStartedSeconds;
        public List<string> Errors = new List<string>();
        public List<Tracked> Processes;
        public List<Sample> Samples = new List<Sample>();
        public List<Phase> Phases = new List<Phase>();
        public List<Cleanup> Cleanup = new List<Cleanup>();
        public string ShortLivedProcessGap = "Unknown unobserved process count/CPU: polling can miss an entire lifetime between snapshots. Totals are observed-process totals, not guaranteed full-tree totals.";
    }
    public static class Accounting {
        public static bool ChildOf(long parentCreated, long parentExited, long childCreated) {
            return childCreated >= parentCreated && (parentExited == 0 || childCreated <= parentExited);
        }
        public static Usage Delta(string group, double previous, double current, double wall, int cores, bool complete) {
            if (current < previous || wall <= 0 || cores <= 0) throw new Exception("Invalid CPU accounting");
            double cpu = current - previous;
            return new Usage { Group=group, CpuSeconds=cpu, OneCorePercent=100*cpu/wall,
                MachinePercent=100*cpu/wall/cores, Complete=complete };
        }
        public static Phase BuildPhase(ICollection<Tracked> records,Sample current,Sample prior,double nominalStart,double nominalEnd,int cores) {
            double wall=current.WallSeconds-(prior==null?0:prior.WallSeconds);
            var previous=prior==null ? new Dictionary<string,double>() : prior.Processes.ToDictionary(p=>p.Key,p=>p.CpuSeconds);
            var delta=current.Processes.ToDictionary(p=>p.Key,p=>p.CpuSeconds-(previous.ContainsKey(p.Key)?previous[p.Key]:0));
            var phase=new Phase { NominalStart=nominalStart,NominalEnd=nominalEnd,WallStart=prior==null?0:prior.WallSeconds,
                WallEnd=current.WallSeconds,WallSeconds=wall,ProcessCpuSeconds=delta,
                SystemBusyCpuSeconds=current.SystemBusyCpuSeconds-(prior==null?0:prior.SystemBusyCpuSeconds),
                ProfilerCpuSeconds=current.ProfilerCpuSeconds-(prior==null?0:prior.ProfilerCpuSeconds),Groups=new List<Usage>() };
            foreach(string group in new[]{"root","webview","other-descendant","helper"}) {
                var members=records.Where(p=>p.Group==group).ToList();
                phase.Groups.Add(Accounting.Delta(group,0,members.Sum(p=>delta[p.Key]),wall,cores,members.All(p=>p.Valid)));
            }
            return phase;
        }
    }
    public static class Native {
        [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct Entry {
            public uint Size, Usage, Pid;
            public UIntPtr Heap;
            public uint Module, Threads, Parent;
            public int Priority;
            public uint Flags;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst=260)] public string Name;
        }
        [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr CreateToolhelp32Snapshot(uint flags,uint pid);
        [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool Process32FirstW(IntPtr snap, ref Entry e);
        [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool Process32NextW(IntPtr snap, ref Entry e);
        [DllImport("kernel32.dll",SetLastError=true)] public static extern IntPtr OpenProcess(uint access,bool inherit,uint pid);
        [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
        [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetProcessTimes(IntPtr h,out long created,out long exited,out long kernel,out long user);
        [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr h,uint ms);
        [DllImport("kernel32.dll")] static extern bool GetExitCodeProcess(IntPtr h,out uint code);
        [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetSystemTimes(out long idle,out long kernel,out long user);
        [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] static extern bool QueryFullProcessImageNameW(IntPtr h,uint flags,StringBuilder text,ref int size);
        [DllImport("kernel32.dll")] static extern bool TerminateProcess(IntPtr h,uint code);
        delegate bool EnumProc(IntPtr h,IntPtr l);
        [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb,IntPtr l);
        [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
        [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
        [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
        [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowTextW(IntPtr h,StringBuilder s,int n);
        public static List<Row> Snapshot() {
            IntPtr snap=CreateToolhelp32Snapshot(2,0);
            if(snap == new IntPtr(-1)) throw new Exception("Toolhelp snapshot failed: "+Marshal.GetLastWin32Error());
            try {
                Entry e=new Entry { Size=(uint)Marshal.SizeOf(typeof(Entry)) };
                if(!Process32FirstW(snap,ref e)) throw new Exception("Process32First failed");
                var rows=new List<Row>();
                do { rows.Add(new Row { Pid=e.Pid,Parent=e.Parent,Name=e.Name }); } while(Process32NextW(snap,ref e));
                if(Marshal.GetLastWin32Error()!=18) throw new Exception("Process32Next failed");
                return rows;
            } finally { CloseHandle(snap); }
        }
        public static Tracked Open(Row row,string group,double firstSeen) {
            IntPtr handle=OpenProcess(0x00101000,false,row.Pid);
            if(handle == IntPtr.Zero) return null;
            var p=new Tracked { Pid=row.Pid,Parent=row.Parent,Name=row.Name,Group=group,Handle=handle,FirstSeen=firstSeen };
            if(!Read(p)) { CloseHandle(handle); return null; }
            p.Key=p.Pid+":"+p.Created;
            if(group=="helper") {
                int n=32768; var text=new StringBuilder(n);
                try { if(QueryFullProcessImageNameW(handle,0,text,ref n)) {
                    using(var stream=File.OpenRead(text.ToString())) using(var sha=SHA256.Create())
                        p.HelperSha256=BitConverter.ToString(sha.ComputeHash(stream)).Replace("-","").ToLowerInvariant();
                }} catch(IOException) {} catch(UnauthorizedAccessException) {}
            }
            return p;
        }
        public static bool Read(Tracked p) {
            long created,exited,kernel,user;
            if(!GetProcessTimes(p.Handle,out created,out exited,out kernel,out user)) { p.Valid=false; return false; }
            double cpu=(kernel+user)/10000000.0;
            if((p.Created!=0 && p.Created!=created) || cpu<p.Cpu) { p.Valid=false; return false; }
            p.Created=created; p.Cpu=cpu;
            p.Exited=WaitForSingleObject(p.Handle,0)==0;
            if(p.Exited) { p.ExitedAt=exited; uint code; if(GetExitCodeProcess(p.Handle,out code)) p.ExitCode=code; }
            return true;
        }
        public static double Busy() {
            long idle,kernel,user;
            if(!GetSystemTimes(out idle,out kernel,out user)) throw new Exception("GetSystemTimes failed");
            return (kernel-idle+user)/10000000.0;
        }
        public static IntPtr MainWindow(uint pid) {
            IntPtr visible=IntPtr.Zero;
            EnumWindows((h,l)=>{ uint p; GetWindowThreadProcessId(h,out p);
                if(p==pid && IsWindowVisible(h) && !IsIconic(h)) {
                    var text=new StringBuilder(128); GetWindowTextW(h,text,128);
                    if(text.ToString()=="Fob") visible=h;
                } return true;
            },IntPtr.Zero);
            return visible;
        }
        public static Cleanup StopOwned(Tracked p) {
            var result=new Cleanup { Key=p.Key,Reason="forced_after_measurement_not_normal_exit" };
            if(Read(p) && p.Exited) { result.Gone=true; result.Reason="exited_before_cleanup"; return result; }
            IntPtr h=OpenProcess(0x00101001,false,p.Pid);
            if(h==IntPtr.Zero) { result.Gone=p.Exited; return result; }
            try {
                long created,exit,kernel,user;
                if(!GetProcessTimes(h,out created,out exit,out kernel,out user) || created!=p.Created) return result;
                if(WaitForSingleObject(h,0)!=0) result.TerminationRequested=TerminateProcess(h,0xD1A6);
                result.Gone=WaitForSingleObject(h,3000)==0;
                return result;
            } finally { CloseHandle(h); }
        }
    }
    public sealed class Collector : IDisposable {
        readonly Dictionary<string,Tracked> ledger=new Dictionary<string,Tracked>();
        readonly HashSet<uint> failedCandidates=new HashSet<uint>();
        readonly Stopwatch clock=new Stopwatch();
        readonly Process self=Process.GetCurrentProcess();
        readonly RunResult result=new RunResult { LogicalProcessors=Environment.ProcessorCount };
        long launchFileTime;
        double busyStart,selfStart,lastSample;
        static bool Helper(Row r) { return string.Equals(r.Name,"fob-sensors.exe",StringComparison.OrdinalIgnoreCase); }
        static bool WebView(Row r) { return string.Equals(r.Name,"msedgewebview2.exe",StringComparison.OrdinalIgnoreCase); }
        static double OwnCpu(Process p) { p.Refresh(); return p.TotalProcessorTime.TotalSeconds; }
        void Add(Tracked p,bool baseline) {
            if(p==null) throw new Exception("Root/helper CPU handle unavailable");
            if(baseline) p.BaselineCpu=p.Cpu;
            ledger.Add(p.Key,p);
        }
        void Discover() {
            var watch=Stopwatch.StartNew();
            var rows=Native.Snapshot(); bool added;
            do {
                added=false;
                foreach(var row in rows) {
                    if(ledger.Values.Any(p=>p.Pid==row.Pid)) continue;
                    var parent=ledger.Values.FirstOrDefault(p=>p.Pid==row.Parent && p.Group!="helper");
                    if(parent==null && !Helper(row)) continue;
                    string group=parent==null ? "helper" : (WebView(row)?"webview":"other-descendant");
                    var child=Native.Open(row,group,clock.Elapsed.TotalSeconds);
                    if(child==null) { failedCandidates.Add(row.Pid); continue; }
                    if(parent!=null && !Accounting.ChildOf(parent.Created,parent.ExitedAt,child.Created)) {
                        Native.CloseHandle(child.Handle); continue;
                    }
                    Add(child,group=="helper" && child.Created<launchFileTime); added=true;
                }
            } while(added);
            result.DiscoveryWallSeconds+=watch.Elapsed.TotalSeconds;
        }
        Sample TakeSample() {
            foreach(var p in ledger.Values) if(!Native.Read(p)) result.Errors.Add("CPU read failed: "+p.Key);
            Discover();
            var sample=new Sample { Processes=ledger.Values.Select(p=>new Point { Key=p.Key,CpuSeconds=p.Total,Exited=p.Exited,Valid=p.Valid }).ToList(),
                SystemBusyCpuSeconds=Native.Busy()-busyStart,ProfilerCpuSeconds=OwnCpu(self)-selfStart };
            sample.WallSeconds=clock.Elapsed.TotalSeconds;
            result.MaxSampleGapSeconds=Math.Max(result.MaxSampleGapSeconds,sample.WallSeconds-lastSample);
            lastSample=sample.WallSeconds;
            if(!result.MainWindowVisibleSeconds.HasValue && Native.MainWindow(result.RootPid)!=IntPtr.Zero) result.MainWindowVisibleSeconds=clock.Elapsed.TotalSeconds;
            result.Samples.Add(sample);
            return sample;
        }
        public RunResult Run(string exe,string screenshotScript,string screenshotPath) {
            foreach(var helper in Native.Snapshot().Where(Helper)) Add(Native.Open(helper,"helper",-1),true);
            busyStart=Native.Busy(); selfStart=OwnCpu(self);
            clock.Start(); launchFileTime=DateTime.UtcNow.ToFileTimeUtc();
            using(var root=Process.Start(new ProcessStartInfo(exe) { UseShellExecute=false,WorkingDirectory=Path.GetDirectoryName(exe) })) {
                if(root==null) throw new Exception("Process.Start returned null");
                result.RootPid=(uint)root.Id; result.LaunchApiSeconds=clock.Elapsed.TotalSeconds;
                try {
                    Add(Native.Open(new Row { Pid=result.RootPid,Name="fob.exe" },"root",clock.Elapsed.TotalSeconds),false);
                    Sample prior=null; double[] boundaries={10,30,60}; int phase=0;
                    while(phase<boundaries.Length) {
                        var sample=TakeSample();
                        var rootRecord=ledger.Values.Single(p=>p.Group=="root");
                        if(rootRecord.Exited) { result.RootExitedDuringMeasurement=true; result.Errors.Add("Root exited during measurement"); break; }
                        if(sample.WallSeconds>=boundaries[phase]) {
                            result.Phases.Add(Accounting.BuildPhase(ledger.Values,sample,prior,phase==0?0:boundaries[phase-1],boundaries[phase],result.LogicalProcessors)); prior=sample; phase++;
                        }
                        if(phase<boundaries.Length) Thread.Sleep(Math.Max(1,(int)Math.Min(250,(boundaries[phase]-clock.Elapsed.TotalSeconds)*1000)));
                    }
                    result.DurationSeconds=clock.Elapsed.TotalSeconds;
                    result.RootAliveAtEnd=!root.HasExited;
                    result.SamplesCount=result.Samples.Count; result.FailedCandidateCount=failedCandidates.Count;
                    result.Processes=ledger.Values.Select(p=>new Tracked { Pid=p.Pid,Parent=p.Parent,Created=p.Created,ExitedAt=p.ExitedAt,
                        Name=p.Name,Group=p.Group,Key=p.Key,HelperSha256=p.HelperSha256,BaselineCpu=p.BaselineCpu,Cpu=p.Cpu,
                        FirstSeen=p.FirstSeen,Exited=p.Exited,Valid=p.Valid,ExitCode=p.ExitCode }).ToList();
                    if(!result.MainWindowVisibleSeconds.HasValue) result.Errors.Add("No visible Fob main window observed");
                    if(!ledger.Values.Any(p=>p.Group=="webview")) result.Errors.Add("No owned WebView observed");
                    if(result.FailedCandidateCount>0) result.Errors.Add("Unopened candidate processes: "+result.FailedCandidateCount);
                    if(!string.IsNullOrEmpty(screenshotPath)) {
                        IntPtr window=Native.MainWindow(result.RootPid);
                        if(window!=IntPtr.Zero) {
                            var shot=new ProcessStartInfo("powershell") { UseShellExecute=false,CreateNoWindow=true,RedirectStandardOutput=true,RedirectStandardError=true };
                            foreach(string arg in new[]{"-NoProfile","-File",screenshotScript,"window-shot",window.ToInt64().ToString(),screenshotPath}) shot.ArgumentList.Add(arg);
                            using(var capture=Process.Start(shot)) {
                                if(!capture.WaitForExit(10000)) { capture.Kill(true); capture.WaitForExit(3000); result.Errors.Add("Post-measurement screenshot timed out"); }
                                else if(capture.ExitCode!=0 || !File.Exists(screenshotPath)) result.Errors.Add("Post-measurement screenshot failed");
                                else result.Screenshot=Path.GetFileName(screenshotPath);
                            }
                        }
                    }
                } finally {
                    result.CleanupStartedSeconds=clock.Elapsed.TotalSeconds;
                    if(!ledger.Values.Any(p=>p.Group=="root") && !root.HasExited) {
                        root.Kill(); root.WaitForExit(3000);
                        result.Cleanup.Add(new Cleanup { Key=result.RootPid+":untracked-root",Reason="forced_after_measurement_not_normal_exit",TerminationRequested=true,Gone=root.HasExited });
                    }
                    foreach(var p in ledger.Values.Where(p=>p.Group!="helper").OrderBy(p=>p.Group=="root"?0:1)) result.Cleanup.Add(Native.StopOwned(p));
                }
            }
            return result;
        }
        public void Dispose() { foreach(var p in ledger.Values) Native.CloseHandle(p.Handle); self.Dispose(); }
    }
    public static class Probe {
        public static RunResult Measure(string exe,string screenshotScript,string screenshotPath) { using(var c=new Collector()) return c.Run(exe,screenshotScript,screenshotPath); }
        public static object Baseline() {
            using(var self=Process.GetCurrentProcess()) {
                double cpu=self.TotalProcessorTime.TotalSeconds,busy=Native.Busy(); int polls=0; var watch=Stopwatch.StartNew();
                while(watch.Elapsed.TotalSeconds<3) { Native.Snapshot(); polls++; Thread.Sleep(250); }
                self.Refresh(); double wall=watch.Elapsed.TotalSeconds;
                return new { WallSeconds=wall,ProfilerCpuSeconds=self.TotalProcessorTime.TotalSeconds-cpu,
                    SystemBusyCpuSeconds=Native.Busy()-busy,SnapshotCount=polls };
            }
        }
        public static int SelfTest() {
            int checks=0;
            Action<bool> check=(ok)=>{if(!ok)throw new Exception("Accounting fixture failed at "+checks);checks++;};
            var fresh=new Tracked { Cpu=2.5,BaselineCpu=0 };
            var existing=new Tracked { Cpu=123,BaselineCpu=120 };
            check(fresh.Total==2.5); check(existing.Total==3);
            var d=Accounting.Delta("root",1,3,10,4,true);
            check(d.CpuSeconds==2 && d.OneCorePercent==20 && d.MachinePercent==5);
            check(Accounting.ChildOf(100,0,101)); check(!Accounting.ChildOf(100,0,99));
            check(Accounting.ChildOf(100,150,149)); check(!Accounting.ChildOf(100,150,151));
            check((new Tracked { Pid=1,Created=100 }).Created!=(new Tracked { Pid=1,Created=200 }).Created);
            var unknown=Accounting.Delta("root",0,0,10,4,false); check(!unknown.Complete);
            bool bad=false; try{Accounting.Delta("root",3,1,10,4,true);}catch(Exception){bad=true;} check(bad);
            check(new Cleanup { Reason="forced_after_measurement_not_normal_exit" }.Reason!="normal_exit");
            var records=new List<Tracked> { new Tracked { Key="root:100",Group="root" },new Tracked { Key="child:101",Group="webview",Exited=true } };
            var first=new Sample { WallSeconds=10.1, Processes=new List<Point> { new Point { Key="root:100",CpuSeconds=.5 },new Point { Key="child:101",CpuSeconds=2 } } };
            var phase=Accounting.BuildPhase(records,first,null,0,10,4);
            check(phase.Groups.Single(g=>g.Group=="webview").CpuSeconds==2);
            records.Add(new Tracked { Key="child:200",Group="webview" });
            var next=new Sample { WallSeconds=30.2,Processes=new List<Point> { new Point { Key="root:100",CpuSeconds=.75 },new Point { Key="child:101",CpuSeconds=2 },new Point { Key="child:200",CpuSeconds=3 } } };
            var later=Accounting.BuildPhase(records,next,first,10,30,4);
            check(later.Groups.Single(g=>g.Group=="webview").CpuSeconds==3);
            check(later.ProcessCpuSeconds["child:101"]==0 && later.ProcessCpuSeconds["child:200"]==3);
            check(later.ProcessCpuSeconds["root:100"]==.25);
            check(Math.Abs(later.WallSeconds-20.1)<.000001);
            records[0].Valid=false;
            check(!Accounting.BuildPhase(records,next,first,10,30,4).Groups.Single(g=>g.Group=="root").Complete);
            if(RuntimeInformation.IsOSPlatform(OSPlatform.Windows)) {
                using(var self=Process.GetCurrentProcess()) {
                    var row=Native.Snapshot().Single(r=>r.Pid==(uint)self.Id);
                    var p=Native.Open(row,"profiler",0);
                    try { check(p!=null && p.Created==self.StartTime.ToUniversalTime().ToFileTimeUtc() && !p.Exited && Native.Read(p)); }
                    finally { if(p!=null) Native.CloseHandle(p.Handle); }
                    check(Native.Busy()>0);
                }
            }
            return checks;
        }
    }
}
