// fob-sensors wire format: one JSON object per line.
// {"ts":<unix ms>,"sensors":[{"hw":..,"id":..,"name":..,"type":"Temperature"|"Fan"|"Control","value":<number|null>}],
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
        public static readonly string[] Types = { "Temperature", "Fan", "Control" };

        public static bool Wanted(string type) => System.Array.IndexOf(Types, type) >= 0;

        public static string Line(long ts, IEnumerable<Reading> readings) => Line(ts, readings, null, 0);

        /// <param name="net">Per-process bytes in the last <paramref name="netMs"/> ms, or null when not tracing.</param>
        public static string Line(long ts, IEnumerable<Reading> readings, IEnumerable<NetUsage> net, long netMs)
        {
            var sb = new StringBuilder(4096);
            sb.Append("{\"ts\":").Append(ts.ToString(CultureInfo.InvariantCulture)).Append(",\"sensors\":[");
            var first = true;
            foreach (var r in readings)
            {
                if (!Wanted(r.Type)) continue;
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
