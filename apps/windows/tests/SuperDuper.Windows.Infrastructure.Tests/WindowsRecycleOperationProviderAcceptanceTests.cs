using System.Diagnostics;
using System.Runtime.InteropServices;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WindowsRecycleOperationProviderAcceptanceTests
{
    private const uint InvalidFileAttributes = 0xFFFFFFFF;
    private const uint FileAttributeDirectory = 0x00000010;
    private const uint FileAttributeOffline = 0x00001000;
    private const uint FileAttributeRecallOnOpen = 0x00040000;
    private const uint FileAttributeRecallOnDataAccess = 0x00400000;
    private const uint PlaceholderFlags =
        FileAttributeOffline | FileAttributeRecallOnOpen | FileAttributeRecallOnDataAccess;

    [TestMethod]
    [TestCategory("RealRecycleBinProvider")]
    public async Task InspectAsync_RegisteredCloudFixturesRemainMetadataIdenticalWithoutProviderTransfer()
    {
        if (!string.Equals(
            Environment.GetEnvironmentVariable("SUPER_DUPER_RUN_REAL_RECYCLE_PROVIDER_TESTS"),
            "1",
            StringComparison.Ordinal))
        {
            Assert.Inconclusive(
                "Run through Invoke-WindowsRecycleBinAcceptance.ps1 with explicit provider fixtures.");
        }

        var cloudRoot = RequiredPath("SUPER_DUPER_RECYCLE_CLOUD_ROOT");
        var localPath = RequiredPath("SUPER_DUPER_RECYCLE_LOCAL_FILE");
        var offlinePath = RequiredPath("SUPER_DUPER_RECYCLE_OFFLINE_FILE");
        var providerNames = RequiredValue("SUPER_DUPER_RECYCLE_PROVIDER_PROCESSES")
            .Split(';', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries)
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray();
        Assert.IsTrue(providerNames.Length > 0, "At least one provider process name is required.");
        Assert.IsTrue(IsWithin(localPath, cloudRoot), "The locally available fixture is outside the cloud root.");
        Assert.IsTrue(IsWithin(offlinePath, cloudRoot), "The offline fixture is outside the cloud root.");

        var detection = await new WindowsCloudLocationService().DetectAsync();
        Assert.AreEqual("complete", detection.Status, detection.ErrorMessage);
        Assert.IsTrue(
            detection.Locations.Any(location => PathsEqual(location.Path, cloudRoot)),
            $"The supplied cloud root is not a registered Cloud Files sync root: {cloudRoot}");

        var localBefore = Snapshot(localPath);
        var offlineBefore = Snapshot(offlinePath);
        Assert.AreEqual(0u, localBefore.Attributes & PlaceholderFlags,
            "The locally available fixture has offline/recall attributes.");
        Assert.AreNotEqual(0u, offlineBefore.Attributes & PlaceholderFlags,
            "The offline fixture does not have offline/recall attributes.");
        Assert.AreEqual(0UL, offlineBefore.AllocatedBytes,
            "The offline fixture is allocated locally; choose a zero-allocation placeholder.");
        var providerBefore = SnapshotProviders(providerNames);
        Assert.IsTrue(providerBefore.Count > 0, "No named provider process is running.");

        using var executor = new WindowsRecycleOperationExecutor();
        var observations = await executor.InspectAsync([
            CreateItem(1, localPath),
            CreateItem(2, offlinePath),
        ]);

        var localAfter = Snapshot(localPath);
        var offlineAfter = Snapshot(offlinePath);
        var providerAfter = SnapshotProviders(providerNames);
        Assert.AreEqual("eligible", observations[0].Status);
        Assert.AreEqual("recycle_bin_query_succeeded", observations[0].ReasonCode);
        Assert.AreEqual("non_recyclable", observations[1].Status);
        Assert.AreEqual("cloud_placeholder", observations[1].ReasonCode);
        Assert.AreEqual(localBefore, localAfter, "Locally available fixture metadata changed.");
        Assert.AreEqual(offlineBefore, offlineAfter, "Offline fixture metadata or allocation changed.");
        CollectionAssert.AreEquivalent(
            providerBefore.Keys.ToArray(), providerAfter.Keys.ToArray(),
            "The provider process set changed during inspection.");
        foreach (var (key, before) in providerBefore)
        {
            Assert.AreEqual(before, providerAfter[key],
                $"Provider I/O counters changed during inspection for {key}.");
        }

        Console.WriteLine(
            $"PROVIDER_NO_HYDRATION localAttributes=0x{localAfter.Attributes:x8} "
            + $"offlineAttributes=0x{offlineAfter.Attributes:x8} "
            + $"offlineLogicalBytes={offlineAfter.LogicalBytes} "
            + $"offlineAllocatedBytes={offlineAfter.AllocatedBytes} "
            + $"providerProcesses={providerAfter.Count}");
    }

    private static string RequiredPath(string name)
    {
        var value = RequiredValue(name);
        Assert.IsTrue(Path.IsPathFullyQualified(value), $"{name} must be an absolute path.");
        return Path.GetFullPath(value).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
    }

    private static string RequiredValue(string name)
    {
        var value = Environment.GetEnvironmentVariable(name);
        Assert.IsFalse(string.IsNullOrWhiteSpace(value), $"{name} is required.");
        return value!;
    }

    private static bool PathsEqual(string left, string right) =>
        string.Equals(
            Path.GetFullPath(left).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar),
            Path.GetFullPath(right).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar),
            StringComparison.OrdinalIgnoreCase);

    private static bool IsWithin(string path, string root)
    {
        var relative = Path.GetRelativePath(root, path);
        return relative != ".."
            && !relative.StartsWith($"..{Path.DirectorySeparatorChar}", StringComparison.Ordinal)
            && !Path.IsPathFullyQualified(relative);
    }

    private static FileSnapshot Snapshot(string path)
    {
        var attributes = GetFileAttributesW(path);
        Assert.AreNotEqual(InvalidFileAttributes, attributes,
            new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error()).Message);
        Assert.AreEqual(0u, attributes & FileAttributeDirectory, "Provider fixture must be a file.");
        Assert.IsTrue(GetFileAttributesExW(path, 0, out var data),
            new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error()).Message);
        var allocatedLow = GetCompressedFileSizeW(path, out var allocatedHigh);
        var allocationError = Marshal.GetLastWin32Error();
        Assert.IsFalse(allocatedLow == uint.MaxValue && allocationError != 0,
            new System.ComponentModel.Win32Exception(allocationError).Message);
        return new FileSnapshot(
            attributes,
            ((ulong)data.FileSizeHigh << 32) | data.FileSizeLow,
            ((ulong)allocatedHigh << 32) | allocatedLow,
            ((long)data.LastWriteTimeHigh << 32) | data.LastWriteTimeLow);
    }

    private static Dictionary<string, ProviderIoSnapshot> SnapshotProviders(IEnumerable<string> names)
    {
        var result = new Dictionary<string, ProviderIoSnapshot>(StringComparer.OrdinalIgnoreCase);
        foreach (var name in names)
        {
            foreach (var process in Process.GetProcessesByName(name))
            {
                using (process)
                {
                    Assert.IsTrue(GetProcessIoCounters(process.Handle, out var counters),
                        new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error()).Message);
                    result[$"{process.ProcessName}:{process.Id}"] = new ProviderIoSnapshot(
                        counters.ReadTransferCount,
                        counters.WriteTransferCount,
                        counters.OtherTransferCount);
                }
            }
        }
        return result;
    }

    private static WorkerRecycleOperationItem CreateItem(long id, string path) =>
        new(id, 1, 1, id - 1, id, null, "file", path, 1, null, null, id, null,
            "16", "eligible", "recycle_bin_query_succeeded", "pending", null, null, null, null);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern uint GetFileAttributesW(string fileName);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetFileAttributesExW(
        string fileName, int informationLevel, out Win32FileAttributeData data);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern uint GetCompressedFileSizeW(string fileName, out uint fileSizeHigh);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetProcessIoCounters(nint process, out IoCounters counters);

    private sealed record FileSnapshot(
        uint Attributes,
        ulong LogicalBytes,
        ulong AllocatedBytes,
        long LastWriteFileTime);

    private sealed record ProviderIoSnapshot(
        ulong ReadTransferCount,
        ulong WriteTransferCount,
        ulong OtherTransferCount);

    [StructLayout(LayoutKind.Sequential)]
    private struct Win32FileAttributeData
    {
        public uint FileAttributes;
        public uint CreationTimeLow;
        public uint CreationTimeHigh;
        public uint LastAccessTimeLow;
        public uint LastAccessTimeHigh;
        public uint LastWriteTimeLow;
        public uint LastWriteTimeHigh;
        public uint FileSizeHigh;
        public uint FileSizeLow;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct IoCounters
    {
        public ulong ReadOperationCount;
        public ulong WriteOperationCount;
        public ulong OtherOperationCount;
        public ulong ReadTransferCount;
        public ulong WriteTransferCount;
        public ulong OtherTransferCount;
    }
}
