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

    static int Main(string[] args)
    {
        // Prove the output does not depend on the machine's culture (decimal commas).
        Thread.CurrentThread.CurrentCulture = new CultureInfo("de-DE");
        var fixture = Path.Combine(AppContext.BaseDirectory, "../../../../../src-tauri/tests/fixtures/sensors-pipe.jsonl");
        var lines = new List<string>();
        for (var tick = 0; tick < 3; tick++)
            lines.Add(Wire.Line(1793612400000 + tick * 1000L, Sample(tick)));
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
        Check(lines[0].Contains("\"value\":null}]}"), "infinity -> null");
        var onDisk = File.Exists(fixture) ? File.ReadAllText(fixture) : "";
        Check(onDisk == text, "fixture " + Path.GetFullPath(fixture) + " is up to date (run with --write)");
        Console.WriteLine(failures == 0 ? "PASS: helper wire format matches the fixture" : $"FAIL: {failures}");
        return failures == 0 ? 0 : 1;
    }

    static int failures;

    static void Check(bool ok, string what)
    {
        if (!ok)
        {
            failures++;
            Console.WriteLine("FAIL: " + what);
        }
    }
}
