using System;

namespace WinperfSensors
{
    static class MftRequest
    {
        internal static bool IsVolumeDevice(string target)
        {
            const string prefix = @"\Device\HarddiskVolume";
            if (target == null || !target.StartsWith(prefix, StringComparison.Ordinal) || target.Length == prefix.Length) return false;
            for (var i = prefix.Length; i < target.Length; i++)
                if (target[i] < '0' || target[i] > '9') return false;
            return true;
        }

        internal static char Parse(byte[] bytes)
        {
            if (bytes == null || bytes.Length != 8 || bytes[0] != 'F' || bytes[1] != 'M'
                || bytes[2] != '0' || bytes[3] != '1' || bytes[4] < 'A' || bytes[4] > 'Z'
                || bytes[5] != 0 || bytes[6] != 0 || bytes[7] != 0)
                throw new ArgumentException("request");
            return (char)bytes[4];
        }
    }
}
