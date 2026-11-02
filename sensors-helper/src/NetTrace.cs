// Real-time ETW session for per-process network bytes, via raw advapi32
// P/Invoke (no TraceEvent: that would add several MB of managed + native DLLs
// for one provider and two integer fields). x64 layouts only (the helper is
// built for x64). Runs only while winperf is connected; the named session is
// stopped on disconnect and on exit, and a stale one (helper killed) is
// stopped before starting.
using System;
using System.Runtime.InteropServices;
using System.Threading;

namespace WinperfSensors
{
    sealed class NetTrace : IDisposable
    {
        public const string SessionName = "winperf-net";
        static readonly Guid KernelNetwork = new Guid("7dd42a49-5329-4832-8dfd-43d979153a88");
        const ulong KeywordIpv4 = 0x10, KeywordIpv6 = 0x20;

        const uint WNODE_FLAG_TRACED_GUID = 0x00020000;
        const uint EVENT_TRACE_REAL_TIME_MODE = 0x00000100;
        const uint EVENT_TRACE_CONTROL_STOP = 1;
        const uint EVENT_CONTROL_CODE_ENABLE_PROVIDER = 1;
        const uint PROCESS_TRACE_MODE_REAL_TIME = 0x00000100;
        const uint PROCESS_TRACE_MODE_EVENT_RECORD = 0x10000000;
        const uint ERROR_ALREADY_EXISTS = 183;
        const ulong INVALID_PROCESSTRACE_HANDLE = ulong.MaxValue;
        const int PropertiesSize = 120;          // sizeof(EVENT_TRACE_PROPERTIES), x64
        const int PropertiesTotal = PropertiesSize + 2 * 1024 * 2; // + logger and file name buffers
        const int LogfileSize = 448;             // sizeof(EVENT_TRACE_LOGFILEW), x64

        [UnmanagedFunctionPointer(CallingConvention.Winapi)]
        delegate void EventRecordCallback(IntPtr record);

        [DllImport("advapi32.dll", CharSet = CharSet.Unicode, EntryPoint = "StartTraceW")]
        static extern uint StartTrace(out ulong handle, string name, IntPtr properties);
        [DllImport("advapi32.dll", CharSet = CharSet.Unicode, EntryPoint = "ControlTraceW")]
        static extern uint ControlTrace(ulong handle, string name, IntPtr properties, uint control);
        [DllImport("advapi32.dll")]
        static extern uint EnableTraceEx2(ulong handle, ref Guid provider, uint control, byte level,
            ulong matchAny, ulong matchAll, uint timeout, IntPtr parameters);
        [DllImport("advapi32.dll", EntryPoint = "OpenTraceW", SetLastError = true)]
        static extern ulong OpenTrace(IntPtr logfile);
        [DllImport("advapi32.dll")]
        static extern uint ProcessTrace(ulong[] handles, uint count, IntPtr start, IntPtr end);
        [DllImport("advapi32.dll")]
        static extern uint CloseTrace(ulong handle);

        readonly NetAgg agg;
        readonly EventRecordCallback callback; // kept alive while ETW may call it
        ulong session, trace = INVALID_PROCESSTRACE_HANDLE;
        IntPtr logfile, loggerName;
        Thread pump;

        public NetTrace(NetAgg agg)
        {
            this.agg = agg;
            callback = OnEvent;
        }

        public bool Running => pump != null;

        static IntPtr NewProperties()
        {
            var p = Marshal.AllocHGlobal(PropertiesTotal);
            Marshal.Copy(new byte[PropertiesTotal], 0, p, PropertiesTotal);
            Marshal.WriteInt32(p, 0, PropertiesTotal);            // Wnode.BufferSize
            Marshal.Copy(Guid.NewGuid().ToByteArray(), 0, p + 24, 16); // Wnode.Guid
            Marshal.WriteInt32(p, 40, 1);                          // Wnode.ClientContext: QPC
            Marshal.WriteInt32(p, 44, unchecked((int)WNODE_FLAG_TRACED_GUID));
            Marshal.WriteInt32(p, 48, 64);                         // BufferSize (KB)
            Marshal.WriteInt32(p, 52, 4);                          // MinimumBuffers
            Marshal.WriteInt32(p, 56, 32);                         // MaximumBuffers
            Marshal.WriteInt32(p, 64, unchecked((int)EVENT_TRACE_REAL_TIME_MODE));
            Marshal.WriteInt32(p, 68, 1);                          // FlushTimer (s)
            Marshal.WriteInt32(p, 112, 0);                         // LogFileNameOffset
            Marshal.WriteInt32(p, 116, PropertiesSize);            // LoggerNameOffset
            return p;
        }

        /// Stop a session with our name (e.g. left by a killed helper).
        public static void StopStale()
        {
            var p = NewProperties();
            try { ControlTrace(0, SessionName, p, EVENT_TRACE_CONTROL_STOP); }
            finally { Marshal.FreeHGlobal(p); }
        }

        public bool Start()
        {
            if (Running) return true;
            var props = NewProperties();
            try
            {
                var err = StartTrace(out session, SessionName, props);
                if (err == ERROR_ALREADY_EXISTS)
                {
                    StopStale();
                    err = StartTrace(out session, SessionName, props);
                }
                if (err != 0) { Program.Log("etw start: " + err); return false; }
            }
            finally { Marshal.FreeHGlobal(props); }

            var provider = KernelNetwork;
            var enabled = EnableTraceEx2(session, ref provider, EVENT_CONTROL_CODE_ENABLE_PROVIDER, 4 /* informational */,
                KeywordIpv4 | KeywordIpv6, 0, 0, IntPtr.Zero);
            if (enabled != 0) { Program.Log("etw enable: " + enabled); Stop(); return false; }

            logfile = Marshal.AllocHGlobal(LogfileSize);
            Marshal.Copy(new byte[LogfileSize], 0, logfile, LogfileSize);
            loggerName = Marshal.StringToHGlobalUni(SessionName);
            Marshal.WriteIntPtr(logfile, 8, loggerName);                       // LoggerName
            Marshal.WriteInt32(logfile, 28, unchecked((int)(PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD)));
            Marshal.WriteIntPtr(logfile, 424, Marshal.GetFunctionPointerForDelegate(callback)); // EventRecordCallback
            trace = OpenTrace(logfile);
            if (trace == INVALID_PROCESSTRACE_HANDLE)
            {
                Program.Log("etw open: " + Marshal.GetLastWin32Error());
                Stop();
                return false;
            }
            var handle = trace;
            pump = new Thread(() =>
            {
                var r = ProcessTrace(new[] { handle }, 1, IntPtr.Zero, IntPtr.Zero);
                if (r != 0) Program.Log("etw process: " + r);
            }) { IsBackground = true, Name = "etw" };
            pump.Start();
            agg.Drain(); // start from zero
            return true;
        }

        void OnEvent(IntPtr record)
        {
            // EVENT_RECORD (x64): EventHeader.EventDescriptor.Id at 40,
            // UserDataLength at 86, UserData at 96. Payload: PID, size (UInt32 each).
            var id = (ushort)Marshal.ReadInt16(record, 40);
            if (NetAgg.Direction(id) == 0) return;
            var length = (ushort)Marshal.ReadInt16(record, 86);
            var data = Marshal.ReadIntPtr(record, 96);
            if (length < 8 || data == IntPtr.Zero) return;
            agg.Add(id, (uint)Marshal.ReadInt32(data, 0), (uint)Marshal.ReadInt32(data, 4));
        }

        public void Stop()
        {
            var props = NewProperties();
            try { ControlTrace(session, SessionName, props, EVENT_TRACE_CONTROL_STOP); }
            finally { Marshal.FreeHGlobal(props); }
            if (trace != INVALID_PROCESSTRACE_HANDLE) CloseTrace(trace);
            trace = INVALID_PROCESSTRACE_HANDLE;
            pump?.Join(2000);
            pump = null;
            session = 0;
            if (logfile != IntPtr.Zero) { Marshal.FreeHGlobal(logfile); logfile = IntPtr.Zero; }
            if (loggerName != IntPtr.Zero) { Marshal.FreeHGlobal(loggerName); loggerName = IntPtr.Zero; }
        }

        public void Dispose() => Stop();
    }
}
