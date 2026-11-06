using System;
using System.Diagnostics;
using System.IO;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Security.Principal;
using System.Text;
using System.Threading;
using Microsoft.Win32.SafeHandles;

namespace WinperfSensors
{
    static class MftService
    {
        const int RequestMs = 2000;
        const int TotalMs = 180000;
        static readonly SecurityIdentifier User = WindowsIdentity.GetCurrent().User;
        static readonly int Session = Process.GetCurrentProcess().SessionId;

        internal static void Start()
        {
            // An unelevated copy cannot reserve a convincing server endpoint.
            if (!new WindowsPrincipal(WindowsIdentity.GetCurrent()).IsInRole(WindowsBuiltInRole.Administrator)) return;
            new Thread(Accept) { IsBackground = true, Name = "mft-accept" }.Start();
        }

        static NamedPipeServerStream Create()
        {
            // Medium integrity permits the asInvoker UI to write, but not low-integrity clients.
            // The mask excludes CreateNewInstance, which shares a bit with AppendData.
            var sddl = "O:" + User.Value + "D:P(D;;GA;;;NU)(A;;0x00100183;;;" + User.Value + ")S:(ML;;NW;;;ME)";
            IntPtr descriptor;
            uint size;
            if (!ConvertStringSecurityDescriptorToSecurityDescriptor(sddl, 1, out descriptor, out size))
                throw new IOException("pipe security");
            try
            {
                var attrs = new SecurityAttributes { Length = Marshal.SizeOf(typeof(SecurityAttributes)), Descriptor = descriptor };
                // First instance, overlapped duplex, byte mode, reject remote clients.
                var handle = CreateNamedPipe(@"\\.\pipe\fob-space-v1-" + User.Value,
                    0x00000003 | 0x00080000 | 0x40000000, 0x00000008, 1, 65536, 4096, 0, ref attrs);
                if (handle.IsInvalid) { handle.Dispose(); throw new IOException("pipe unavailable"); }
                return new NamedPipeServerStream(PipeDirection.InOut, true, false, handle);
            }
            finally { LocalFree(descriptor); }
        }

        static void Accept()
        {
            while (true)
            {
                try
                {
                    using (var pipe = Create())
                    {
                        pipe.WaitForConnection();
                        Serve(pipe);
                    }
                }
                catch (Exception) { Thread.Sleep(1000); }
            }
        }

        static bool Authorized(NamedPipeServerStream pipe)
        {
            uint pid;
            if (!GetNamedPipeClientProcessId(pipe.SafePipeHandle, out pid)) return false;
            using (var client = Process.GetProcessById((int)pid))
                if (client.SessionId != Session) return false;
            var matches = false;
            pipe.RunAsClient(() => {
                using (var identity = WindowsIdentity.GetCurrent()) matches = User.Equals(identity.User);
            });
            return matches;
        }

        static void Serve(NamedPipeServerStream pipe)
        {
            Process worker = null;
            // Disposal breaks reads and writes too, including a peer that never reads.
            using (var deadline = new Timer(_ => { try { pipe.Dispose(); } catch { } }, null, RequestMs, Timeout.Infinite))
            {
                try
                {
                    var request = new byte[8];
                    var got = 0;
                    while (got < request.Length)
                    {
                        var n = pipe.Read(request, got, request.Length - got);
                        if (n == 0) return;
                        got += n;
                    }
                    var drive = MftRequest.Parse(request);
                    // Pipe impersonation identifies the last bytes read, not an unread connection.
                    if (!Authorized(pipe)) return;
                    var target = new StringBuilder(1024);
                    if (QueryDosDevice(drive + ":", target, target.Capacity) == 0
                        || !MftRequest.IsVolumeDevice(target.ToString()))
                    { Error(pipe, "unsupported"); return; }
                    var info = new DriveInfo(drive + @":\");
                    if (info.DriveType != DriveType.Fixed || !info.IsReady || info.DriveFormat != "NTFS")
                    { Error(pipe, "unsupported"); return; }
                    var path = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "fob-mft.exe");
                    // Keep the verified executable locked against replacement until it exits.
                    using (var locked = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read))
                    using (var sha = SHA256.Create())
                    {
                        var hash = BitConverter.ToString(sha.ComputeHash(locked)).Replace("-", "").ToLowerInvariant();
                        if (hash != MftWorkerHash.Value) { Error(pipe, "worker_mismatch"); return; }
                        worker = new Process { StartInfo = new ProcessStartInfo {
                            FileName = path, Arguments = drive.ToString(), UseShellExecute = false,
                            CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardInput = true,
                            WorkingDirectory = AppDomain.CurrentDomain.BaseDirectory,
                        } };
                        worker.Start();
                        try { worker.PriorityClass = ProcessPriorityClass.BelowNormal; } catch { }
                        deadline.Change(TotalMs, Timeout.Infinite);
                        var running = worker;
                        // One bounded async read, no accumulating command queue. EOF is cancel.
                        var cancel = new byte[1];
                        pipe.ReadAsync(cancel, 0, 1).ContinueWith(_ => {
                            try { running.Kill(); } catch { }
                            try { pipe.Dispose(); } catch { }
                        });
                        var buffer = new byte[65536];
                        long total = 0;
                        while (true)
                        {
                            var n = worker.StandardOutput.BaseStream.Read(buffer, 0, buffer.Length);
                            if (n == 0) break;
                            total += n;
                            if (total > 768L * 1024 * 1024) break;
                            pipe.Write(buffer, 0, n);
                        }
                    }
                }
                catch (ArgumentException) { try { Error(pipe, "request"); } catch { } }
                catch (Exception) { /* Disconnect and worker failures invalidate the partial stream. */ }
                finally
                {
                    if (worker != null)
                    {
                        try { worker.StandardInput.Close(); } catch { }
                        try { if (!worker.HasExited) worker.Kill(); } catch { }
                        worker.Dispose();
                    }
                }
            }
        }

        static void Error(Stream pipe, string code)
        {
            var bytes = Encoding.ASCII.GetBytes(code);
            pipe.Write(new byte[] { 70, 77, 48, 49, 3, (byte)bytes.Length, 0, 0, 0 }, 0, 9);
            pipe.Write(bytes, 0, bytes.Length);
        }

        [StructLayout(LayoutKind.Sequential)]
        struct SecurityAttributes { internal int Length; internal IntPtr Descriptor; internal int Inherit; }
        [DllImport("advapi32.dll", EntryPoint = "ConvertStringSecurityDescriptorToSecurityDescriptorW", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern bool ConvertStringSecurityDescriptorToSecurityDescriptor(string sddl, uint revision, out IntPtr descriptor, out uint size);
        [DllImport("kernel32.dll")]
        static extern IntPtr LocalFree(IntPtr memory);
        [DllImport("kernel32.dll", EntryPoint = "CreateNamedPipeW", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern SafePipeHandle CreateNamedPipe(string name, uint openMode, uint pipeMode, uint maxInstances,
            uint outBuffer, uint inBuffer, uint timeout, ref SecurityAttributes attributes);
        [DllImport("kernel32.dll", EntryPoint = "QueryDosDeviceW", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern uint QueryDosDevice(string name, StringBuilder target, int max);
        [DllImport("kernel32.dll", SetLastError = true)]
        static extern bool GetNamedPipeClientProcessId(SafePipeHandle pipe, out uint pid);
    }
}
