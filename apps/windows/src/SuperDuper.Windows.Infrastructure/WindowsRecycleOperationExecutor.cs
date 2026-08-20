using System.Collections.Concurrent;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Infrastructure;

/// <summary>
/// Windows-only Recycle Bin capability and batch executor. The production application deliberately
/// does not register this type yet; operator acceptance must complete before it replaces the
/// disabled executor.
/// </summary>
public sealed class WindowsRecycleOperationExecutor : IRecycleOperationCapabilityExecutor, IDisposable
{
    internal const int MaximumBatchItems = 32;
    private readonly StaDispatcher _dispatcher = new("SuperDuper Recycle Bin STA");
    private bool _disposed;

    public bool IsEnabled => true;

    internal int DedicatedThreadId => _dispatcher.ThreadId;

    internal ApartmentState DedicatedApartmentState => _dispatcher.ApartmentState;

    public Task<IReadOnlyList<RecycleEligibilityObservation>> InspectAsync(
        IReadOnlyList<WorkerRecycleOperationItem> items,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
        ArgumentNullException.ThrowIfNull(items);
        if (items.Count > 200)
        {
            throw new ArgumentOutOfRangeException(nameof(items), "Capability inspection is limited to 200 items.");
        }

        return _dispatcher.InvokeAsync<IReadOnlyList<RecycleEligibilityObservation>>(
            () => InspectOnSta(items, cancellationToken), cancellationToken);
    }

    public Task<RecycleBatchExecutionResult> ExecuteBatchAsync(
        WorkerRecycleOperationBatch batch,
        Func<CancellationToken, Task> acknowledgeShellStart,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
        ArgumentNullException.ThrowIfNull(batch);
        ArgumentNullException.ThrowIfNull(acknowledgeShellStart);
        if (batch.Items.Count is < 1 or > MaximumBatchItems)
        {
            throw new ArgumentOutOfRangeException(nameof(batch), $"Shell batches must contain 1-{MaximumBatchItems} items.");
        }
        if (!string.Equals(batch.Status, "admitted", StringComparison.Ordinal))
        {
            throw new InvalidOperationException("Only a freshly admitted batch can reach Windows Shell.");
        }
        if (!TryReadUnexpiredAdmission(batch.AdmissionExpiresAt))
        {
            throw new InvalidOperationException("The batch admission lease is absent, invalid, or expired.");
        }

        return _dispatcher.InvokeAsync(
            () => ExecuteOnSta(batch, acknowledgeShellStart, cancellationToken), cancellationToken);
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }
        _disposed = true;
        _dispatcher.Dispose();
    }

    private static IReadOnlyList<RecycleEligibilityObservation> InspectOnSta(
        IReadOnlyList<WorkerRecycleOperationItem> items,
        CancellationToken cancellationToken)
    {
        EnsureSta();
        var rootResults = new Dictionary<string, string?>(StringComparer.OrdinalIgnoreCase);
        var results = new List<RecycleEligibilityObservation>(items.Count);
        foreach (var item in items)
        {
            cancellationToken.ThrowIfCancellationRequested();
            var reason = ClassifyItem(item, rootResults);
            results.Add(reason is null
                ? new RecycleEligibilityObservation(item.Id, "eligible", "recycle_bin_query_succeeded")
                : new RecycleEligibilityObservation(item.Id, "non_recyclable", reason));
        }
        return results;
    }

    private static RecycleBatchExecutionResult ExecuteOnSta(
        WorkerRecycleOperationBatch batch,
        Func<CancellationToken, Task> acknowledgeShellStart,
        CancellationToken cancellationToken)
    {
        EnsureSta();
        cancellationToken.ThrowIfCancellationRequested();

        var eligibility = InspectOnSta(batch.Items, cancellationToken);
        var blocked = eligibility.FirstOrDefault(item => item.Status != "eligible");
        if (blocked is not null)
        {
            throw new InvalidOperationException(
                $"Fresh Shell admission rejected item {blocked.ItemId}: {blocked.ReasonCode}.");
        }

        IFileOperation? operation = null;
        var shellItems = new List<IShellItem>(batch.Items.Count);
        uint adviseCookie = 0;
        var shellStarted = false;
        var sink = new FileOperationProgressSink(batch.Items, cancellationToken);
        try
        {
            operation = CreateFileOperation();
            ThrowIfFailed(operation.SetOperationFlags(NativeMethods.RecycleOnlyOperationFlags));
            ThrowIfFailed(operation.Advise(sink, out adviseCookie));

            foreach (var item in batch.Items)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var shellItem = CreateShellItem(item.Path);
                shellItems.Add(shellItem);
                ThrowIfFailed(operation.DeleteItem(shellItem, null));
            }

            // DeleteItem is declarative. A durable worker acknowledgement is mandatory before the
            // one call that can mutate the filesystem. A lost acknowledgement never calls Shell.
            acknowledgeShellStart(cancellationToken).ConfigureAwait(false).GetAwaiter().GetResult();
            shellStarted = true;

            var performHresult = operation.PerformOperations();
            var abortedHresult = operation.GetAnyOperationsAborted(out var aborted);
            if (abortedHresult < 0)
            {
                aborted = true;
            }
            return sink.BuildResult(performHresult, aborted, abortedHresult, shellStarted);
        }
        catch (OperationCanceledException) when (!shellStarted && cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception) when (shellStarted)
        {
            return sink.BuildExceptionalResult(Marshal.GetHRForException(exception), shellStarted);
        }
        finally
        {
            if (operation is not null && adviseCookie != 0)
            {
                _ = operation.Unadvise(adviseCookie);
            }
            foreach (var item in shellItems)
            {
                ReleaseComObject(item);
            }
            if (operation is not null)
            {
                ReleaseComObject(operation);
            }
        }
    }

    private static string? ClassifyItem(
        WorkerRecycleOperationItem item,
        IDictionary<string, string?> rootResults)
    {
        if (string.IsNullOrWhiteSpace(item.Path) || !Path.IsPathFullyQualified(item.Path))
        {
            return "path_not_absolute";
        }

        var path = Path.GetFullPath(WindowsShellPath.ToParsingPath(item.Path));
        var classification = NativeMethods.ClassifyWithoutOpen(path);
        if (classification != PathClassification.File && classification != PathClassification.Directory)
        {
            return classification switch
            {
                PathClassification.Missing => "path_missing",
                PathClassification.CloudPlaceholder => "cloud_placeholder",
                PathClassification.ReparsePoint => "reparse_point",
                PathClassification.Unavailable => "path_unavailable",
                _ => "unsupported_path_type",
            };
        }
        if ((item.TargetKind == "file" && classification != PathClassification.File)
            || (item.TargetKind == "folder" && classification != PathClassification.Directory))
        {
            return "wrong_type";
        }

        var root = NativeMethods.LocalRoot(path);
        if (root is null)
        {
            return "unsupported_root";
        }
        if (!rootResults.TryGetValue(root, out var rootReason))
        {
            rootReason = NativeMethods.ClassifyRecycleRoot(root);
            rootResults[root] = rootReason;
        }
        return rootReason;
    }

    private static bool TryReadUnexpiredAdmission(string? value) =>
        DateTimeOffset.TryParse(value, out var expiresAt)
        && DateTimeOffset.UtcNow <= expiresAt.ToUniversalTime();

    private static IFileOperation CreateFileOperation()
    {
        var type = Type.GetTypeFromCLSID(NativeMethods.FileOperationClassId, throwOnError: true)
            ?? throw new COMException("Windows did not expose the FileOperation COM class.");
        return (IFileOperation)(Activator.CreateInstance(type)
            ?? throw new COMException("Windows did not create IFileOperation."));
    }

    private static IShellItem CreateShellItem(string path)
    {
        var interfaceId = typeof(IShellItem).GUID;
        ThrowIfFailed(NativeMethods.SHCreateItemFromParsingName(
            Path.GetFullPath(WindowsShellPath.ToParsingPath(path)),
            nint.Zero,
            ref interfaceId,
            out var shellItem));
        return shellItem;
    }

    private static void ThrowIfFailed(int hresult)
    {
        if (hresult < 0)
        {
            Marshal.ThrowExceptionForHR(hresult);
        }
    }

    private static void ReleaseComObject(object value)
    {
        if (Marshal.IsComObject(value))
        {
            _ = Marshal.FinalReleaseComObject(value);
        }
    }

    private static void EnsureSta()
    {
        if (Thread.CurrentThread.GetApartmentState() != ApartmentState.STA)
        {
            throw new InvalidOperationException("IFileOperation must run on the dedicated STA thread.");
        }
    }

    private sealed class StaDispatcher : IDisposable
    {
        private readonly BlockingCollection<Action> _queue = new(64);
        private readonly Thread _thread;
        private readonly TaskCompletionSource _started = new(TaskCreationOptions.RunContinuationsAsynchronously);
        private bool _disposed;

        public StaDispatcher(string name)
        {
            _thread = new Thread(Run) { IsBackground = true, Name = name };
            _thread.SetApartmentState(ApartmentState.STA);
            _thread.Start();
            _started.Task.GetAwaiter().GetResult();
        }

        public int ThreadId { get; private set; }

        public ApartmentState ApartmentState { get; private set; }

        public Task<T> InvokeAsync<T>(Func<T> action, CancellationToken cancellationToken)
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
            cancellationToken.ThrowIfCancellationRequested();
            var completion = new TaskCompletionSource<T>(TaskCreationOptions.RunContinuationsAsynchronously);
            var executionState = 0;
            var registration = cancellationToken.Register(() =>
            {
                if (Interlocked.CompareExchange(ref executionState, -1, 0) == 0)
                {
                    completion.TrySetCanceled(cancellationToken);
                }
            });
            try
            {
                _queue.Add(() =>
                {
                    if (Interlocked.CompareExchange(ref executionState, 1, 0) != 0)
                    {
                        registration.Dispose();
                        return;
                    }
                    // Cancellation may prevent queued work from starting. Once the STA owns the
                    // work, the operation itself must observe cancellation and return callbacks;
                    // cancelling the result task here would discard durable Shell evidence.
                    registration.Dispose();
                    try
                    {
                        completion.TrySetResult(action());
                    }
                    catch (OperationCanceledException exception)
                    {
                        completion.TrySetCanceled(exception.CancellationToken);
                    }
                    catch (Exception exception)
                    {
                        completion.TrySetException(exception);
                    }
                    finally
                    {
                        registration.Dispose();
                    }
                }, cancellationToken);
            }
            catch
            {
                registration.Dispose();
                throw;
            }
            return completion.Task;
        }

        public void Dispose()
        {
            if (_disposed)
            {
                return;
            }
            _disposed = true;
            _queue.CompleteAdding();
            if (Thread.CurrentThread.ManagedThreadId != _thread.ManagedThreadId)
            {
                _thread.Join(TimeSpan.FromSeconds(10));
            }
            _queue.Dispose();
        }

        private void Run()
        {
            ThreadId = Environment.CurrentManagedThreadId;
            ApartmentState = Thread.CurrentThread.GetApartmentState();
            _started.TrySetResult();
            foreach (var action in _queue.GetConsumingEnumerable())
            {
                action();
            }
        }
    }

    [ComVisible(true)]
    [ClassInterface(ClassInterfaceType.None)]
    private sealed class FileOperationProgressSink : IFileOperationProgressSink
    {
        private readonly Dictionary<string, ItemState> _states;
        private readonly CancellationToken _cancellationToken;
        private int? _finishHresult;

        public FileOperationProgressSink(
            IReadOnlyList<WorkerRecycleOperationItem> items,
            CancellationToken cancellationToken)
        {
            _cancellationToken = cancellationToken;
            _states = items.ToDictionary(
                item => NativeMethods.PathKey(item.Path),
                item => new ItemState(item),
                StringComparer.OrdinalIgnoreCase);
        }

        public int StartOperations() => NativeMethods.S_OK;

        public int FinishOperations(int hresult)
        {
            _finishHresult = hresult;
            return NativeMethods.S_OK;
        }

        public int PreDeleteItem(uint flags, IShellItem item)
        {
            var state = Find(item);
            if (state is null)
            {
                return NativeMethods.E_ABORT;
            }
            state.PreDeleteSeen = true;
            if (_cancellationToken.IsCancellationRequested)
            {
                state.PreDeleteCode = "cancelled_before_item";
                return NativeMethods.HResultCancelled;
            }

            var reason = NativeMethods.ValidateExpectedMetadata(state.Item);
            if (reason is not null)
            {
                state.PreDeleteCode = reason;
                return NativeMethods.E_ABORT;
            }
            return NativeMethods.S_OK;
        }

        public int PostDeleteItem(uint flags, IShellItem item, int hresult, IShellItem? newlyCreatedItem)
        {
            var state = Find(item);
            if (state is null)
            {
                return NativeMethods.S_OK;
            }
            state.PostDeleteSeen = true;
            state.DeleteHresult = hresult;
            state.RecycledItemPresent = newlyCreatedItem is not null;
            return NativeMethods.S_OK;
        }

        public RecycleBatchExecutionResult BuildResult(
            int performHresult,
            bool aborted,
            int? abortQueryHresult,
            bool shellStarted)
        {
            var observations = _states.Values
                .OrderBy(state => state.Item.Ordinal)
                .Select(state => state.Observation(
                    aborted,
                    abortQueryHresult is not null && abortQueryHresult.Value >= 0))
                .ToArray();
            return new RecycleBatchExecutionResult(
                observations,
                NativeMethods.NumericHresult(performHresult),
                _finishHresult is null ? null : NativeMethods.NumericHresult(_finishHresult.Value),
                aborted,
                abortQueryHresult is null ? null : NativeMethods.NumericHresult(abortQueryHresult.Value),
                shellStarted);
        }

        public RecycleBatchExecutionResult BuildExceptionalResult(int hresult, bool shellStarted)
        {
            foreach (var state in _states.Values.Where(state => !state.PostDeleteSeen))
            {
                state.ForcedUnknownHresult = hresult;
            }
            return BuildResult(
                hresult,
                aborted: true,
                abortQueryHresult: null,
                shellStarted: shellStarted);
        }

        private ItemState? Find(IShellItem item)
        {
            var path = NativeMethods.FileSystemPath(item);
            return path is not null && _states.TryGetValue(NativeMethods.PathKey(path), out var state)
                ? state
                : null;
        }

        public int PreRenameItem(uint flags, IShellItem item, string newName) => NativeMethods.S_OK;
        public int PostRenameItem(uint flags, IShellItem item, string newName, int hresult, IShellItem? newItem) => NativeMethods.S_OK;
        public int PreMoveItem(uint flags, IShellItem item, IShellItem destinationFolder, string? newName) => NativeMethods.S_OK;
        public int PostMoveItem(uint flags, IShellItem item, IShellItem destinationFolder, string? newName, int hresult, IShellItem? newItem) => NativeMethods.S_OK;
        public int PreCopyItem(uint flags, IShellItem item, IShellItem destinationFolder, string? newName) => NativeMethods.S_OK;
        public int PostCopyItem(uint flags, IShellItem item, IShellItem destinationFolder, string? newName, int hresult, IShellItem? newItem) => NativeMethods.S_OK;
        public int PreNewItem(uint flags, IShellItem destinationFolder, string newName) => NativeMethods.S_OK;
        public int PostNewItem(uint flags, IShellItem destinationFolder, string newName, string? templateName, uint fileAttributes, int hresult, IShellItem? newItem) => NativeMethods.S_OK;
        public int UpdateProgress(uint workTotal, uint workSoFar) => NativeMethods.S_OK;
        public int ResetTimer() => NativeMethods.S_OK;
        public int PauseTimer() => NativeMethods.S_OK;
        public int ResumeTimer() => NativeMethods.S_OK;
    }

    private sealed class ItemState(WorkerRecycleOperationItem item)
    {
        public WorkerRecycleOperationItem Item { get; } = item;
        public bool PreDeleteSeen { get; set; }
        public string? PreDeleteCode { get; set; }
        public bool PostDeleteSeen { get; set; }
        public int DeleteHresult { get; set; }
        public bool RecycledItemPresent { get; set; }
        public int? ForcedUnknownHresult { get; set; }

        public RecycleItemResultObservation Observation(bool aborted, bool abortEvidenceReliable)
        {
            if (PostDeleteSeen && DeleteHresult >= 0 && RecycledItemPresent)
            {
                return new(Item.Id, "recycled", "recycled_item_confirmed",
                    NativeMethods.NumericHresult(DeleteHresult), true);
            }
            if (PostDeleteSeen && DeleteHresult >= 0)
            {
                return new(Item.Id, "unknown", "recycled_item_missing",
                    NativeMethods.NumericHresult(DeleteHresult), false);
            }
            if (PostDeleteSeen)
            {
                var code = PreDeleteCode ?? NativeMethods.MapFailure(DeleteHresult);
                var cancelled = DeleteHresult == NativeMethods.HResultCancelled
                    || string.Equals(code, "cancelled_before_item", StringComparison.Ordinal);
                return new(Item.Id, cancelled ? "cancelled" : "failed", code,
                    NativeMethods.NumericHresult(DeleteHresult), null);
            }
            if (ForcedUnknownHresult is int exceptional)
            {
                return new(Item.Id, "unknown", "shell_exception_after_start",
                    NativeMethods.NumericHresult(exceptional), null);
            }
            if (!PreDeleteSeen && aborted && abortEvidenceReliable)
            {
                return new(Item.Id, "cancelled", "shell_aborted_before_item", null, null);
            }
            return new(Item.Id, "unknown", "missing_post_delete_callback", null, null);
        }
    }

    private static class NativeMethods
    {
        internal const int S_OK = 0;
        internal const int E_ABORT = unchecked((int)0x80004004);
        internal const int HResultCancelled = unchecked((int)0x800704C7);
        internal const uint RecycleOnlyOperationFlags =
            0x00000004 // FOF_SILENT
            | 0x00000010 // FOF_NOCONFIRMATION
            | 0x00000400 // FOF_NOERRORUI
            | 0x00080000 // FOFX_RECYCLEONDELETE
            | 0x00100000; // FOFX_EARLYFAILURE

        internal static readonly Guid FileOperationClassId = new("3ad05575-8857-4850-9277-11b85bdb8e09");
        private const uint InvalidFileAttributes = 0xFFFFFFFF;
        private const uint FileAttributeDirectory = 0x00000010;
        private const uint FileAttributeReparsePoint = 0x00000400;
        private const uint FileAttributeOffline = 0x00001000;
        private const uint FileAttributeRecallOnOpen = 0x00040000;
        private const uint FileAttributeRecallOnDataAccess = 0x00400000;
        private const uint FileReadAttributes = 0x00000080;
        private const uint FileShareRead = 0x00000001;
        private const uint FileShareWrite = 0x00000002;
        private const uint FileShareDelete = 0x00000004;
        private const uint OpenExisting = 3;
        private const uint FileFlagBackupSemantics = 0x02000000;
        private const uint DriveRemovable = 2;
        private const uint DriveFixed = 3;
        private const uint SigDnFileSystemPath = 0x80058000;
        private const long WindowsToUnixFileTimeTicks = 116444736000000000;

        [DllImport("shell32.dll", CharSet = CharSet.Unicode, PreserveSig = true)]
        internal static extern int SHCreateItemFromParsingName(
            string path, nint bindContext, ref Guid interfaceId,
            [MarshalAs(UnmanagedType.Interface)] out IShellItem shellItem);

        [DllImport("shell32.dll", CharSet = CharSet.Unicode, PreserveSig = true)]
        private static extern int SHQueryRecycleBinW(string rootPath, ref ShQueryRecycleBinInfo info);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern uint GetFileAttributesW(string fileName);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
        private static extern uint GetDriveTypeW(string rootPathName);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern SafeFileHandle CreateFileW(
            string fileName, uint desiredAccess, uint shareMode, nint securityAttributes,
            uint creationDisposition, uint flagsAndAttributes, nint templateFile);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GetFileInformationByHandle(
            SafeFileHandle file, out ByHandleFileInformation information);

        internal static PathClassification ClassifyWithoutOpen(string path)
        {
            var attributes = GetFileAttributesW(path);
            if (attributes == InvalidFileAttributes)
            {
                var error = Marshal.GetLastWin32Error();
                return error is 2 or 3 ? PathClassification.Missing : PathClassification.Unavailable;
            }
            if ((attributes & (FileAttributeOffline | FileAttributeRecallOnOpen | FileAttributeRecallOnDataAccess)) != 0)
            {
                return PathClassification.CloudPlaceholder;
            }
            if ((attributes & FileAttributeReparsePoint) != 0)
            {
                return PathClassification.ReparsePoint;
            }
            return (attributes & FileAttributeDirectory) != 0
                ? PathClassification.Directory
                : PathClassification.File;
        }

        internal static string? LocalRoot(string path)
        {
            if (path.StartsWith(@"\\", StringComparison.Ordinal))
            {
                return null;
            }
            var root = Path.GetPathRoot(path);
            return string.IsNullOrWhiteSpace(root) || root.StartsWith(@"\\", StringComparison.Ordinal)
                ? null
                : root;
        }

        internal static string? ClassifyRecycleRoot(string root)
        {
            var driveType = GetDriveTypeW(root);
            if (driveType is not (DriveFixed or DriveRemovable))
            {
                return driveType == 4 ? "remote_root_unsupported" : "local_root_unsupported";
            }
            var info = new ShQueryRecycleBinInfo { Size = (uint)Marshal.SizeOf<ShQueryRecycleBinInfo>() };
            return SHQueryRecycleBinW(root, ref info) >= 0 ? null : "recycle_bin_query_failed";
        }

        internal static string? ValidateExpectedMetadata(WorkerRecycleOperationItem item)
        {
            var path = Path.GetFullPath(WindowsShellPath.ToParsingPath(item.Path));
            var classification = ClassifyWithoutOpen(path);
            if (classification != PathClassification.File && classification != PathClassification.Directory)
            {
                return classification switch
                {
                    PathClassification.Missing => "admission_path_missing",
                    PathClassification.CloudPlaceholder => "admission_cloud_placeholder",
                    PathClassification.ReparsePoint => "admission_reparse_point",
                    _ => "admission_path_unavailable",
                };
            }
            if ((item.TargetKind == "file" && classification != PathClassification.File)
                || (item.TargetKind == "folder" && classification != PathClassification.Directory))
            {
                return "admission_wrong_type";
            }
            if (item.TargetKind == "folder")
            {
                return null;
            }

            using var handle = CreateFileW(path, FileReadAttributes,
                FileShareRead | FileShareWrite | FileShareDelete, nint.Zero,
                OpenExisting, FileFlagBackupSemantics, nint.Zero);
            if (handle.IsInvalid)
            {
                return Marshal.GetLastWin32Error() switch
                {
                    2 or 3 => "item_disappeared",
                    5 => "access_denied",
                    32 => "sharing_violation",
                    _ => "admission_metadata_unavailable",
                };
            }
            if (!GetFileInformationByHandle(handle, out var info))
            {
                return "admission_metadata_unavailable";
            }

            var fileIndex = ((ulong)info.FileIndexHigh << 32) | info.FileIndexLow;
            var identity = $"{info.VolumeSerialNumber:x8}:{fileIndex:x16}";
            if (item.SnapshotFileIdentity is null
                || !string.Equals(identity, item.SnapshotFileIdentity, StringComparison.OrdinalIgnoreCase))
            {
                return "admission_identity_changed";
            }
            var size = ((ulong)info.FileSizeHigh << 32) | info.FileSizeLow;
            if (!ulong.TryParse(item.SnapshotFileSize, out var expectedSize) || size != expectedSize)
            {
                return "admission_size_changed";
            }
            var fileTime = ((long)info.LastWriteTimeHigh << 32) | info.LastWriteTimeLow;
            var modifiedNanoseconds = checked((fileTime - WindowsToUnixFileTimeTicks) * 100);
            return item.SnapshotLastModified == modifiedNanoseconds
                ? null
                : "admission_modified_changed";
        }

        internal static string PathKey(string path) =>
            Path.GetFullPath(WindowsShellPath.ToParsingPath(path))
                .TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);

        internal static string? FileSystemPath(IShellItem item)
        {
            var result = item.GetDisplayName(SigDnFileSystemPath, out var value);
            if (result < 0 || value == nint.Zero)
            {
                return null;
            }
            try
            {
                return Marshal.PtrToStringUni(value);
            }
            finally
            {
                Marshal.FreeCoTaskMem(value);
            }
        }

        internal static long NumericHresult(int value) => unchecked((uint)value);

        internal static string MapFailure(int hresult) => unchecked((uint)hresult) switch
        {
            0x80270000 or 0x80270001 => "cancelled_by_system",
            0x80270002 => "elevation_required",
            0x80270021 => "access_denied",
            0x80270023 => "item_disappeared",
            0x80270025 => "root_disconnected",
            0x80270027 => "sharing_violation",
            0x80270032 or 0x80270033 or 0x80270037 => "recycle_bin_capacity",
            0x80270036 => "unsupported_recycling",
            0x80270038 => "recycle_path_too_long",
            0x8027003A => "recycle_bin_unavailable",
            0x80270042 => "provider_unavailable",
            0x80270045 => "provider_failure",
            0x80270046 => "provider_paused",
            0x80070005 => "access_denied",
            0x80070020 => "sharing_violation",
            0x80070002 or 0x80070003 => "item_disappeared",
            0x80070015 => "root_disconnected",
            0x800704C7 => "cancelled_by_system",
            _ => "unmapped_shell_failure",
        };

        [StructLayout(LayoutKind.Sequential)]
        private struct ShQueryRecycleBinInfo
        {
            public uint Size;
            public long RecycleBinSize;
            public long ItemCount;
        }

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
}

internal enum PathClassification
{
    Missing,
    File,
    Directory,
    CloudPlaceholder,
    ReparsePoint,
    Unavailable,
}

[ComImport]
[Guid("43826D1E-E718-42EE-BC55-A1E261C37BFE")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IShellItem
{
    [PreserveSig] int BindToHandler(nint bindContext, ref Guid handlerId, ref Guid interfaceId, out nint result);
    [PreserveSig] int GetParent([MarshalAs(UnmanagedType.Interface)] out IShellItem parent);
    [PreserveSig] int GetDisplayName(uint displayNameType, out nint name);
    [PreserveSig] int GetAttributes(uint mask, out uint attributes);
    [PreserveSig] int Compare([MarshalAs(UnmanagedType.Interface)] IShellItem other, uint hint, out int order);
}

[ComImport]
[Guid("947AAB5F-0A5C-4C13-B4D6-4BF7836FC9F8")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IFileOperation
{
    [PreserveSig] int Advise([MarshalAs(UnmanagedType.Interface)] IFileOperationProgressSink sink, out uint cookie);
    [PreserveSig] int Unadvise(uint cookie);
    [PreserveSig] int SetOperationFlags(uint operationFlags);
    [PreserveSig] int SetProgressMessage([MarshalAs(UnmanagedType.LPWStr)] string message);
    [PreserveSig] int SetProgressDialog(nint progressDialog);
    [PreserveSig] int SetProperties(nint propertyChangeArray);
    [PreserveSig] int SetOwnerWindow(nint ownerWindow);
    [PreserveSig] int ApplyPropertiesToItem([MarshalAs(UnmanagedType.Interface)] IShellItem item);
    [PreserveSig] int ApplyPropertiesToItems(nint items);
    [PreserveSig] int RenameItem([MarshalAs(UnmanagedType.Interface)] IShellItem item, [MarshalAs(UnmanagedType.LPWStr)] string newName, [MarshalAs(UnmanagedType.Interface)] IFileOperationProgressSink? sink);
    [PreserveSig] int RenameItems(nint items, [MarshalAs(UnmanagedType.LPWStr)] string newName);
    [PreserveSig] int MoveItem([MarshalAs(UnmanagedType.Interface)] IShellItem item, [MarshalAs(UnmanagedType.Interface)] IShellItem destinationFolder, [MarshalAs(UnmanagedType.LPWStr)] string? newName, [MarshalAs(UnmanagedType.Interface)] IFileOperationProgressSink? sink);
    [PreserveSig] int MoveItems(nint items, [MarshalAs(UnmanagedType.Interface)] IShellItem destinationFolder);
    [PreserveSig] int CopyItem([MarshalAs(UnmanagedType.Interface)] IShellItem item, [MarshalAs(UnmanagedType.Interface)] IShellItem destinationFolder, [MarshalAs(UnmanagedType.LPWStr)] string? copyName, [MarshalAs(UnmanagedType.Interface)] IFileOperationProgressSink? sink);
    [PreserveSig] int CopyItems(nint items, [MarshalAs(UnmanagedType.Interface)] IShellItem destinationFolder);
    [PreserveSig] int DeleteItem([MarshalAs(UnmanagedType.Interface)] IShellItem item, [MarshalAs(UnmanagedType.Interface)] IFileOperationProgressSink? sink);
    [PreserveSig] int DeleteItems(nint items);
    [PreserveSig] int NewItem([MarshalAs(UnmanagedType.Interface)] IShellItem destinationFolder, uint fileAttributes, [MarshalAs(UnmanagedType.LPWStr)] string name, [MarshalAs(UnmanagedType.LPWStr)] string? templateName, [MarshalAs(UnmanagedType.Interface)] IFileOperationProgressSink? sink);
    [PreserveSig] int PerformOperations();
    [PreserveSig] int GetAnyOperationsAborted([MarshalAs(UnmanagedType.Bool)] out bool anyOperationsAborted);
}

[ComImport]
[Guid("04B0F1A7-9490-44BC-96E1-4296A31252E2")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IFileOperationProgressSink
{
    [PreserveSig] int StartOperations();
    [PreserveSig] int FinishOperations(int hresult);
    [PreserveSig] int PreRenameItem(uint flags, IShellItem item, [MarshalAs(UnmanagedType.LPWStr)] string newName);
    [PreserveSig] int PostRenameItem(uint flags, IShellItem item, [MarshalAs(UnmanagedType.LPWStr)] string newName, int hresult, IShellItem? newItem);
    [PreserveSig] int PreMoveItem(uint flags, IShellItem item, IShellItem destinationFolder, [MarshalAs(UnmanagedType.LPWStr)] string? newName);
    [PreserveSig] int PostMoveItem(uint flags, IShellItem item, IShellItem destinationFolder, [MarshalAs(UnmanagedType.LPWStr)] string? newName, int hresult, IShellItem? newItem);
    [PreserveSig] int PreCopyItem(uint flags, IShellItem item, IShellItem destinationFolder, [MarshalAs(UnmanagedType.LPWStr)] string? newName);
    [PreserveSig] int PostCopyItem(uint flags, IShellItem item, IShellItem destinationFolder, [MarshalAs(UnmanagedType.LPWStr)] string? newName, int hresult, IShellItem? newItem);
    [PreserveSig] int PreDeleteItem(uint flags, IShellItem item);
    [PreserveSig] int PostDeleteItem(uint flags, IShellItem item, int hresult, IShellItem? newlyCreatedItem);
    [PreserveSig] int PreNewItem(uint flags, IShellItem destinationFolder, [MarshalAs(UnmanagedType.LPWStr)] string newName);
    [PreserveSig] int PostNewItem(uint flags, IShellItem destinationFolder, [MarshalAs(UnmanagedType.LPWStr)] string newName, [MarshalAs(UnmanagedType.LPWStr)] string? templateName, uint fileAttributes, int hresult, IShellItem? newItem);
    [PreserveSig] int UpdateProgress(uint workTotal, uint workSoFar);
    [PreserveSig] int ResetTimer();
    [PreserveSig] int PauseTimer();
    [PreserveSig] int ResumeTimer();
}
