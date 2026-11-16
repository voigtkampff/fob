// fob-sensors wire format: one JSON object per line.
// {"ts":<unix ms>,"sensors":[{"hw":..,"id":..,"name":..,"type":"Temperature"|"Fan"|"Control"|"Voltage"|"Power","value":<number|null>}],
//  "net":{"ms":<interval>,"procs":[{"pid":..,"name":..,"rx":<bytes>,"tx":<bytes>}]}}
//  ("net" only while ETW runs; "name" is the helper's best guess, winperf prefers its own by pid)
// Hand-written (no System.Text.Json) so it is tiny, culture-proof and testable on any .NET.
using System.Collections.Generic;
using System.Globalization;
using System.Text;

namespace WinperfSensors
{
    public sealed class Reading
    {
        public string Hw;
        public string Id;
        public string Name;
        public string Type;
        public float? Value;

        public Reading(string hw, string id, string name, string type, float? value)
        {
            Hw = hw; Id = id; Name = name; Type = type; Value = value;
        }
    }

    public static class Wire
    {
        public static readonly string[] Types = { "Temperature", "Fan", "Control", "Voltage", "Power" };

        static bool CpuId(string id)
        {
            return id.StartsWith("/intelcpu/", System.StringComparison.OrdinalIgnoreCase)
                || id.StartsWith("/amdcpu/", System.StringComparison.OrdinalIgnoreCase)
                || id.StartsWith("/cpu/", System.StringComparison.OrdinalIgnoreCase);
        }

        // Rails reported next to the core supply that do not measure it:
        // the termination rail, the memory / IO / PLL supplies and an APU's
        // SoC rail. They never reach the UI (helper 0.8.4).
        static bool NonCoreRail(string name)
        {
            var words = new[] { "termination", "vccio", "vccsa", "vccin", "vddq", "vtt", "pll", "soc" };
            foreach (var word in words)
                if (name.IndexOf(word, System.StringComparison.OrdinalIgnoreCase) >= 0) return true;
            return false;
        }

        // Voltage goes on the wire for the CPU's core supply only: an
        // identifier under /intelcpu/, /amdcpu/ or /cpu/, or a Super I/O
        // channel the board labels for the CPU ("Vcore"). Every other
        // channel of a Super I/O chip is a whole-board rail (AVCC, +3.3V,
        // 5V, 12V, "Voltage #N") and stays here. A rail listed in
        // NonCoreRail is not the core supply either. Power goes on the wire
        // for the CPU package only (LHM reports it in W); a GPU's power comes
        // from NVML in winperf, not from here.
        public static bool Wanted(Reading r)
        {
            if (System.Array.IndexOf(Types, r.Type) < 0) return false;
            var id = r.Id ?? "";
            var name = r.Name ?? "";
            if (r.Type == "Power")
                return CpuId(id) && name.IndexOf("package", System.StringComparison.OrdinalIgnoreCase) >= 0;
            if (r.Type != "Voltage") return true;
            if (NonCoreRail(name)) return false;
            if (CpuId(id)) return true;
            return name.IndexOf("vcore", System.StringComparison.OrdinalIgnoreCase) >= 0
                || name.IndexOf("cpu", System.StringComparison.OrdinalIgnoreCase) >= 0;
        }

        public static string Line(long ts, IEnumerable<Reading> readings) => Line(ts, readings, null, 0);

        /// <param name="net">Per-process bytes in the last <paramref name="netMs"/> ms, or null when not tracing.</param>
        /// <param name="throttle">MSR-derived throttle state, or null when the helper has none.</param>
        public static string Line(long ts, IEnumerable<Reading> readings, IEnumerable<NetUsage> net, long netMs, ThrottleState throttle = null)
        {
            var sb = new StringBuilder(4096);
            sb.Append("{\"ts\":").Append(ts.ToString(CultureInfo.InvariantCulture)).Append(",\"sensors\":[");
            var first = true;
            foreach (var r in readings)
            {
                if (!Wanted(r)) continue;
                if (!first) sb.Append(',');
                first = false;
                sb.Append("{\"hw\":"); Str(sb, r.Hw);
                sb.Append(",\"id\":"); Str(sb, r.Id);
                sb.Append(",\"name\":"); Str(sb, r.Name);
                sb.Append(",\"type\":"); Str(sb, r.Type);
                sb.Append(",\"value\":"); Num(sb, r.Value);
                sb.Append('}');
            }
            sb.Append(']');
            if (throttle != null)
            {
                sb.Append(",\"throttle\":");
                ThrottleJson(sb, throttle);
            }
            if (net != null)
            {
                sb.Append(",\"net\":{\"ms\":").Append(netMs.ToString(CultureInfo.InvariantCulture)).Append(",\"procs\":[");
                var firstNet = true;
                foreach (var n in net)
                {
                    if (!firstNet) sb.Append(',');
                    firstNet = false;
                    sb.Append("{\"pid\":").Append(n.Pid.ToString(CultureInfo.InvariantCulture));
                    if (n.Name != null) { sb.Append(",\"name\":"); Str(sb, n.Name); }
                    sb.Append(",\"rx\":").Append(n.Rx.ToString(CultureInfo.InvariantCulture))
                      .Append(",\"tx\":").Append(n.Tx.ToString(CultureInfo.InvariantCulture)).Append('}');
                }
                sb.Append("]}");
            }
            return sb.Append('}').ToString();
        }

        static void ThrottleJson(StringBuilder sb, ThrottleState t)
        {
            sb.Append("{\"supported\":").Append(t.Supported ? "true" : "false");
            sb.Append(",\"thermal\":").Append(t.Thermal ? "true" : "false");
            sb.Append(",\"thermalLog\":").Append(t.ThermalLog ? "true" : "false");
            sb.Append(",\"prochot\":").Append(t.Prochot ? "true" : "false");
            sb.Append(",\"prochotLog\":").Append(t.ProchotLog ? "true" : "false");
            sb.Append(",\"package\":").Append(t.Package ? "true" : "false");
            sb.Append(",\"distanceToTjMax\":");
            if (t.DistanceToTjMax.HasValue)
                sb.Append(t.DistanceToTjMax.Value.ToString(CultureInfo.InvariantCulture));
            else
                sb.Append("null");
            sb.Append(",\"thermalPercent\":").Append(t.ThermalPercent.ToString("0.#", CultureInfo.InvariantCulture));
            sb.Append(",\"packagePercent\":").Append(t.PackagePercent.ToString("0.#", CultureInfo.InvariantCulture));
            sb.Append('}');
        }

        static void Num(StringBuilder sb, float? v)
        {
            if (v is float f && !float.IsNaN(f) && !float.IsInfinity(f))
                sb.Append(((double)f).ToString("0.##", CultureInfo.InvariantCulture));
            else
                sb.Append("null");
        }

        static void Str(StringBuilder sb, string s)
        {
            sb.Append('"');
            foreach (var c in s ?? "")
            {
                switch (c)
                {
                    case '"': sb.Append("\\\""); break;
                    case '\\': sb.Append("\\\\"); break;
                    case '\n': sb.Append("\\n"); break;
                    case '\r': sb.Append("\\r"); break;
                    case '\t': sb.Append("\\t"); break;
                    default:
                        if (c < 0x20) sb.Append("\\u").Append(((int)c).ToString("x4"));
                        else sb.Append(c);
                        break;
                }
            }
            sb.Append('"');
        }
    }

    public struct NetUsage
    {
        public uint Pid;
        public string Name;
        public ulong Rx;
        public ulong Tx;

        public NetUsage(uint pid, string name, ulong rx, ulong tx) { Pid = pid; Name = name; Rx = rx; Tx = tx; }
    }

    /// Per-process byte counts from Microsoft-Windows-Kernel-Network events.
    /// The owning process is the PID in the event payload: the event header's
    /// process is often System (receives complete in DPCs).
    public sealed class NetAgg
    {
        // TCP v4/v6 and UDP v4/v6 "data sent" / "data received". Event 18
        // (TCP data copied on behalf of the user) would double count receives.
        public static int Direction(int eventId)
        {
            switch (eventId)
            {
                case 10: case 26: case 42: case 58: return 1;  // sent
                case 11: case 27: case 43: case 59: return -1; // received
                default: return 0;
            }
        }

        readonly object gate = new object();
        Dictionary<uint, NetUsage> current = new Dictionary<uint, NetUsage>();

        public void Add(int eventId, uint pid, uint size)
        {
            var dir = Direction(eventId);
            if (dir == 0) return;
            lock (gate)
            {
                current.TryGetValue(pid, out var u);
                u.Pid = pid;
                if (dir > 0) u.Tx += size; else u.Rx += size;
                current[pid] = u;
            }
        }

        /// Totals since the last call, busiest first; resets the counters.
        public List<NetUsage> Drain()
        {
            Dictionary<uint, NetUsage> taken;
            lock (gate)
            {
                taken = current;
                current = new Dictionary<uint, NetUsage>();
            }
            var list = new List<NetUsage>(taken.Values);
            list.Sort((a, b) =>
            {
                var c = (b.Rx + b.Tx).CompareTo(a.Rx + a.Tx);
                return c != 0 ? c : a.Pid.CompareTo(b.Pid);
            });
            return list;
        }
    }
}
