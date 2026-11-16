// fob-sensors: READ-ONLY sensor helper for Fob.
// Reads CPU / GPU / motherboard temperatures and fan speeds through
// LibreHardwareMonitorLib (needs admin + the PawnIO driver) and serves them on
// the named pipe \\.\pipe\fob-sensors, one JSON line per second.
//  - Never changes anything: IsControllerEnabled = false and no SetSoftware()
//    call exists in this program. Fan "Control" values are only read.
//  - Started at logon by the "fob-sensors" scheduled task (highest
//    privileges). It keeps running; while no client is connected it does not
//    touch the hardware, it only sleeps.
//  - While a client is connected it also runs the "fob-net" ETW session
//    (Microsoft-Windows-Kernel-Network) for per-process network bytes; it is
//    stopped as soon as no client is connected, and on exit.
//  - LibreHardwareMonitorLib takes global mutexes around ISA/SMBus access, so
//    this coexists with FanControl, HWiNFO-style tools and LHM itself.
using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Pipes;
using System.Security.AccessControl;
using System.Security.Principal;
using System.Text;
using System.Threading;
using LibreHardwareMonitor.Hardware;
using LibreHardwareMonitor.PawnIo;

namespace WinperfSensors
{
    static class Program
    {
        const string PipeName = "fob-sensors";
        const int MaxClients = 4;

        sealed class Snapshot
        {
            public long Ts;
            public byte[] Bytes;
        }

        static Snapshot latest;
        static MsrReader msr;
        static int clients;
        static readonly NetAgg netAgg = new NetAgg();
        static NetTrace net;
        static long netSince;
        const int MaxNetProcs = 64;

        static int Main(string[] args)
        {
            bool fresh;
            using (var mutex = new Mutex(true, @"Local\fob-sensors-helper", out fresh))
            {
                if (!fresh) return 0; // already running in this session
                try
                {
                    return Run(Array.IndexOf(args, "--once") >= 0);
                }
                catch (Exception e)
                {
                    Log("fatal: " + e);
                    return 1;
                }
            }
        }

        static int Run(bool once)
        {
            var computer = new Computer
            {
                IsCpuEnabled = true,
                IsGpuEnabled = true,
                IsMotherboardEnabled = true,
                IsControllerEnabled = false, // read-only: no fan controller access
                IsStorageEnabled = false,
                IsMemoryEnabled = false,
                IsNetworkEnabled = false,
                IsPsuEnabled = false,
                IsBatteryEnabled = false,
            };
            computer.Open();
            msr = new MsrReader();
            Log("started");
            NetTrace.StopStale();
            net = new NetTrace(netAgg);
            AppDomain.CurrentDomain.ProcessExit += (_, __) => net.Stop();
            try
            {
                if (once)
                {
                    Update(computer);
                    Console.Out.Write(Encoding.UTF8.GetString(latest.Bytes));
                    return 0;
                }
                MftService.Start();
                new Thread(Accept) { IsBackground = true, Name = "pipe" }.Start();
                while (true)
                {
                    var connected = Volatile.Read(ref clients) > 0;
                    if (connected && !net.Running && net.Start())
                        netSince = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
                    else if (!connected && net.Running)
                        net.Stop();
                    if (connected) Update(computer);
                    Thread.Sleep(1000);
                }
            }
            finally
            {
                msr?.Close();
                net?.Stop();
                computer.Close();
            }
        }

        static void Update(Computer computer)
        {
            var list = new List<Reading>();
            foreach (var hw in computer.Hardware) Walk(hw, list);
            var ts = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            List<NetUsage> procs = null;
            long ms = 0;
            if (net != null && net.Running)
            {
                procs = netAgg.Drain();
                if (procs.Count > MaxNetProcs) procs.RemoveRange(MaxNetProcs, procs.Count - MaxNetProcs);
                for (var i = 0; i < procs.Count; i++)
                {
                    var p = procs[i];
                    p.Name = ProcessName(p.Pid);
                    procs[i] = p;
                }
                ms = Math.Max(1, ts - netSince);
                netSince = ts;
            }
            var line = Wire.UpdateLine(ts, list, procs, ms, () => msr?.Read()) + "\n";
            Volatile.Write(ref latest, new Snapshot { Ts = ts, Bytes = Encoding.UTF8.GetBytes(line) });
        }

        static string ProcessName(uint pid)
        {
            if (pid == 0) return "Idle";
            if (pid == 4) return "System";
            try
            {
                using (var p = System.Diagnostics.Process.GetProcessById((int)pid)) return p.ProcessName;
            }
            catch { return null; } // exited
        }

        /// <summary>
        /// Reads the Intel thermal status MSRs through LHM's public PawnIO
        /// path. The helper already runs elevated; without that, PawnIO
        /// returns E_ACCESSDENIED. Any failure degrades to "not supported"
        /// and is logged once, never once per second.
        /// </summary>
        sealed class MsrReader
        {
            IntelMsr msr;
            readonly ThrottleWindow window = new ThrottleWindow();
            bool available;
            uint? lastPackageTime;

            public MsrReader()
            {
                try
                {
                    msr = new IntelMsr();
                    available = true;
                }
                catch (Exception e)
                {
                    Log("throttle: " + e.GetType().Name + ": " + e.Message);
                }
            }

            public ThrottleState Read()
            {
                if (!available || msr == null) return null;
                try
                {
                    var thermal = false;
                    var thermalLog = false;
                    var prochot = false;
                    var prochotLog = false;
                    int? distance = null;
                    var any = false;

                    ulong packageStatus;
                    if (msr.ReadMsr(0x1B1, out packageStatus))
                    {
                        any = true;
                        thermal |= MsrDecode.Thermal(packageStatus);
                        thermalLog |= MsrDecode.ThermalLog(packageStatus);
                        prochot |= MsrDecode.Prochot(packageStatus);
                        prochotLog |= MsrDecode.ProchotLog(packageStatus);
                        distance = MinDistance(distance, MsrDecode.DistanceToTjMax(packageStatus));
                    }

                    var cores = Math.Min(64, Environment.ProcessorCount);
                    for (var i = 0; i < cores; i++)
                    {
                        uint eax, edx;
                        if (!msr.ReadMsr(0x19C, out eax, out edx, GroupAffinity.Single(0, i))) continue;
                        var value = ((ulong)edx << 32) | eax;
                        any = true;
                        thermal |= MsrDecode.Thermal(value);
                        thermalLog |= MsrDecode.ThermalLog(value);
                        prochot |= MsrDecode.Prochot(value);
                        prochotLog |= MsrDecode.ProchotLog(value);
                        distance = MinDistance(distance, MsrDecode.DistanceToTjMax(value));
                    }

                    if (!any)
                    {
                        available = false;
                        Log("throttle: no thermal status MSR readable");
                        return null;
                    }

                    var package = false;
                    ulong performance;
                    if (msr.ReadMsr(0x613, out performance))
                    {
                        var now = (uint)performance;
                        package = MsrDecode.PackageTimerAdvanced(now, lastPackageTime);
                        lastPackageTime = now;
                    }

                    window.Add(thermal, package);
                    return new ThrottleState
                    {
                        Supported = true,
                        Thermal = thermal,
                        ThermalLog = thermalLog,
                        Prochot = prochot,
                        ProchotLog = prochotLog,
                        Package = package,
                        DistanceToTjMax = distance,
                        ThermalPercent = window.ThermalPercent,
                        PackagePercent = window.PackagePercent,
                    };
                }
                catch (Exception e)
                {
                    available = false;
                    Log("throttle: " + e.GetType().Name + ": " + e.Message);
                    return null;
                }
            }

            static int? MinDistance(int? current, int next) =>
                current.HasValue ? Math.Min(current.Value, next) : next;

            public void Close()
            {
                try { msr?.Close(); } catch { }
                msr = null;
                available = false;
            }
        }

        static void Walk(IHardware hw, List<Reading> list)
        {
            hw.Update();
            // Super I/O chips are sub-hardware of the board: label them with the board name.
            var label = hw.Parent != null ? hw.Parent.Name : hw.Name;
            foreach (var s in hw.Sensors)
            {
                var reading = new Reading(label, s.Identifier.ToString(), s.Name, s.SensorType.ToString(), s.Value);
                if (Wire.Wanted(reading)) list.Add(reading);
            }
            foreach (var sub in hw.SubHardware) Walk(sub, list);
        }

        static PipeSecurity Security()
        {
            var sec = new PipeSecurity();
            // No remote access; the interactive user may only read.
            sec.AddAccessRule(new PipeAccessRule(new SecurityIdentifier(WellKnownSidType.NetworkSid, null),
                PipeAccessRights.FullControl, AccessControlType.Deny));
            sec.AddAccessRule(new PipeAccessRule(new SecurityIdentifier(WellKnownSidType.BuiltinAdministratorsSid, null),
                PipeAccessRights.FullControl, AccessControlType.Allow));
            sec.AddAccessRule(new PipeAccessRule(new SecurityIdentifier(WellKnownSidType.LocalSystemSid, null),
                PipeAccessRights.FullControl, AccessControlType.Allow));
            sec.AddAccessRule(new PipeAccessRule(new SecurityIdentifier(WellKnownSidType.InteractiveSid, null),
                PipeAccessRights.Read | PipeAccessRights.Synchronize, AccessControlType.Allow));
            return sec;
        }

        static void Accept()
        {
            var sec = Security();
            while (true)
            {
                NamedPipeServerStream server = null;
                try
                {
                    server = new NamedPipeServerStream(PipeName, PipeDirection.Out, MaxClients,
                        PipeTransmissionMode.Byte, PipeOptions.None, 0, 64 * 1024, sec);
                    server.WaitForConnection();
                    var s = server;
                    server = null;
                    new Thread(() => Serve(s)) { IsBackground = true, Name = "client" }.Start();
                }
                catch (Exception e)
                {
                    server?.Dispose();
                    Log("accept: " + e.Message);
                    Thread.Sleep(2000); // all instances busy, or the name is taken
                }
            }
        }

        static void Serve(NamedPipeServerStream pipe)
        {
            Interlocked.Increment(ref clients);
            var since = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds() - 1500;
            long sent = 0;
            try
            {
                while (pipe.IsConnected)
                {
                    var snap = Volatile.Read(ref latest);
                    // Skip readings older than the connection (taken before an idle gap).
                    if (snap != null && snap.Ts > sent && snap.Ts >= since)
                    {
                        pipe.Write(snap.Bytes, 0, snap.Bytes.Length);
                        pipe.Flush();
                        sent = snap.Ts;
                    }
                    Thread.Sleep(200);
                }
            }
            catch (IOException) { } // client went away
            catch (ObjectDisposedException) { }
            finally
            {
                Interlocked.Decrement(ref clients);
                pipe.Dispose();
            }
        }

        static readonly object logLock = new object();

        internal static void Log(string message)
        {
            try
            {
                lock (logLock)
                {
                    var path = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "fob-sensors.log");
                    var info = new FileInfo(path);
                    if (info.Exists && info.Length > 256 * 1024) info.Delete();
                    File.AppendAllText(path, DateTime.Now.ToString("s") + " " + message + Environment.NewLine);
                }
            }
            catch { }
        }
    }
}
