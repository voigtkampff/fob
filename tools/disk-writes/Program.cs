// Sums write I/O in a WPR trace (DiskIO + FileIO profiles) by file, Fob
// component, process and minute. Two commands:
//   disk-writes phase <etl> <label> <out.json> <c-disk-number> <user-profile> [typeperf.csv]
//   disk-writes report <out.md> <phase.json>...
// "disk" numbers are DiskIo/Write events (what reached the disk driver, lazy
// writer flushes included, attributed to the file); "file" numbers are
// FileIo/Write calls (what the app asked for, cached or not).
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using Microsoft.Diagnostics.Tracing;
using Microsoft.Diagnostics.Tracing.Parsers.Kernel;

static class P
{
    static int Main(string[] a)
    {
        if (a.Length >= 6 && a[0] == "phase") return Phase(a[1], a[2], a[3], int.Parse(a[4]), a[5], a.Length > 6 ? a[6] : null);
        if (a.Length >= 3 && a[0] == "report") return Report(a[1], a.Skip(2).ToArray());
        Console.Error.WriteLine("usage: phase <etl> <label> <out.json> <c-disk> <profile> | report <out.md> <json>...");
        return 2;
    }

    sealed class Acc
    {
        public long DiskBytes, DiskOps, FileBytes, FileOps, Flushes, Creates, Renames, Deletes;
        public HashSet<string> Writers = new();
    }

    static readonly string[] FobMarks = { @"\com.hcb.fob\", @"\fob\sensors\", @"\fob-sensors" };
    static bool IsFob(string p) { var l = p.ToLowerInvariant(); return FobMarks.Any(l.Contains); }
    static bool IsFobProcess(string n) => n is "fob" or "msedgewebview2" or "fob-sensors";

    static string Norm(string path, string profile)
    {
        var p = path;
        if (profile.Length > 0) p = Regex.Replace(p, Regex.Escape(profile), "%USERPROFILE%", RegexOptions.IgnoreCase);
        p = Regex.Replace(p, @"~RF[0-9a-fA-F]+\.TMP", "~RF*.TMP");
        p = Regex.Replace(p, @"\d{3,}", "#");
        return p;
    }

    static string Component(string p)
    {
        var l = p.ToLowerInvariant();
        var w = l.IndexOf(@"\ebwebview\");
        if (w >= 0)
        {
            var rest = p[(w + 11)..].Split('\\');
            return "WebView2 " + string.Join('\\', rest.Take(Math.Min(2, rest.Length)));
        }
        if (l.Contains(@"\roaming\com.hcb.fob\")) return "Fob config " + Path.GetFileName(p);
        if (l.Contains(@"\fob\sensors\") || l.Contains("fob-sensors")) return "helper " + Path.GetFileName(p);
        return "Fob other " + Path.GetDirectoryName(p);
    }

    // Mean of each typeperf column whose name contains "Write Bytes/sec".
    static Dictionary<string, double> Counters(string? csv)
    {
        var means = new Dictionary<string, double>();
        if (csv is null || !File.Exists(csv)) return means;
        var rows = File.ReadAllLines(csv).Where(l => l.StartsWith('"')).Select(l => l.Trim('"').Split("\",\"")).ToList();
        if (rows.Count < 2) return means;
        for (var c = 1; c < rows[0].Length; c++)
        {
            var vals = rows.Skip(1).Select(r => c < r.Length && double.TryParse(r[c], System.Globalization.CultureInfo.InvariantCulture, out var v) ? v : double.NaN)
                .Where(v => !double.IsNaN(v)).ToList();
            if (vals.Count > 0) means[Regex.Replace(rows[0][c], @"^\\\\[^\\]+", "")] = Math.Round(vals.Average(), 1);
        }
        return means;
    }

    static int Phase(string etl, string label, string outPath, int cDisk, string profile, string? csv)
    {
        var files = new Dictionary<string, Acc>();
        var procs = new Dictionary<string, Acc>();
        var minutes = new SortedDictionary<int, long[]>(); // fob disk, fob file, C: disk, all disk
        long allDisk = 0, allOps = 0, cDiskBytes = 0, cOps = 0, diskFlushes = 0;
        Acc F(string path) { var k = Norm(path.Length == 0 ? "(unnamed)" : path, profile); if (!files.TryGetValue(k, out var x)) files[k] = x = new Acc(); return x; }
        Acc Pr(TraceEvent e) { var k = e.ProcessName is { Length: > 0 } n ? n : $"pid {e.ProcessID}"; if (!procs.TryGetValue(k, out var x)) procs[k] = x = new Acc(); return x; }
        long[] M(TraceEvent e) { var m = (int)(e.TimeStampRelativeMSec / 60000); if (!minutes.TryGetValue(m, out var x)) minutes[m] = x = new long[4]; return x; }
        double duration;
        using (var src = new ETWTraceEventSource(etl))
        {
            var k = src.Kernel;
            k.DiskIOWrite += d =>
            {
                allDisk += d.TransferSize; allOps++;
                var m = M(d); m[3] += d.TransferSize;
                if (d.DiskNumber == cDisk) { cDiskBytes += d.TransferSize; cOps++; m[2] += d.TransferSize; }
                var f = F(d.FileName); f.DiskBytes += d.TransferSize; f.DiskOps++;
                if (d.DiskNumber != cDisk) f.Writers.Add("(other disk)");
                Pr(d).DiskBytes += d.TransferSize;
                if (IsFob(d.FileName)) m[0] += d.TransferSize;
            };
            k.DiskIOFlushBuffers += d => diskFlushes++;
            k.FileIOWrite += w =>
            {
                var f = F(w.FileName); f.FileBytes += w.IoSize; f.FileOps++;
                if (w.ProcessName is { Length: > 0 } n) f.Writers.Add(n);
                var p = Pr(w); p.FileBytes += w.IoSize; p.FileOps++;
                if (IsFob(w.FileName)) M(w)[1] += w.IoSize;
            };
            k.FileIOFlush += x => { F(x.FileName).Flushes++; Pr(x).Flushes++; };
            k.FileIOCreate += x => { if (IsFob(x.FileName)) F(x.FileName).Creates++; };
            k.FileIORename += x => { if (IsFob(x.FileName)) F(x.FileName).Renames++; };
            k.FileIODelete += x => { if (IsFob(x.FileName)) F(x.FileName).Deletes++; };
            src.Process();
            duration = src.SessionDuration.TotalSeconds;
        }
        object Row(string key, Acc v) => new
        {
            path = key, disk_bytes = v.DiskBytes, disk_ops = v.DiskOps, file_bytes = v.FileBytes, file_ops = v.FileOps,
            flushes = v.Flushes, creates = v.Creates, renames = v.Renames, deletes = v.Deletes,
            writers = v.Writers.OrderBy(s => s).ToArray(),
        };
        var fobFiles = files.Where(f => IsFob(f.Key)).ToList();
        var comps = fobFiles.GroupBy(f => Component(f.Key)).Select(g => new
        {
            component = g.Key,
            disk_bytes = g.Sum(f => f.Value.DiskBytes), disk_ops = g.Sum(f => f.Value.DiskOps),
            file_bytes = g.Sum(f => f.Value.FileBytes), file_ops = g.Sum(f => f.Value.FileOps),
            flushes = g.Sum(f => f.Value.Flushes), files = g.Count(),
        }).OrderByDescending(c => c.disk_bytes).ThenByDescending(c => c.file_bytes).ToList();
        var result = new
        {
            label, seconds = Math.Round(duration, 1),
            counters_mean_bps = Counters(csv),
            disk = new { all_bytes = allDisk, all_ops = allOps, c_bytes = cDiskBytes, c_ops = cOps, flushes = diskFlushes },
            fob = new
            {
                disk_bytes = fobFiles.Sum(f => f.Value.DiskBytes), disk_ops = fobFiles.Sum(f => f.Value.DiskOps),
                file_bytes = fobFiles.Sum(f => f.Value.FileBytes), file_ops = fobFiles.Sum(f => f.Value.FileOps),
                flushes = fobFiles.Sum(f => f.Value.Flushes),
            },
            // Everything the Fob processes wrote anywhere (FileIO), to catch paths outside the markers.
            fob_process_writes = files.Where(f => f.Value.Writers.Any(IsFobProcess))
                .OrderByDescending(f => f.Value.FileBytes).Take(40).Select(f => Row(f.Key, f.Value)).ToList(),
            components = comps,
            fob_files = fobFiles.OrderByDescending(f => f.Value.DiskBytes).ThenByDescending(f => f.Value.FileBytes)
                .Take(80).Select(f => Row(f.Key, f.Value)).ToList(),
            top_disk_files = files.OrderByDescending(f => f.Value.DiskBytes).Take(30).Select(f => Row(f.Key, f.Value)).ToList(),
            processes = procs.OrderByDescending(p => p.Value.FileBytes + p.Value.DiskBytes).Take(25).Select(p => new
            {
                name = p.Key, file_bytes = p.Value.FileBytes, file_ops = p.Value.FileOps, flushes = p.Value.Flushes,
                disk_bytes_issued = p.Value.DiskBytes,
            }).ToList(),
            minutes = minutes.Select(m => new { minute = m.Key, fob_disk = m.Value[0], fob_file = m.Value[1], c_disk = m.Value[2], all_disk = m.Value[3] }).ToList(),
        };
        File.WriteAllText(outPath, JsonSerializer.Serialize(result, new JsonSerializerOptions { WriteIndented = true }));
        Console.WriteLine($"{label}: {duration:F0}s, C: disk {cDiskBytes} B, Fob disk {result.fob.disk_bytes} B, Fob file {result.fob.file_bytes} B");
        return 0;
    }

    static string Kb(double b) => (b / 1024).ToString("F1");

    static int Report(string outPath, string[] inputs)
    {
        var sb = new StringBuilder();
        sb.AppendLine("## Write I/O per phase (runner)\n");
        sb.AppendLine("Per hour = measured bytes scaled to 3600 s. disk = DiskIo/Write (reached the disk driver), file = FileIo/Write (requested by the app).\n");
        sb.AppendLine("| phase | s | C: disk KB/h | Fob disk KB/h | Fob disk ops/h | Fob file KB/h | Fob file ops/h | Fob flushes/h | steady Fob disk KB/h | steady Fob file KB/h | counter C: KB/s |");
        sb.AppendLine("|---|---|---|---|---|---|---|---|---|---|---|");
        var docs = inputs.Select(i => (i, JsonDocument.Parse(File.ReadAllText(i)).RootElement)).ToList();
        foreach (var (path, r) in docs)
        {
            double s = r.GetProperty("seconds").GetDouble(), h = 3600 / Math.Max(1, s);
            var d = r.GetProperty("disk"); var f = r.GetProperty("fob");
            var counter = r.GetProperty("counters_mean_bps").EnumerateObject().Where(p => p.Name.Contains("LogicalDisk(C:)"))
                .Select(p => Kb(p.Value.GetDouble())).FirstOrDefault() ?? "-";
            // Steady: minutes 2 and later, so a launch at minute 0 is left out.
            var steady = r.GetProperty("minutes").EnumerateArray().Where(m => m.GetProperty("minute").GetInt32() >= 2).ToList();
            var steadyH = 3600 / Math.Max(1, s - 120);
            var sd = Kb(steady.Sum(m => m.GetProperty("fob_disk").GetInt64()) * steadyH);
            var sf = Kb(steady.Sum(m => m.GetProperty("fob_file").GetInt64()) * steadyH);
            sb.AppendLine($"| {r.GetProperty("label").GetString()} | {s:F0} | {Kb(d.GetProperty("c_bytes").GetInt64() * h)} | {Kb(f.GetProperty("disk_bytes").GetInt64() * h)} | {f.GetProperty("disk_ops").GetInt64() * h:F0} | {Kb(f.GetProperty("file_bytes").GetInt64() * h)} | {f.GetProperty("file_ops").GetInt64() * h:F0} | {f.GetProperty("flushes").GetInt64() * h:F0} | {sd} | {sf} | {counter} |");
        }
        foreach (var (path, r) in docs)
        {
            double h = 3600 / Math.Max(1, r.GetProperty("seconds").GetDouble());
            sb.AppendLine($"\n### {r.GetProperty("label").GetString()}: Fob components\n");
            sb.AppendLine("| component | files | disk KB/h | disk ops/h | file KB/h | file ops/h | flushes/h |");
            sb.AppendLine("|---|---|---|---|---|---|---|");
            foreach (var c in r.GetProperty("components").EnumerateArray())
                sb.AppendLine($"| {c.GetProperty("component").GetString()} | {c.GetProperty("files").GetInt32()} | {Kb(c.GetProperty("disk_bytes").GetInt64() * h)} | {c.GetProperty("disk_ops").GetInt64() * h:F0} | {Kb(c.GetProperty("file_bytes").GetInt64() * h)} | {c.GetProperty("file_ops").GetInt64() * h:F0} | {c.GetProperty("flushes").GetInt64() * h:F0} |");
            sb.AppendLine("\nminute: Fob disk KB / Fob file KB / C: disk KB");
            sb.AppendLine(string.Join(", ", r.GetProperty("minutes").EnumerateArray().Select(m =>
                $"{m.GetProperty("minute").GetInt32()}: {Kb(m.GetProperty("fob_disk").GetInt64())}/{Kb(m.GetProperty("fob_file").GetInt64())}/{Kb(m.GetProperty("c_disk").GetInt64())}")));
        }
        File.WriteAllText(outPath, sb.ToString());
        Console.Write(sb.ToString());
        return 0;
    }
}
