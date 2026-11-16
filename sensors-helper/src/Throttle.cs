// Pure throttle types shared by the helper and its tests. No LHM or Windows
// API here; Program.cs owns the MSR reader.
using System;
using System.Collections.Generic;

namespace WinperfSensors
{
    /// <summary>One second's MSR-derived throttle state.</summary>
    public sealed class ThrottleState
    {
        public bool Supported;
        public bool Thermal;
        public bool ThermalLog;
        public bool Prochot;
        public bool ProchotLog;
        public bool Package;
        public int? DistanceToTjMax;
        public double ThermalPercent;
        public double PackagePercent;
    }

    /// <summary>
    /// Sliding 120-sample (about two minutes) duty cycle for the two real
    /// throttle signals. The helper samples once per second while connected.
    /// </summary>
    public sealed class ThrottleWindow
    {
        public const int Capacity = 120;
        readonly List<bool> thermal = new List<bool>();
        readonly List<bool> package = new List<bool>();

        public void Add(bool isThermal, bool isPackage)
        {
            thermal.Add(isThermal);
            package.Add(isPackage);
            if (thermal.Count > Capacity)
            {
                thermal.RemoveAt(0);
                package.RemoveAt(0);
            }
        }

        public double ThermalPercent => Percent(thermal);
        public double PackagePercent => Percent(package);

        static double Percent(List<bool> values)
        {
            if (values.Count == 0) return 0;
            var on = 0;
            foreach (var value in values) if (value) on++;
            return 100.0 * on / values.Count;
        }
    }

    /// <summary>Decoders for the two Intel MSRs we read.</summary>
    public static class MsrDecode
    {
        // IA32_THERM_STATUS / IA32_PACKAGE_THERM_STATUS:
        // bit 0 thermal status, 1 thermal status log, 2 PROCHOT,
        // 3 PROCHOT log, bits 16-22 distance to TjMax (°C).
        public static bool Thermal(ulong value) => (value & 1) != 0;
        public static bool ThermalLog(ulong value) => (value & 2) != 0;
        public static bool Prochot(ulong value) => (value & 4) != 0;
        public static bool ProchotLog(ulong value) => (value & 8) != 0;
        public static int DistanceToTjMax(ulong value) => (int)((value >> 16) & 0x7F);

        /// <summary>
        /// MSR 0x613 counts accumulated package-throttled time. A change since
        /// the previous sample means package throttling happened in that
        /// interval; it does not say whether power or current caused it.
        /// </summary>
        public static bool PackageTimerAdvanced(uint current, uint? previous) =>
            previous.HasValue && current != previous.Value;
    }
}
