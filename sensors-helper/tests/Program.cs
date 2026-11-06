// dotnet run --project tests            -> compare with the fixture (exit 1 on mismatch)
// dotnet run --project tests -- --write -> regenerate the fixture
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Threading;
using WinperfSensors;

static class WireTests
{
    const string Board = "Example Board";
    const string Cpu = "Example Hybrid CPU";
    const string Gpu = "NVIDIA Example GPU";

    // Sensor identifiers/names as LibreHardwareMonitorLib 0.9.x reports them for
    // an Intel 13th-gen CPU, a Example Chip Super I/O and an NVIDIA GPU.
    static IEnumerable<Reading> Sample(int tick)
    {
        float d = tick; // small drift per line
        yield return new Reading(Cpu, "/intelcpu/0/temperature/0", "CPU Core #1", "Temperature", 47 + d);
        yield return new Reading(Cpu, "/intelcpu/0/temperature/1", "CPU Core #2", "Temperature", 45 + d);
        yield return new Reading(Cpu, "/intelcpu/0/temperature/16", "CPU Package", "Temperature", 52 + d);
        yield return new Reading(Cpu, "/intelcpu/0/temperature/17", "Core Max", "Temperature", 55 + d);
        yield return new Reading(Cpu, "/intelcpu/0/temperature/18", "Core Average", "Temperature", 46.25f + d);
        yield return new Reading(Cpu, "/intelcpu/0/load/0", "CPU Total", "Load", 12.5f); // filtered out
        yield return new Reading(Board, "/lpc/example/0/temperature/0", "CPU", "Temperature", 41);
        yield return new Reading(Board, "/lpc/example/0/temperature/1", "Motherboard", "Temperature", 33.5f);
        yield return new Reading(Board, "/lpc/example/0/temperature/6", "Temperature #6", "Temperature", null);
        yield return new Reading(Board, "/lpc/example/0/fan/0", "Fan #1", "Fan", 1102 + 10 * d);
        yield return new Reading(Board, "/lpc/example/0/fan/1", "Fan #2", "Fan", 1349 - 5 * d);
        yield return new Reading(Board, "/lpc/example/0/fan/2", "Fan #3", "Fan", 0);
        yield return new Reading(Board, "/lpc/example/0/fan/6", "Fan #7", "Fan", float.NaN);
        yield return new Reading(Board, "/lpc/example/0/control/1", "Fan #2", "Control", 42.35294f);
        yield return new Reading(Gpu, "/gpu-nvidia/0/temperature/0", "GPU Core", "Temperature", 48.5f + d);
        yield return new Reading(Gpu, "/gpu-nvidia/0/temperature/2", "GPU Hot Spot", "Temperature", 58.9f + d);
        yield return new Reading(Gpu, "/gpu-nvidia/0/fan/1", "GPU Fan 1", "Fan", 0);
        yield return new Reading(Gpu, "/gpu-nvidia/0/control/1", "GPU Fan 1", "Control", 0);
        yield return new Reading("Weird \"name\"\\ \t", "/x/0/temperature/0", "Ünïcode °", "Temperature", float.PositiveInfinity);
    }

    // Per-process network bytes per second as NetAgg would drain them.
    static List<NetUsage> Net(int tick)
    {
        var agg = new NetAgg();
        var names = new Dictionary<uint, string>
        {
            [10412] = "chrome", [7788] = "steam", [5120] = "Dropbox", [3344] = "svchost",
            [9001] = "OneDrive", [4] = "System", [6200] = "WeChat",
        };
        // chrome: 1.2 MB/s down over TCPv4 + IPv6, a little up
        agg.Add(11, 10412, 800_000u + (uint)tick * 50_000); agg.Add(27, 10412, 400_000);
        agg.Add(10, 10412, 35_000);
        agg.Add(11, 7788, 2_400_000u - (uint)tick * 300_000);  // steam download
        agg.Add(10, 5120, 180_000); agg.Add(11, 5120, 12_000); // Dropbox upload
        agg.Add(43, 3344, 2_048); agg.Add(42, 3344, 1_024);    // svchost UDP (DNS)
        agg.Add(59, 9001, 64_000); agg.Add(58, 9001, 9_000);   // UDP over IPv6
        agg.Add(11, 4, 512);
        agg.Add(18, 6200, 99_999);                             // ignored: "copied on behalf of user"
        agg.Add(12, 6200, 1);                                  // ignored: connect
        var list = agg.Drain();
        Check(agg.Drain().Count == 0, "drain resets");
        for (var i = 0; i < list.Count; i++)
        {
            var u = list[i];
            u.Name = names[u.Pid];
            list[i] = u;
        }
        return list;
    }

    static int Main(string[] args)
    {
        Check(MftRequest.Parse(new byte[] {70,77,48,49,67,0,0,0}) == 'C', "MFT drive root request");
        Check(MftRequest.IsVolumeDevice(@"\Device\HarddiskVolume3"), "local volume device");
        foreach (var target in new[] { @"\??\C:\Users", @"\Device\Mup\server", @"\Device\HarddiskVolume3\folder", @"\Device\HarddiskVolume" })
            Check(!MftRequest.IsVolumeDevice(target), "reject SUBST, remote, and subtree aliases");
        var rejected = 0;
        foreach (var bytes in new[] { new byte[0], new byte[7], new byte[9],
            new byte[] {70,77,48,48,67,0,0,0}, new byte[] {70,77,48,49,99,0,0,0},
            new byte[] {70,77,48,49,67,1,0,0}, new byte[] {70,77,48,49,47,0,0,0} })
        {
            try { MftRequest.Parse(bytes); } catch (ArgumentException) { rejected++; }
        }
        Check(rejected == 7, "MFT rejects malformed, oversized, old-version, path, and unknown requests");
        // Prove the output does not depend on the machine's culture (decimal commas).
        Thread.CurrentThread.CurrentCulture = new CultureInfo("de-DE");
        var fixture = Path.Combine(AppContext.BaseDirectory, "../../../../../src-tauri/tests/fixtures/sensors-pipe.jsonl");
        var lines = new List<string>();
        for (var tick = 0; tick < 3; tick++)
            lines.Add(Wire.Line(1793612400000 + tick * 1000L, Sample(tick), Net(tick), 1000));
        var oldStyle = Wire.Line(1793612400000, Sample(0)); // helper without ETW
        var text = string.Join("\n", lines) + "\n";

        if (Array.IndexOf(args, "--write") >= 0)
        {
            File.WriteAllText(fixture, text);
            Console.WriteLine("wrote " + Path.GetFullPath(fixture));
            return 0;
        }
        Check(lines[0].StartsWith("{\"ts\":1793612400000,\"sensors\":[{\"hw\":\"Example Hybrid CPU\""), "prefix");
        Check(!lines[0].Contains("Load"), "non-wanted types are dropped");
        Check(lines[0].Contains("\"value\":46.25}"), "invariant decimals");
        Check(lines[0].Contains("\"value\":42.35}"), "two decimals at most");
        Check(lines[0].Contains("\"id\":\"/lpc/example/0/fan/6\",\"name\":\"Fan #7\",\"type\":\"Fan\",\"value\":null}"), "NaN -> null");
        Check(lines[0].Contains("\"hw\":\"Weird \\\"name\\\"\\\\ \\t\""), "escaping");
        Check(oldStyle.EndsWith("\"value\":null}]}"), "infinity -> null, no net field without ETW");
        Check(lines[0].Contains(",\"net\":{\"ms\":1000,\"procs\":[{\"pid\":7788,\"name\":\"steam\",\"rx\":2400000,\"tx\":0},{\"pid\":10412,\"name\":\"chrome\",\"rx\":1200000,\"tx\":35000}"),
            "net: busiest first, TCP/UDP v4/v6 summed per pid");
        Check(!lines[0].Contains("\"pid\":6200"), "ignored event ids");
        Check(NetAgg.Direction(10) == 1 && NetAgg.Direction(59) == -1 && NetAgg.Direction(18) == 0, "directions");
        var onDisk = File.Exists(fixture) ? File.ReadAllText(fixture) : "";
        Check(onDisk == text, "fixture " + Path.GetFullPath(fixture) + " is up to date (run with --write)");
        Console.WriteLine(failures == 0 ?  $"PASS: {checks} checks, sensor fixture and MFT request validation" : $"FAIL: {failures}");
        return failures == 0 ? 0 : 1;
    }

    static int failures;
    static int checks;

    static void Check(bool ok, string what)
    {
        checks++;
        if (!ok)
        {
            failures++;
            Console.WriteLine("FAIL: " + what);
        }
    }
}
