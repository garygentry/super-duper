using System.Diagnostics;
using System.Text;
using System.Text.Json;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Infrastructure.Protocol;

namespace SuperDuper.Windows.Infrastructure;

public sealed class WorkerClient : IRestartableWorkerClient, IDisposable
{
    private static readonly TimeSpan DefaultStartupTimeout = TimeSpan.FromSeconds(10);
    private static readonly TimeSpan ShutdownTimeout = TimeSpan.FromSeconds(2);
    private const int MaximumDiagnosticCharacters = 16_384;

    private readonly TimeSpan _startupTimeout;
    private readonly string? _databasePath;
    private readonly string? _hashCachePath;
    private readonly string _diagnosticLogPath;
    private readonly ResponseCorrelator _responses = new();
    private readonly SemaphoreSlim _connectionGate = new(1, 1);
    private readonly SemaphoreSlim _writeGate = new(1, 1);
    private readonly SemaphoreSlim _stopGate = new(1, 1);
    private CancellationTokenSource _lifetime = new();
    private readonly StringBuilder _standardError = new();
    private readonly object _standardErrorLock = new();

    private Process? _process;
    private StreamWriter? _standardInput;
    private Task? _standardOutputPump;
    private Task? _standardErrorPump;
    private Task? _exitMonitor;
    private WorkerHelloResult? _hello;
    private long _nextRequestId;
    private int _disposed;
    private int _stopping;

    public WorkerClient(string executablePath)
        : this(executablePath, DefaultStartupTimeout)
    {
    }

    internal WorkerClient(string executablePath, TimeSpan startupTimeout)
        : this(executablePath, startupTimeout, databasePath: null, diagnosticLogPath: null)
    {
    }

    internal WorkerClient(string executablePath, TimeSpan startupTimeout, string? databasePath)
        : this(executablePath, startupTimeout, databasePath, diagnosticLogPath: null)
    {
    }

    internal WorkerClient(
        string executablePath,
        TimeSpan startupTimeout,
        string? databasePath,
        string? diagnosticLogPath)
        : this(executablePath, startupTimeout, databasePath, diagnosticLogPath, hashCachePath: null)
    {
    }

    internal WorkerClient(
        string executablePath,
        TimeSpan startupTimeout,
        string? databasePath,
        string? diagnosticLogPath,
        string? hashCachePath)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(executablePath);
        ExecutablePath = Path.GetFullPath(executablePath);
        _startupTimeout = startupTimeout;
        _databasePath = databasePath is null ? null : Path.GetFullPath(databasePath);
        _hashCachePath = hashCachePath is null ? null : Path.GetFullPath(hashCachePath);
        _diagnosticLogPath = Path.GetFullPath(diagnosticLogPath ?? DefaultDiagnosticLogPath());
    }

    public string ExecutablePath { get; }

    public string DiagnosticLogPath => _diagnosticLogPath;

    public event EventHandler<WorkerRunProgressEventArgs>? RunProgress;

    public event EventHandler<WorkerRunLifecycleEventArgs>? RunLifecycleChanged;

    public event EventHandler<WorkerUnexpectedExitEventArgs>? UnexpectedExit;

    internal int? OwnedProcessId => _process?.Id;

    public async Task<WorkerHelloResult> ConnectAsync(CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref _disposed) != 0, this);

        await _connectionGate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            return await ConnectCoreAsync(cancellationToken).ConfigureAwait(false);
        }
        finally
        {
            _connectionGate.Release();
        }
    }

    public async Task<WorkerHelloResult> RestartAsync(CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref _disposed) != 0, this);
        await _connectionGate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            _hello = null;
            await StopProcessAsync().ConfigureAwait(false);
            ResetConnectionLifetime();
            return await ConnectCoreAsync(cancellationToken).ConfigureAwait(false);
        }
        finally
        {
            _connectionGate.Release();
        }
    }

    private async Task<WorkerHelloResult> ConnectCoreAsync(CancellationToken cancellationToken)
    {
        if (_hello is not null)
        {
            return _hello;
        }
        if (_process is not null)
        {
            throw CreateConnectionException("The previous worker connection ended. Restart is required.");
        }

        StartWorker();

        using var timeoutSource = new CancellationTokenSource(_startupTimeout);
        using var startupSource = CancellationTokenSource.CreateLinkedTokenSource(
            cancellationToken,
            timeoutSource.Token);

        try
        {
            var response = await SendRequestAsync(
                "hello",
                new
                {
                    protocolVersions = new[] { 1 },
                    client = new
                    {
                        name = "SuperDuper.Windows",
                        version = typeof(WorkerClient).Assembly.GetName().Version?.ToString(3) ?? "0.1.0",
                    },
                },
                startupSource.Token).ConfigureAwait(false);

            var hello = response.Result?.Deserialize<WorkerHelloResult>(JsonLineProtocol.SerializerOptions)
                ?? throw new WorkerProtocolException("hello response has no readable result.");

            if (hello.ProtocolVersion != 1)
            {
                throw new WorkerProtocolException(
                    $"Worker selected unoffered protocol version {hello.ProtocolVersion}.");
            }

            _hello = hello;
            return hello;
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            await StopProcessAsync().ConfigureAwait(false);
            throw;
        }
        catch (OperationCanceledException exception) when (timeoutSource.IsCancellationRequested)
        {
            await StopProcessAsync().ConfigureAwait(false);
            throw CreateConnectionException("The hello handshake timed out.", exception);
        }
        catch (Exception exception) when (exception is not WorkerConnectionException)
        {
            await StopProcessAsync().ConfigureAwait(false);
            throw CreateConnectionException("The hello handshake failed.", exception);
        }
    }

    public Task<WorkerSessionPage> ListSessionsAsync(
        long offset = 0,
        int limit = 100,
        CancellationToken cancellationToken = default) =>
        InvokeAsync<WorkerSessionPage>("session.list", new { offset, limit }, cancellationToken);

    public async Task<WorkerSessionDefinition> GetSessionAsync(
        long sessionId,
        CancellationToken cancellationToken = default) =>
        (await InvokeAsync<SessionResult>(
            "session.get",
            new { sessionId },
            cancellationToken).ConfigureAwait(false)).Session;

    public async Task<WorkerSessionDefinition> CreateSessionAsync(
        string name,
        IReadOnlyList<string> roots,
        IReadOnlyList<string> ignorePatterns,
        string cloudPolicy,
        IReadOnlyList<string> manualLocationExclusions,
        IReadOnlyList<WorkerRegisteredCloudLocation> registeredCloudLocations,
        string cloudDetectionStatus,
        CancellationToken cancellationToken = default) =>
        (await InvokeAsync<SessionResult>(
            "session.create",
            new
            {
                name,
                roots,
                ignorePatterns,
                cloudPolicy,
                manualLocationExclusions,
                registeredCloudLocations,
                cloudDetectionStatus,
            },
            cancellationToken).ConfigureAwait(false)).Session;

    public Task<WorkerSessionDefinition> CreateSessionAsync(
        string name,
        IReadOnlyList<string> roots,
        IReadOnlyList<string> ignorePatterns,
        CancellationToken cancellationToken = default) =>
        CreateSessionAsync(
            name,
            roots,
            ignorePatterns,
            CloudPolicyNames.ExcludeRegisteredRoots,
            [],
            [],
            CloudDetectionStatusNames.Complete,
            cancellationToken);

    public async Task<WorkerSessionDefinition> UpdateSessionAsync(
        long sessionId,
        string name,
        IReadOnlyList<string> roots,
        IReadOnlyList<string> ignorePatterns,
        string cloudPolicy,
        IReadOnlyList<string> manualLocationExclusions,
        IReadOnlyList<WorkerRegisteredCloudLocation> registeredCloudLocations,
        string cloudDetectionStatus,
        CancellationToken cancellationToken = default) =>
        (await InvokeAsync<SessionResult>(
            "session.update",
            new
            {
                sessionId,
                name,
                roots,
                ignorePatterns,
                cloudPolicy,
                manualLocationExclusions,
                registeredCloudLocations,
                cloudDetectionStatus,
            },
            cancellationToken).ConfigureAwait(false)).Session;

    public Task<WorkerSessionDefinition> UpdateSessionAsync(
        long sessionId,
        string name,
        IReadOnlyList<string> roots,
        IReadOnlyList<string> ignorePatterns,
        CancellationToken cancellationToken = default) =>
        UpdateSessionAsync(
            sessionId,
            name,
            roots,
            ignorePatterns,
            CloudPolicyNames.ExcludeRegisteredRoots,
            [],
            [],
            CloudDetectionStatusNames.Complete,
            cancellationToken);

    public async Task DeleteSessionAsync(
        long sessionId,
        CancellationToken cancellationToken = default)
    {
        _ = await InvokeAsync<DeleteSessionResult>(
            "session.delete",
            new { sessionId },
            cancellationToken).ConfigureAwait(false);
    }

    public Task<WorkerRunPage> ListRunsAsync(
        long? sessionId = null,
        long offset = 0,
        int limit = 100,
        CancellationToken cancellationToken = default) =>
        InvokeAsync<WorkerRunPage>(
            "run.list",
            new { sessionId, offset, limit },
            cancellationToken);

    public async Task<WorkerRun> GetRunAsync(
        long runId,
        CancellationToken cancellationToken = default) =>
        (await InvokeAsync<RunResult>(
            "run.get",
            new { runId },
            cancellationToken).ConfigureAwait(false)).Run;

    public Task<WorkerRunExclusionPage> GetRunExclusionsAsync(
        long runId,
        long offset = 0,
        int limit = 100,
        CancellationToken cancellationToken = default) =>
        InvokeAsync<WorkerRunExclusionPage>(
            "run_exclusion.page",
            new { runId, offset, limit },
            cancellationToken);

    public async Task<WorkerRun> StartRunAsync(
        long sessionId,
        CancellationToken cancellationToken = default) =>
        (await InvokeAsync<RunResult>(
            "run.start",
            new { sessionId },
            cancellationToken).ConfigureAwait(false)).Run;

    public async Task<WorkerRun> CancelRunAsync(
        long runId,
        CancellationToken cancellationToken = default) =>
        (await InvokeAsync<RunResult>(
            "run.cancel",
            new { runId },
            cancellationToken).ConfigureAwait(false)).Run;

    public Task<WorkerDuplicateFileGroupPage> GetDuplicateFileGroupsAsync(
        DuplicateFileGroupQuery query,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(query);
        return InvokeAsync<WorkerDuplicateFileGroupPage>(
            "duplicate_file_group.page",
            new
            {
                runId = query.RunId,
                pageSize = query.PageSize,
                sort = new
                {
                    field = GroupSortField(query.SortField),
                    direction = SortDirection(query.SortDirection),
                },
                filter = new
                {
                    search = query.Filter.Search,
                    pathMatch = PathMatch(query.Filter.PathMatch),
                    extension = query.Filter.Extension,
                    extensionMatch = ExtensionMatch(query.Filter.ExtensionMatch),
                    minimumSize = query.Filter.MinimumSize,
                    minimumCopyCount = query.Filter.MinimumCopyCount,
                    acrossDrives = query.Filter.AcrossDrives,
                    selectedRoot = query.Filter.SelectedRoot,
                    selectedDrive = query.Filter.SelectedDrive,
                },
                cursor = query.Cursor,
            },
            cancellationToken);
    }

    public Task<WorkerDuplicateFileSelectedRootFacetPage> GetDuplicateFileSelectedRootFacetsAsync(
        DuplicateFileSelectedRootFacetQuery query,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(query);
        return InvokeAsync<WorkerDuplicateFileSelectedRootFacetPage>(
            "duplicate_file_selected_root_facet.page",
            new
            {
                runId = query.RunId,
                pageSize = query.PageSize,
                sort = new
                {
                    field = SelectedRootFacetSortField(query.SortField),
                    direction = SortDirection(query.SortDirection),
                },
                filter = new
                {
                    search = query.Filter.Search,
                    pathMatch = PathMatch(query.Filter.PathMatch),
                    extension = query.Filter.Extension,
                    extensionMatch = ExtensionMatch(query.Filter.ExtensionMatch),
                    minimumSize = query.Filter.MinimumSize,
                    minimumCopyCount = query.Filter.MinimumCopyCount,
                    acrossDrives = query.Filter.AcrossDrives,
                    selectedDrive = query.Filter.SelectedDrive,
                },
                cursor = query.Cursor,
            },
            cancellationToken);
    }

    public Task<WorkerDuplicateFileDriveFacetPage> GetDuplicateFileDriveFacetsAsync(
        DuplicateFileDriveFacetQuery query,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(query);
        return InvokeAsync<WorkerDuplicateFileDriveFacetPage>(
            "duplicate_file_drive_facet.page",
            new
            {
                runId = query.RunId,
                pageSize = query.PageSize,
                sort = new
                {
                    field = DriveFacetSortField(query.SortField),
                    direction = SortDirection(query.SortDirection),
                },
                filter = new
                {
                    search = query.Filter.Search,
                    pathMatch = PathMatch(query.Filter.PathMatch),
                    extension = query.Filter.Extension,
                    extensionMatch = ExtensionMatch(query.Filter.ExtensionMatch),
                    minimumSize = query.Filter.MinimumSize,
                    minimumCopyCount = query.Filter.MinimumCopyCount,
                    acrossDrives = query.Filter.AcrossDrives,
                    selectedRoot = query.Filter.SelectedRoot,
                },
                cursor = query.Cursor,
            },
            cancellationToken);
    }

    public Task<WorkerDuplicateFileMemberPage> GetDuplicateFileGroupMembersAsync(
        DuplicateFileMemberQuery query,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(query);
        return InvokeAsync<WorkerDuplicateFileMemberPage>(
            "duplicate_file_group.members",
            new
            {
                runId = query.RunId,
                groupId = query.GroupId,
                pageSize = query.PageSize,
                sort = new
                {
                    field = MemberSortField(query.SortField),
                    direction = SortDirection(query.SortDirection),
                },
                filter = new { search = query.Filter.Search },
                cursor = query.Cursor,
            },
            cancellationToken);
    }

    public Task<WorkerReviewPlanView> GetReviewPlanAsync(
        long runId,
        CancellationToken cancellationToken = default) =>
        InvokeAsync<WorkerReviewPlanView>(
            "review_plan.get",
            new { runId },
            cancellationToken);

    public Task<WorkerReviewGroupPage> GetReviewGroupsAsync(
        long runId,
        int pageSize,
        string? cursor = null,
        CancellationToken cancellationToken = default) =>
        InvokeAsync<WorkerReviewGroupPage>(
            "review_group.page",
            new { runId, pageSize, cursor },
            cancellationToken);

    public Task<WorkerReviewDecisionMutation> SetReviewDecisionAsync(
        string operationId,
        long runId,
        long groupId,
        long fileId,
        string decision,
        long expectedRevision,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(operationId);
        ArgumentException.ThrowIfNullOrWhiteSpace(decision);
        return InvokeAsync<WorkerReviewDecisionMutation>(
            "review_decision.set",
            new
            {
                operationId,
                runId,
                groupId,
                fileId,
                decision,
                expectedRevision,
            },
            cancellationToken);
    }

    public Task<WorkerReviewFolderGroupPage> GetReviewFolderGroupsAsync(
        long runId,
        int pageSize,
        string? cursor = null,
        CancellationToken cancellationToken = default) =>
        InvokeAsync<WorkerReviewFolderGroupPage>(
            "review_folder_group.page",
            new { runId, pageSize, cursor },
            cancellationToken);

    public Task<WorkerReviewFolderDecisionMutation> SetReviewFolderDecisionAsync(
        string operationId,
        long runId,
        long folderGroupId,
        long folderMemberId,
        string decision,
        long expectedRevision,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(operationId);
        ArgumentException.ThrowIfNullOrWhiteSpace(decision);
        return InvokeAsync<WorkerReviewFolderDecisionMutation>(
            "review_folder_decision.set",
            new
            {
                operationId,
                runId,
                folderGroupId,
                folderMemberId,
                decision,
                expectedRevision,
            },
            cancellationToken);
    }

    public Task<WorkerDuplicateFolderGroupPage> GetDuplicateFolderGroupsAsync(
        DuplicateFolderGroupQuery query,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(query);
        return InvokeAsync<WorkerDuplicateFolderGroupPage>(
            "duplicate_folder_group.page",
            new
            {
                runId = query.RunId,
                pageSize = query.PageSize,
                sort = new
                {
                    field = FolderGroupSortField(query.SortField),
                    direction = SortDirection(query.SortDirection),
                },
                filter = new { search = query.Filter.Search, minimumSize = query.Filter.MinimumSize },
                cursor = query.Cursor,
            },
            cancellationToken);
    }

    public Task<WorkerDuplicateFolderMemberPage> GetDuplicateFolderGroupMembersAsync(
        DuplicateFolderMemberQuery query,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(query);
        return InvokeAsync<WorkerDuplicateFolderMemberPage>(
            "duplicate_folder_group.members",
            new
            {
                runId = query.RunId,
                groupId = query.GroupId,
                pageSize = query.PageSize,
                sort = new { field = "path", direction = SortDirection(query.SortDirection) },
                filter = new { search = query.Filter.Search },
                cursor = query.Cursor,
            },
            cancellationToken);
    }

    public async ValueTask DisposeAsync()
    {
        if (Interlocked.Exchange(ref _disposed, 1) != 0)
        {
            return;
        }

        await StopProcessAsync().ConfigureAwait(false);
        _lifetime.Dispose();
        _connectionGate.Dispose();
        _writeGate.Dispose();
        _stopGate.Dispose();
    }

    public void Dispose() => DisposeAsync().AsTask().GetAwaiter().GetResult();

    private void StartWorker()
    {
        if (_process is not null)
        {
            throw new InvalidOperationException("This worker client has already started a process.");
        }

        var startInfo = new ProcessStartInfo
        {
            FileName = ExecutablePath,
            WorkingDirectory = Path.GetDirectoryName(ExecutablePath) ?? Environment.CurrentDirectory,
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            StandardInputEncoding = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false, throwOnInvalidBytes: true),
            StandardOutputEncoding = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false, throwOnInvalidBytes: true),
            StandardErrorEncoding = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false, throwOnInvalidBytes: true),
        };
        if (_databasePath is not null)
        {
            startInfo.Environment["SUPER_DUPER_DB_PATH"] = _databasePath;
        }
        if (_hashCachePath is not null)
        {
            startInfo.Environment["HASH_CACHE_PATH"] = _hashCachePath;
        }

        try
        {
            _process = Process.Start(startInfo)
                ?? throw new InvalidOperationException("The operating system did not start the worker process.");
        }
        catch (Exception exception)
        {
            throw CreateConnectionException("The worker process could not be started.", exception);
        }

        _standardInput = _process.StandardInput;
        _standardInput.NewLine = "\n";
        _standardOutputPump = PumpStandardOutputAsync(_process.StandardOutput, _lifetime.Token);
        _standardErrorPump = PumpStandardErrorAsync(_process.StandardError, _lifetime.Token);
        _exitMonitor = MonitorExitAsync(_process, _lifetime.Token);
    }

    private async Task<ResponseEnvelope> SendRequestAsync(
        string method,
        object parameters,
        CancellationToken cancellationToken)
    {
        var id = Interlocked.Increment(ref _nextRequestId).ToString(System.Globalization.CultureInfo.InvariantCulture);
        var frame = JsonLineProtocol.EncodeRequestFrame(id, method, parameters);
        var responseTask = _responses.Register(id);
        using var cancellationRegistration = cancellationToken.Register(
            () => _responses.TryCancel(id, cancellationToken));

        try
        {
            await _writeGate.WaitAsync(cancellationToken).ConfigureAwait(false);
            try
            {
                var input = _standardInput
                    ?? throw new WorkerProtocolException("Worker stdin is unavailable.");
                await input.WriteAsync(frame.AsMemory(), cancellationToken).ConfigureAwait(false);
                await input.FlushAsync(cancellationToken).ConfigureAwait(false);
            }
            finally
            {
                _writeGate.Release();
            }
        }
        catch (Exception exception)
        {
            _responses.TryFail(id, exception);
            throw;
        }

        var response = await responseTask.ConfigureAwait(false);
        if (!response.Ok)
        {
            var error = response.Error;
            throw error is null
                ? new WorkerProtocolException("Worker request failed without a structured error.")
                : new WorkerProtocolException(
                    error.Code,
                    error.Message,
                    error.Retryable,
                    error.Details.Clone());
        }

        return response;
    }

    private async Task<TResult> InvokeAsync<TResult>(
        string method,
        object parameters,
        CancellationToken cancellationToken)
        where TResult : class
    {
        _ = await ConnectAsync(cancellationToken).ConfigureAwait(false);
        var response = await SendRequestAsync(method, parameters, cancellationToken).ConfigureAwait(false);
        if (response.Result is not JsonElement result)
        {
            throw new WorkerProtocolException($"{method} response has no result.");
        }

        return result.Deserialize<TResult>(JsonLineProtocol.SerializerOptions)
            ?? throw new WorkerProtocolException($"{method} response has no readable result.");
    }

    private static string GroupSortField(DuplicateFileGroupSortField field) => field switch
    {
        DuplicateFileGroupSortField.RecoverableBytes => "recoverableBytes",
        DuplicateFileGroupSortField.GroupSize => "groupSize",
        DuplicateFileGroupSortField.CopyCount => "copyCount",
        DuplicateFileGroupSortField.RepresentativeName => "representativeName",
        _ => throw new ArgumentOutOfRangeException(nameof(field)),
    };

    private static string PathMatch(DuplicateFilePathMatchMode pathMatch) => pathMatch switch
    {
        DuplicateFilePathMatchMode.Substring => "substring",
        DuplicateFilePathMatchMode.Exact => "exact",
        _ => throw new ArgumentOutOfRangeException(nameof(pathMatch)),
    };

    private static string ExtensionMatch(DuplicateFileExtensionMatchMode extensionMatch) => extensionMatch switch
    {
        DuplicateFileExtensionMatchMode.AnyMember => "any",
        DuplicateFileExtensionMatchMode.AllMembers => "all",
        _ => throw new ArgumentOutOfRangeException(nameof(extensionMatch)),
    };

    private static string MemberSortField(DuplicateFileMemberSortField field) => field switch
    {
        DuplicateFileMemberSortField.Path => "path",
        DuplicateFileMemberSortField.ModifiedTime => "modifiedTime",
        DuplicateFileMemberSortField.Size => "size",
        _ => throw new ArgumentOutOfRangeException(nameof(field)),
    };

    private static string SelectedRootFacetSortField(
        DuplicateFileSelectedRootFacetSortField field) => field switch
    {
        DuplicateFileSelectedRootFacetSortField.MatchingGroupCount => "matchingGroupCount",
        DuplicateFileSelectedRootFacetSortField.Value => "value",
        _ => throw new ArgumentOutOfRangeException(nameof(field)),
    };

    private static string DriveFacetSortField(DuplicateFileDriveFacetSortField field) => field switch
    {
        DuplicateFileDriveFacetSortField.MatchingGroupCount => "matchingGroupCount",
        DuplicateFileDriveFacetSortField.Value => "value",
        _ => throw new ArgumentOutOfRangeException(nameof(field)),
    };

    private static string FolderGroupSortField(DuplicateFolderGroupSortField field) => field switch
    {
        DuplicateFolderGroupSortField.TotalBytes => "totalBytes",
        DuplicateFolderGroupSortField.CopyCount => "copyCount",
        DuplicateFolderGroupSortField.FileCount => "fileCount",
        DuplicateFolderGroupSortField.RepresentativePath => "representativePath",
        _ => throw new ArgumentOutOfRangeException(nameof(field)),
    };

    private static string SortDirection(WorkerSortDirection direction) => direction switch
    {
        WorkerSortDirection.Ascending => "ascending",
        WorkerSortDirection.Descending => "descending",
        _ => throw new ArgumentOutOfRangeException(nameof(direction)),
    };

    private async Task PumpStandardOutputAsync(StreamReader output, CancellationToken cancellationToken)
    {
        try
        {
            while (await output.ReadLineAsync(cancellationToken).ConfigureAwait(false) is { } line)
            {
                var frame = JsonLineProtocol.ParseInboundFrame(line);
                if (frame.Response is not null && !_responses.TryComplete(frame.Response))
                {
                    throw new WorkerProtocolException(
                        $"Worker returned an unknown request ID: {frame.Response.Id}");
                }
                if (frame.Event is not null)
                {
                    DispatchEvent(frame.Event);
                }
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            _responses.FailAll(exception);
            TryTerminateProcess();
        }
    }

    private void DispatchEvent(EventEnvelope frame)
    {
        try
        {
            switch (frame.Name)
            {
                case "run.progress":
                    var progress = frame.Data.Deserialize<WorkerRunProgressEventArgs>(
                        JsonLineProtocol.SerializerOptions)
                        ?? throw new WorkerProtocolException("run.progress event data is invalid.");
                    RunProgress?.Invoke(this, progress);
                    break;

                case "run.started":
                case "run.completed":
                case "run.cancelled":
                case "run.failed":
                    var lifecycle = frame.Data.Deserialize<RunResult>(JsonLineProtocol.SerializerOptions)
                        ?? throw new WorkerProtocolException($"{frame.Name} event data is invalid.");
                    RunLifecycleChanged?.Invoke(
                        this,
                        new WorkerRunLifecycleEventArgs
                        {
                            EventName = frame.Name,
                            Run = lifecycle.Run,
                        });
                    break;
            }
        }
        catch (Exception exception) when (exception is not WorkerProtocolException)
        {
            throw new WorkerProtocolException($"Worker event {frame.Name} could not be read.", exception);
        }
    }

    private async Task PumpStandardErrorAsync(StreamReader error, CancellationToken cancellationToken)
    {
        await using var diagnosticLog = BoundedDiagnosticLog.TryOpen(_diagnosticLogPath);
        try
        {
            while (await error.ReadLineAsync(cancellationToken).ConfigureAwait(false) is { } line)
            {
                lock (_standardErrorLock)
                {
                    _standardError.AppendLine(line);
                    if (_standardError.Length > MaximumDiagnosticCharacters)
                    {
                        _standardError.Remove(0, _standardError.Length - MaximumDiagnosticCharacters);
                    }
                }
                if (diagnosticLog is not null)
                {
                    await diagnosticLog.TryWriteLineAsync(
                        $"{DateTimeOffset.UtcNow:O} {line}",
                        cancellationToken).ConfigureAwait(false);
                }
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
    }

    private static string DefaultDiagnosticLogPath() => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "SuperDuper",
        "logs",
        "worker.log");

    private async Task MonitorExitAsync(Process process, CancellationToken cancellationToken)
    {
        try
        {
            await process.WaitForExitAsync(cancellationToken).ConfigureAwait(false);
            if (Volatile.Read(ref _disposed) == 0 && Volatile.Read(ref _stopping) == 0)
            {
                var message = $"The worker exited unexpectedly with code {process.ExitCode}.";
                var exception = CreateConnectionException(message);
                _hello = null;
                _responses.FailAll(exception);
                UnexpectedExit?.Invoke(
                    this,
                    new WorkerUnexpectedExitEventArgs
                    {
                        ExitCode = process.ExitCode,
                        Message = message,
                        ExecutablePath = ExecutablePath,
                        DiagnosticLogPath = DiagnosticLogPath,
                    });
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
    }

    private async Task StopProcessAsync()
    {
        await _stopGate.WaitAsync().ConfigureAwait(false);
        Interlocked.Exchange(ref _stopping, 1);
        try
        {
            var process = _process;
            if (process is null)
            {
                return;
            }

            var standardInput = _standardInput;
            _standardInput = null;
            _responses.FailAll(new ObjectDisposedException(nameof(WorkerClient)));
            if (standardInput is not null)
            {
                await _writeGate.WaitAsync().ConfigureAwait(false);
                try
                {
                    await standardInput.DisposeAsync().ConfigureAwait(false);
                }
                catch (IOException)
                {
                }
                finally
                {
                    _writeGate.Release();
                }
            }

            if (!process.HasExited)
            {
                using var shutdownSource = new CancellationTokenSource(ShutdownTimeout);
                try
                {
                    await process.WaitForExitAsync(shutdownSource.Token).ConfigureAwait(false);
                }
                catch (OperationCanceledException) when (shutdownSource.IsCancellationRequested)
                {
                    process.Kill(entireProcessTree: true);
                    await process.WaitForExitAsync().ConfigureAwait(false);
                }
            }

            _lifetime.Cancel();
            await ObservePumpAsync(_standardOutputPump).ConfigureAwait(false);
            await ObservePumpAsync(_standardErrorPump).ConfigureAwait(false);
            await ObservePumpAsync(_exitMonitor).ConfigureAwait(false);
            process.Dispose();
            _process = null;
            _standardOutputPump = null;
            _standardErrorPump = null;
            _exitMonitor = null;
        }
        finally
        {
            Interlocked.Exchange(ref _stopping, 0);
            _stopGate.Release();
        }
    }

    private void ResetConnectionLifetime()
    {
        _lifetime.Cancel();
        _lifetime.Dispose();
        _lifetime = new CancellationTokenSource();
        lock (_standardErrorLock)
        {
            _standardError.Clear();
        }
    }

    private static async Task ObservePumpAsync(Task? task)
    {
        if (task is null)
        {
            return;
        }

        try
        {
            await task.ConfigureAwait(false);
        }
        catch
        {
            // The originating operation already surfaces stream/process failures.
        }
    }

    private WorkerConnectionException CreateConnectionException(string message, Exception? exception = null)
    {
        var diagnostics = GetStandardError();
        if (!string.IsNullOrWhiteSpace(diagnostics))
        {
            message += $" Worker stderr: {diagnostics.Trim()}";
        }

        return new WorkerConnectionException(ExecutablePath, message, exception);
    }

    private string GetStandardError()
    {
        lock (_standardErrorLock)
        {
            return _standardError.ToString();
        }
    }

    private void TryTerminateProcess()
    {
        try
        {
            if (_process is { HasExited: false } process)
            {
                process.Kill(entireProcessTree: true);
            }
        }
        catch (InvalidOperationException)
        {
        }
    }

    private sealed record SessionResult(WorkerSessionDefinition Session);

    private sealed record RunResult(WorkerRun Run);

    private sealed record DeleteSessionResult(long SessionId);
}
