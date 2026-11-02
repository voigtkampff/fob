// winperf-sensors wire format: one JSON object per line.
// {"ts":<unix ms>,"sensors":[{"hw":..,"id":..,"name":..,"type":"Temperature"|"Fan"|"Control","value":<number|null>}]}
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

        public static string Line(long ts, IEnumerable<Reading> readings)
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
            return sb.Append("]}").ToString();
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
}
