using System.Diagnostics;
using System.Text;
using System.Text.Json;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Infrastructure.Protocol;

namespace SuperDuper.Windows.Infrastructure;

public sealed class WorkerClient : IWorkerClient
{
    private static readonly TimeSpan DefaultStartupTimeout = TimeSpan.FromSeconds(10);
    private static readonly TimeSpan ShutdownTimeout = TimeSpan.FromSeconds(2);
    private const int MaximumDiagnosticCharacters = 16_384;

    private readonly TimeSpan _startupTimeout;
    private readonly string? _databasePath;
    private readonly ResponseCorrelator _responses = new();
    private readonly SemaphoreSlim _connectionGate = new(1, 1);
    private readonly SemaphoreSlim _writeGate = new(1, 1);
    private readonly SemaphoreSlim _stopGate = new(1, 1);
    private readonly CancellationTokenSource _lifetime = new();
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

    public WorkerClient(string executablePath)
        : this(executablePath, DefaultStartupTimeout)
    {
    }

    internal WorkerClient(string executablePath, TimeSpan startupTimeout)
        : this(executablePath, startupTimeout, databasePath: null)
    {
    }

    internal WorkerClient(string executablePath, TimeSpan startupTimeout, string? databasePath)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(executablePath);
        ExecutablePath = Path.GetFullPath(executablePath);
        _startupTimeout = startupTimeout;
        _databasePath = databasePath is null ? null : Path.GetFullPath(databasePath);
    }

    public string ExecutablePath { get; }

    public event EventHandler<WorkerRunProgressEventArgs>? RunProgress;

    public event EventHandler<WorkerRunLifecycleEventArgs>? RunLifecycleChanged;

    public async Task<WorkerHelloResult> ConnectAsync(CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref _disposed) != 0, this);

        if (_hello is not null)
        {
            return _hello;
        }

        await _connectionGate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            if (_hello is not null)
            {
                return _hello;
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
        finally
        {
            _connectionGate.Release();
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
        CancellationToken cancellationToken = default) =>
        (await InvokeAsync<SessionResult>(
            "session.create",
            new { name, roots, ignorePatterns },
            cancellationToken).ConfigureAwait(false)).Session;

    public async Task<WorkerSessionDefinition> UpdateSessionAsync(
        long sessionId,
        string name,
        IReadOnlyList<string> roots,
        IReadOnlyList<string> ignorePatterns,
        CancellationToken cancellationToken = default) =>
        (await InvokeAsync<SessionResult>(
            "session.update",
            new { sessionId, name, roots, ignorePatterns },
            cancellationToken).ConfigureAwait(false)).Session;

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
                    minimumSize = query.Filter.MinimumSize,
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

    private static string MemberSortField(DuplicateFileMemberSortField field) => field switch
    {
        DuplicateFileMemberSortField.Path => "path",
        DuplicateFileMemberSortField.ModifiedTime => "modifiedTime",
        DuplicateFileMemberSortField.Size => "size",
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
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
    }

    private async Task MonitorExitAsync(Process process, CancellationToken cancellationToken)
    {
        try
        {
            await process.WaitForExitAsync(cancellationToken).ConfigureAwait(false);
            if (Volatile.Read(ref _disposed) == 0)
            {
                _responses.FailAll(CreateConnectionException(
                    $"The worker exited unexpectedly with code {process.ExitCode}."));
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
    }

    private async Task StopProcessAsync()
    {
        await _stopGate.WaitAsync().ConfigureAwait(false);
        try
        {
            var process = _process;
            if (process is null)
            {
                return;
            }

            if (_standardInput is not null)
            {
                try
                {
                    await _standardInput.DisposeAsync().ConfigureAwait(false);
                }
                catch (IOException)
                {
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
            _standardInput = null;
        }
        finally
        {
            _stopGate.Release();
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
