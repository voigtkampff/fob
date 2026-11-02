// winperf-sensors: READ-ONLY sensor helper for winperf.
// Reads CPU / GPU / motherboard temperatures and fan speeds through
// LibreHardwareMonitorLib (needs admin + the PawnIO driver) and serves them on
// the named pipe \\.\pipe\winperf-sensors, one JSON line per second.
//  - Never changes anything: IsControllerEnabled = false and no SetSoftware()
//    call exists in this program. Fan "Control" values are only read.
//  - Started at logon by the "winperf-sensors" scheduled task (highest
//    privileges). It keeps running; while no client is connected it does not
//    touch the hardware, it only sleeps.
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

namespace WinperfSensors
{
    static class Program
    {
        const string PipeName = "winperf-sensors";
        const int MaxClients = 4;

        sealed class Snapshot
        {
            public long Ts;
            public byte[] Bytes;
        }

        static Snapshot latest;
        static int clients;

        static int Main(string[] args)
        {
            bool fresh;
            using (var mutex = new Mutex(true, @"Local\winperf-sensors-helper", out fresh))
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
            Log("started");
            try
            {
                if (once)
                {
                    Update(computer);
                    Console.Out.Write(Encoding.UTF8.GetString(latest.Bytes));
                    return 0;
                }
                new Thread(Accept) { IsBackground = true, Name = "pipe" }.Start();
                while (true)
                {
                    if (Volatile.Read(ref clients) > 0) Update(computer);
                    Thread.Sleep(1000);
                }
            }
            finally
            {
                computer.Close();
            }
        }

        static void Update(Computer computer)
        {
            var list = new List<Reading>();
            foreach (var hw in computer.Hardware) Walk(hw, list);
            var ts = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            var line = Wire.Line(ts, list) + "\n";
            Volatile.Write(ref latest, new Snapshot { Ts = ts, Bytes = Encoding.UTF8.GetBytes(line) });
        }

        static void Walk(IHardware hw, List<Reading> list)
        {
            hw.Update();
            // Super I/O chips are sub-hardware of the board: label them with the board name.
            var label = hw.Parent != null ? hw.Parent.Name : hw.Name;
            foreach (var s in hw.Sensors)
            {
                var type = s.SensorType.ToString();
                if (Wire.Wanted(type))
                    list.Add(new Reading(label, s.Identifier.ToString(), s.Name, type, s.Value));
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

        static void Log(string message)
        {
            try
            {
                lock (logLock)
                {
                    var path = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "winperf-sensors.log");
                    var info = new FileInfo(path);
                    if (info.Exists && info.Length > 256 * 1024) info.Delete();
                    File.AppendAllText(path, DateTime.Now.ToString("s") + " " + message + Environment.NewLine);
                }
            }
            catch { }
        }
    }
}
