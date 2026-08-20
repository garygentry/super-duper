using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WindowsRecycleOperationExecutorTests
{
    [TestMethod]
    public void OperationFlags_AreExactAndKeepWindowsUndoOmittedPendingAcceptance()
    {
        const uint expected =
            0x00000004 // FOF_SILENT
            | 0x00000010 // FOF_NOCONFIRMATION
            | 0x00000400 // FOF_NOERRORUI
            | 0x00080000 // FOFX_RECYCLEONDELETE
            | 0x00100000; // FOFX_EARLYFAILURE
        const uint addUndoRecord = 0x20000000;

        Assert.AreEqual(expected, WindowsRecycleOperationExecutor.RecycleOnlyOperationFlags);
        Assert.AreEqual(0u, WindowsRecycleOperationExecutor.RecycleOnlyOperationFlags & addUndoRecord);
    }

    [TestMethod]
    [DataRow(unchecked((int)0x80270000), "cancelled_by_system")]
    [DataRow(unchecked((int)0x80270001), "cancelled_by_system")]
    [DataRow(unchecked((int)0x80270002), "elevation_required")]
    [DataRow(unchecked((int)0x80270021), "access_denied")]
    [DataRow(unchecked((int)0x80270023), "item_disappeared")]
    [DataRow(unchecked((int)0x80270025), "root_disconnected")]
    [DataRow(unchecked((int)0x80270027), "sharing_violation")]
    [DataRow(unchecked((int)0x80270032), "recycle_bin_capacity")]
    [DataRow(unchecked((int)0x80270033), "recycle_bin_capacity")]
    [DataRow(unchecked((int)0x80270037), "recycle_bin_capacity")]
    [DataRow(unchecked((int)0x80270036), "unsupported_recycling")]
    [DataRow(unchecked((int)0x80270038), "recycle_path_too_long")]
    [DataRow(unchecked((int)0x8027003A), "recycle_bin_unavailable")]
    [DataRow(unchecked((int)0x80270042), "provider_unavailable")]
    [DataRow(unchecked((int)0x80270045), "provider_failure")]
    [DataRow(unchecked((int)0x80270046), "provider_paused")]
    [DataRow(unchecked((int)0x80070005), "access_denied")]
    [DataRow(unchecked((int)0x80070020), "sharing_violation")]
    [DataRow(unchecked((int)0x80070002), "item_disappeared")]
    [DataRow(unchecked((int)0x80070003), "item_disappeared")]
    [DataRow(unchecked((int)0x80070015), "root_disconnected")]
    [DataRow(unchecked((int)0x800704C7), "cancelled_by_system")]
    [DataRow(unchecked((int)0x81234567), "unmapped_shell_failure")]
    public void MapShellFailure_UsesStableReasonCodes(int hresult, string expected)
    {
        Assert.AreEqual(expected, WindowsRecycleOperationExecutor.MapShellFailure(hresult));
    }

    [TestMethod]
    public void Constructor_CreatesOneLongLivedStaThread()
    {
        using var executor = new WindowsRecycleOperationExecutor();

        Assert.AreEqual(ApartmentState.STA, executor.DedicatedApartmentState);
        Assert.AreNotEqual(Environment.CurrentManagedThreadId, executor.DedicatedThreadId);
    }

    [TestMethod]
    public async Task InspectAsync_UsesPositiveRootEvidenceWithoutMutatingTheFile()
    {
        var root = CreateTestDirectory();
        var path = Path.Combine(root, "capability.bin");
        var content = new byte[] { 1, 3, 5, 7, 9 };
        await File.WriteAllBytesAsync(path, content);
        var before = File.GetLastWriteTimeUtc(path);
        try
        {
            using var executor = new WindowsRecycleOperationExecutor();
            var observations = await executor.InspectAsync([
                CreateItem(1, path, "file"),
                CreateItem(2, @"\\?\" + path, "file"),
            ]);

            Assert.AreEqual(2, observations.Count);
            Assert.AreEqual("eligible", observations[0].Status);
            Assert.AreEqual("recycle_bin_query_succeeded", observations[0].ReasonCode);
            Assert.AreEqual("eligible", observations[1].Status);
            CollectionAssert.AreEqual(content, await File.ReadAllBytesAsync(path));
            Assert.AreEqual(before, File.GetLastWriteTimeUtc(path));
        }
        finally
        {
            await TestDirectoryCleanup.DeleteAsync(root);
        }
    }

    [TestMethod]
    public async Task InspectAsync_RejectsOfflineAndMissingItemsWithoutReadingContent()
    {
        var root = CreateTestDirectory();
        var path = Path.Combine(root, "offline.bin");
        var content = new byte[] { 2, 4, 6, 8 };
        await File.WriteAllBytesAsync(path, content);
        File.SetAttributes(path, File.GetAttributes(path) | FileAttributes.Offline);
        try
        {
            using var executor = new WindowsRecycleOperationExecutor();
            var observations = await executor.InspectAsync([
                CreateItem(1, path, "file"),
                CreateItem(2, Path.Combine(root, "missing.bin"), "file"),
            ]);

            Assert.AreEqual("non_recyclable", observations[0].Status);
            Assert.AreEqual("cloud_placeholder", observations[0].ReasonCode);
            Assert.AreEqual("path_missing", observations[1].ReasonCode);
            CollectionAssert.AreEqual(content, await File.ReadAllBytesAsync(path));
        }
        finally
        {
            if (File.Exists(path))
            {
                File.SetAttributes(path, FileAttributes.Normal);
            }
            await TestDirectoryCleanup.DeleteAsync(root);
        }
    }

    [TestMethod]
    public async Task ExecuteBatchAsync_RejectsExpiredAdmissionBeforeAcknowledgement()
    {
        var acknowledged = false;
        using var executor = new WindowsRecycleOperationExecutor();
        var batch = new WorkerRecycleOperationBatch(
            1, 1, 0, "signature", "admitted", DateTimeOffset.UtcNow.AddSeconds(-1).ToString("O"),
            null, null, null, [CreateItem(1, @"C:\missing.bin", "file")]);

        await Assert.ThrowsExceptionAsync<InvalidOperationException>(() => executor.ExecuteBatchAsync(
            batch,
            _ =>
            {
                acknowledged = true;
                return Task.CompletedTask;
            }));
        Assert.IsFalse(acknowledged);
    }

    [TestMethod]
    public async Task ExecuteBatchAsync_RejectsMoreThanThirtyTwoEntriesBeforeAcknowledgement()
    {
        var acknowledged = false;
        using var executor = new WindowsRecycleOperationExecutor();
        var items = Enumerable.Range(1, 33)
            .Select(id => CreateItem(id, $@"C:\missing-{id}.bin", "file"))
            .ToArray();
        var batch = new WorkerRecycleOperationBatch(
            1, 1, 0, "signature", "admitted", DateTimeOffset.UtcNow.AddSeconds(30).ToString("O"),
            null, null, null, items);

        await Assert.ThrowsExceptionAsync<ArgumentOutOfRangeException>(() => executor.ExecuteBatchAsync(
            batch,
            _ =>
            {
                acknowledged = true;
                return Task.CompletedTask;
            }));
        Assert.IsFalse(acknowledged);
    }

    [TestMethod]
    public async Task ExecuteBatchAsync_RejectsOfflineItemBeforeAcknowledgementWithoutContentChange()
    {
        var root = CreateTestDirectory();
        var path = Path.Combine(root, "offline-execution.bin");
        var payload = Guid.NewGuid().ToByteArray();
        await File.WriteAllBytesAsync(path, payload);
        File.SetAttributes(path, File.GetAttributes(path) | FileAttributes.Offline);
        var acknowledged = false;
        try
        {
            using var executor = new WindowsRecycleOperationExecutor();
            var exception = await Assert.ThrowsExceptionAsync<InvalidOperationException>(() =>
                executor.ExecuteBatchAsync(
                    CreateBatch(1, CreateItem(1, path, "file")),
                    _ =>
                    {
                        acknowledged = true;
                        return Task.CompletedTask;
                    }));

            StringAssert.Contains(exception.Message, "cloud_placeholder");
            Assert.IsFalse(acknowledged);
            CollectionAssert.AreEqual(payload, await File.ReadAllBytesAsync(path));
        }
        finally
        {
            if (File.Exists(path))
            {
                File.SetAttributes(path, FileAttributes.Normal);
            }
            await TestDirectoryCleanup.DeleteAsync(root);
        }
    }

    [TestMethod]
    public async Task ExecuteBatchAsync_RejectsWrongTypeBeforeAcknowledgement()
    {
        var root = CreateTestDirectory();
        var acknowledged = false;
        try
        {
            using var executor = new WindowsRecycleOperationExecutor();
            var exception = await Assert.ThrowsExceptionAsync<InvalidOperationException>(() =>
                executor.ExecuteBatchAsync(
                    CreateBatch(1, CreateItem(1, root, "file")),
                    _ =>
                    {
                        acknowledged = true;
                        return Task.CompletedTask;
                    }));

            StringAssert.Contains(exception.Message, "wrong_type");
            Assert.IsFalse(acknowledged);
            Assert.IsTrue(Directory.Exists(root));
        }
        finally
        {
            await TestDirectoryCleanup.DeleteAsync(root);
        }
    }

    [TestMethod]
    [TestCategory("RealRecycleBin")]
    public async Task ExecuteBatchAsync_RealRecycleBinPreservesHardLinkAndExactFolderSurvivors()
    {
        if (!string.Equals(
            Environment.GetEnvironmentVariable("SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS"),
            "1",
            StringComparison.Ordinal))
        {
            Assert.Inconclusive("Set SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS=1 for disposable Shell mutation acceptance.");
        }

        var root = CreateTestDirectory();
        var keptAlias = Path.Combine(root, "hard-link-survivor.bin");
        var removedAlias = Path.Combine(root, "hard-link-remove.bin");
        var keptFolder = Path.Combine(root, "exact-folder-survivor");
        var removedFolder = Path.Combine(root, "exact-folder-remove");
        var payload = Guid.NewGuid().ToByteArray();
        await File.WriteAllBytesAsync(keptAlias, payload);
        Assert.IsTrue(CreateHardLinkW(removedAlias, keptAlias, nint.Zero), new System.ComponentModel.Win32Exception().Message);
        Directory.CreateDirectory(Path.Combine(keptFolder, "nested"));
        Directory.CreateDirectory(Path.Combine(removedFolder, "nested"));
        await File.WriteAllBytesAsync(Path.Combine(keptFolder, "nested", "payload.bin"), payload);
        await File.WriteAllBytesAsync(Path.Combine(removedFolder, "nested", "payload.bin"), payload);

        try
        {
            using var executor = new WindowsRecycleOperationExecutor();
            var aliasItem = WithSnapshot(CreateItem(1, removedAlias, "file"));
            var aliasResult = await executor.ExecuteBatchAsync(
                CreateBatch(1, aliasItem),
                _ => Task.CompletedTask);

            Assert.IsTrue(aliasResult.ShellStarted);
            Assert.AreEqual("recycled", aliasResult.Items.Single().Status);
            Assert.IsTrue(aliasResult.Items.Single().RecycledItemPresent);
            Assert.AreEqual(0L, aliasResult.PerformHresult);
            Assert.AreEqual(0L, aliasResult.FinishHresult);
            Assert.IsFalse(aliasResult.AnyOperationsAborted);
            Assert.AreEqual(0L, aliasResult.AbortQueryHresult);
            Assert.IsFalse(File.Exists(removedAlias));
            CollectionAssert.AreEqual(payload, await File.ReadAllBytesAsync(keptAlias));

            var folderItem = CreateItem(2, removedFolder, "folder");
            var folderResult = await executor.ExecuteBatchAsync(
                CreateBatch(2, folderItem),
                _ => Task.CompletedTask);

            Assert.IsTrue(folderResult.ShellStarted);
            Assert.AreEqual("recycled", folderResult.Items.Single().Status);
            Assert.IsFalse(Directory.Exists(removedFolder));
            CollectionAssert.AreEqual(
                payload,
                await File.ReadAllBytesAsync(Path.Combine(keptFolder, "nested", "payload.bin")));
        }
        finally
        {
            if (Directory.Exists(root))
            {
                await TestDirectoryCleanup.DeleteAsync(root);
            }
        }
    }

    [TestMethod]
    [TestCategory("RealRecycleBin")]
    public async Task ExecuteBatchAsync_CancellationAfterDurableStartStopsBeforeCurrentItem()
    {
        if (!string.Equals(
            Environment.GetEnvironmentVariable("SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS"),
            "1",
            StringComparison.Ordinal))
        {
            Assert.Inconclusive("Set SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS=1 for disposable Shell cancellation acceptance.");
        }

        var root = CreateTestDirectory();
        var path = Path.Combine(root, "cancel-before-predelete.bin");
        var payload = Guid.NewGuid().ToByteArray();
        await File.WriteAllBytesAsync(path, payload);
        using var cancellation = new CancellationTokenSource();
        try
        {
            using var executor = new WindowsRecycleOperationExecutor();
            var result = await executor.ExecuteBatchAsync(
                CreateBatch(1, WithSnapshot(CreateItem(1, path, "file"))),
                _ =>
                {
                    cancellation.Cancel();
                    return Task.CompletedTask;
                },
                cancellation.Token);

            Assert.IsTrue(result.ShellStarted);
            Assert.AreEqual("cancelled", result.Items.Single().Status);
            Assert.AreEqual("cancelled_before_item", result.Items.Single().ReasonCode);
            CollectionAssert.AreEqual(payload, await File.ReadAllBytesAsync(path));
        }
        finally
        {
            if (Directory.Exists(root))
            {
                await TestDirectoryCleanup.DeleteAsync(root);
            }
        }
    }

    [TestMethod]
    [TestCategory("RealRecycleBin")]
    public async Task ExecuteBatchAsync_LockedFileReturnsStructuredFailureAndLeavesSource()
    {
        if (!string.Equals(
            Environment.GetEnvironmentVariable("SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS"),
            "1",
            StringComparison.Ordinal))
        {
            Assert.Inconclusive("Set SUPER_DUPER_RUN_REAL_RECYCLE_BIN_TESTS=1 for disposable Shell locked-file acceptance.");
        }

        var root = CreateTestDirectory();
        var path = Path.Combine(root, "locked.bin");
        var payload = Guid.NewGuid().ToByteArray();
        await File.WriteAllBytesAsync(path, payload);
        try
        {
            var item = WithSnapshot(CreateItem(1, path, "file"));
            using var locked = new FileStream(path, FileMode.Open, FileAccess.ReadWrite, FileShare.None);
            using var executor = new WindowsRecycleOperationExecutor();
            var result = await executor.ExecuteBatchAsync(
                CreateBatch(1, item),
                _ => Task.CompletedTask);

            Assert.IsTrue(result.ShellStarted);
            Assert.AreEqual("failed", result.Items.Single().Status);
            Assert.AreEqual(
                "sharing_violation",
                result.Items.Single().ReasonCode,
                $"Shell HRESULT: {result.Items.Single().ShellHresult}");
            Assert.IsTrue(File.Exists(path));
            locked.Position = 0;
            var observed = new byte[payload.Length];
            await locked.ReadExactlyAsync(observed);
            CollectionAssert.AreEqual(payload, observed);
        }
        finally
        {
            if (Directory.Exists(root))
            {
                await TestDirectoryCleanup.DeleteAsync(root);
            }
        }
    }

    private static WorkerRecycleOperationBatch CreateBatch(long id, WorkerRecycleOperationItem item) =>
        new(id, 1, id - 1, $"signature-{id}", "admitted",
            DateTimeOffset.UtcNow.AddSeconds(30).ToString("O"), null, null, null, [item]);

    private static WorkerRecycleOperationItem CreateItem(long id, string path, string kind) =>
        new(id, 1, 1, id - 1, id, null, kind, path, 1, null, null,
            kind == "file" ? id : null, kind == "folder" ? id : null,
            "16", "eligible", "recycle_bin_query_succeeded", "pending", null, null, null, null);

    private static WorkerRecycleOperationItem WithSnapshot(WorkerRecycleOperationItem item)
    {
        using var handle = CreateFileW(item.Path, 0x80, 0x7, nint.Zero, 3, 0x02000000, nint.Zero);
        Assert.IsFalse(handle.IsInvalid, new System.ComponentModel.Win32Exception().Message);
        Assert.IsTrue(GetFileInformationByHandle(handle, out var info), new System.ComponentModel.Win32Exception().Message);
        var fileIndex = ((ulong)info.FileIndexHigh << 32) | info.FileIndexLow;
        var identity = $"{info.VolumeSerialNumber:x8}:{fileIndex:x16}";
        var size = ((ulong)info.FileSizeHigh << 32) | info.FileSizeLow;
        var fileTime = ((long)info.LastWriteTimeHigh << 32) | info.LastWriteTimeLow;
        var modified = checked((fileTime - 116444736000000000L) * 100);
        return item with
        {
            SnapshotFileIdentity = identity,
            SnapshotFileSize = size.ToString(),
            SnapshotLastModified = modified,
        };
    }

    private static string CreateTestDirectory()
    {
        var path = Path.Combine(Path.GetTempPath(), $"super-duper-recycle-{Guid.NewGuid():N}");
        Directory.CreateDirectory(path);
        return path;
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool CreateHardLinkW(string fileName, string existingFileName, nint securityAttributes);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern SafeFileHandle CreateFileW(
        string fileName, uint desiredAccess, uint shareMode, nint securityAttributes,
        uint creationDisposition, uint flagsAndAttributes, nint templateFile);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetFileInformationByHandle(
        SafeFileHandle file, out ByHandleFileInformation information);

    [StructLayout(LayoutKind.Sequential)]
    private struct ByHandleFileInformation
    {
        public uint FileAttributes;
        public uint CreationTimeLow;
        public uint CreationTimeHigh;
        public uint LastAccessTimeLow;
        public uint LastAccessTimeHigh;
        public uint LastWriteTimeLow;
        public uint LastWriteTimeHigh;
        public uint VolumeSerialNumber;
        public uint FileSizeHigh;
        public uint FileSizeLow;
        public uint NumberOfLinks;
        public uint FileIndexHigh;
        public uint FileIndexLow;
    }
}
