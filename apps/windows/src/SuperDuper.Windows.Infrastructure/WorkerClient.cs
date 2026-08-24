using System.Diagnostics;
using System.Text;
using System.Text.Json;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Infrastructure.Protocol;

namespace SuperDuper.Windows.Infrastructure;

public sealed class WorkerClient : IRestartableWorkerClient, IRecycleOperationWorkerClient, IDisposable
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

    public Task<WorkerReviewLiveValidationResult> ValidateReviewFilesAsync(
        ReviewLiveValidationRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        return InvokeAsync<WorkerReviewLiveValidationResult>(
            "review_live_validation.run",
            new
            {
                operationId = request.OperationId,
                runId = request.RunId,
                groupId = request.GroupId,
                expectedReviewRevision = request.ExpectedReviewRevision,
                scope = request.Scope,
                fileIds = request.FileIds,
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

    public async Task<WorkerPreflight?> GetLatestPreflightAsync(
        long runId,
        CancellationToken cancellationToken = default)
    {
        var result = await InvokeAsync<PreflightResult>(
            "preflight.get",
            new { runId },
            cancellationToken).ConfigureAwait(false);
        return result.Preflight;
    }

    public async Task<WorkerPreflight> GetPreflightAsync(
        long preflightId,
        CancellationToken cancellationToken = default)
    {
        var result = await InvokeAsync<PreflightResult>(
            "preflight.get",
            new { preflightId },
            cancellationToken).ConfigureAwait(false);
        return result.Preflight
            ?? throw new WorkerProtocolException("preflight.get returned no preflight");
    }

    public Task<WorkerPreflightStartResult> StartPreflightAsync(
        string operationId,
        long runId,
        long expectedReviewRevision,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(operationId);
        return InvokeAsync<WorkerPreflightStartResult>(
            "preflight.start",
            new { operationId, runId, expectedReviewRevision },
            cancellationToken);
    }

    public Task<WorkerPreflightItemPage> GetPreflightItemsAsync(
        PreflightItemQuery query,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(query);
        return InvokeAsync<WorkerPreflightItemPage>(
            "preflight.item.page",
            new
            {
                preflightId = query.PreflightId,
                pageSize = query.PageSize,
                outcome = query.Outcome,
                cursor = query.Cursor,
            },
            cancellationToken);
    }

    public async Task<WorkerPreflight> CancelPreflightAsync(
        long preflightId,
        CancellationToken cancellationToken = default)
    {
        var result = await InvokeAsync<PreflightResult>(
            "preflight.cancel",
            new { preflightId },
            cancellationToken).ConfigureAwait(false);
        return result.Preflight
            ?? throw new WorkerProtocolException("preflight.cancel returned no preflight");
    }

    public Task<WorkerRecycleOperationResult> PrepareRecycleOperationAsync(
        string operationId,
        long runId,
        long preflightId,
        long expectedReviewRevision,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(operationId);
        return InvokeAsync<WorkerRecycleOperationResult>(
            "recycle_operation.prepare",
            new { operationId, runId, preflightId, expectedReviewRevision },
            cancellationToken);
    }

    public async Task<WorkerRecycleOperation?> GetLatestRecycleOperationAsync(
        long runId,
        CancellationToken cancellationToken = default)
    {
        var result = await InvokeAsync<RecycleOperationGetResult>(
            "recycle_operation.get",
            new { runId },
            cancellationToken).ConfigureAwait(false);
        return result.Operation;
    }

    public async Task<WorkerRecycleOperation> GetRecycleOperationAsync(
        long recycleOperationId,
        CancellationToken cancellationToken = default)
    {
        var result = await InvokeAsync<RecycleOperationGetResult>(
            "recycle_operation.get",
            new { recycleOperationId },
            cancellationToken).ConfigureAwait(false);
        return result.Operation
            ?? throw new WorkerProtocolException("recycle_operation.get returned no operation");
    }

    public Task<WorkerRecycleOperationItemPage> GetRecycleOperationItemsAsync(
        RecycleOperationItemQuery query,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(query);
        return InvokeAsync<WorkerRecycleOperationItemPage>(
            "recycle_operation.item.page",
            new
            {
                recycleOperationId = query.RecycleOperationId,
                pageSize = query.PageSize,
                resultStatus = query.ResultStatus,
                cursor = query.Cursor,
            },
            cancellationToken);
    }

    public Task<WorkerRecycleOperationResult> ReportRecycleEligibilityAsync(
        string reportOperationId,
        long recycleOperationId,
        IReadOnlyList<RecycleEligibilityObservation> items,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(reportOperationId);
        ArgumentNullException.ThrowIfNull(items);
        return InvokeAsync<WorkerRecycleOperationResult>(
            "recycle_operation.eligibility.report",
            new { reportOperationId, recycleOperationId, items },
            cancellationToken);
    }

    public Task<WorkerRecycleOperationResult> ConfirmRecycleOperationAsync(
        string reportOperationId,
        long recycleOperationId,
        string confirmationSignature,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(reportOperationId);
        ArgumentException.ThrowIfNullOrWhiteSpace(confirmationSignature);
        return InvokeAsync<WorkerRecycleOperationResult>(
            "recycle_operation.confirm",
            new { reportOperationId, recycleOperationId, confirmationSignature },
            cancellationToken);
    }

    public async Task<WorkerRecycleOperation> CancelRecycleOperationAsync(
        long recycleOperationId,
        CancellationToken cancellationToken = default)
    {
        var result = await InvokeAsync<RecycleOperationGetResult>(
            "recycle_operation.cancel",
            new { recycleOperationId },
            cancellationToken).ConfigureAwait(false);
        return result.Operation
            ?? throw new WorkerProtocolException("recycle_operation.cancel returned no operation");
    }

    public Task<WorkerRecycleOperationBatchResult> GetNextRecycleOperationBatchAsync(
        long recycleOperationId,
        CancellationToken cancellationToken = default) =>
        InvokeAsync<WorkerRecycleOperationBatchResult>(
            "recycle_operation.batch.next",
            new { recycleOperationId },
            cancellationToken);

    public Task<WorkerRecycleOperationResult> BeginRecycleOperationBatchAsync(
        string reportOperationId,
        long recycleOperationId,
        long batchId,
        string shellAttemptId,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(reportOperationId);
        ArgumentException.ThrowIfNullOrWhiteSpace(shellAttemptId);
        return InvokeAsync<WorkerRecycleOperationResult>(
            "recycle_operation.batch.begin",
            new { reportOperationId, recycleOperationId, batchId, shellAttemptId },
            cancellationToken);
    }

    public Task<WorkerRecycleOperationResult> ReportRecycleOperationBatchAsync(
        string reportOperationId,
        long recycleOperationId,
        long batchId,
        IReadOnlyList<RecycleItemResultObservation> items,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(reportOperationId);
        ArgumentNullException.ThrowIfNull(items);
        return InvokeAsync<WorkerRecycleOperationResult>(
            "recycle_operation.batch.report",
            new { reportOperationId, recycleOperationId, batchId, items },
            cancellationToken);
    }

    public Task<WorkerRecoveryReviewResult> GetRecoveryReviewAsync(
        long recycleOperationId,
        CancellationToken cancellationToken = default) =>
        InvokeAsync<WorkerRecoveryReviewResult>(
            "recovery_review.get",
            new { recycleOperationId },
            cancellationToken);

    public Task<WorkerRecoveryReviewObservationPage> GetRecoveryReviewObservationsAsync(
        RecoveryReviewObservationQuery query,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(query);
        return InvokeAsync<WorkerRecoveryReviewObservationPage>(
            "recovery_review.observation.page",
            new
            {
                recycleOperationId = query.RecycleOperationId,
                pageSize = query.PageSize,
                currentOnly = query.CurrentOnly,
                cursor = query.Cursor,
            },
            cancellationToken);
    }

    public Task<WorkerRecoveryReviewMutationResult> RecordRecoveryReviewObservationAsync(
        RecoveryReviewObservationRecord record,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(record);
        ArgumentException.ThrowIfNullOrWhiteSpace(record.RequestId);
        return InvokeAsync<WorkerRecoveryReviewMutationResult>(
            "recovery_review.observation.record",
            new
            {
                requestId = record.RequestId,
                recycleOperationId = record.RecycleOperationId,
                itemId = record.ItemId,
                observation = record.Observation,
                observedAt = record.ObservedAt,
                note = record.Note,
                evidenceVersion = record.EvidenceVersion,
                supersedesObservationId = record.SupersedesObservationId,
                correctionReason = record.CorrectionReason,
            },
            cancellationToken);
    }

    public Task<WorkerPreferenceRulePage> ListPreferenceRulesAsync(
        long offset = 0,
        int limit = 200,
        CancellationToken cancellationToken = default) =>
        InvokeAsync<WorkerPreferenceRulePage>(
            "preference_rule.list",
            new { offset, limit },
            cancellationToken);

    public async Task<WorkerPreferenceRule> GetPreferenceRuleAsync(
        long ruleId,
        CancellationToken cancellationToken = default)
    {
        var result = await InvokeAsync<PreferenceRuleResult>(
            "preference_rule.get",
            new { ruleId },
            cancellationToken).ConfigureAwait(false);
        return result.Rule;
    }

    public Task<WorkerPreferenceRuleSaveResult> SavePreferenceRuleAsync(
        string operationId,
        long? ruleId,
        string name,
        IReadOnlyList<string> roots,
        long expectedRevision,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(operationId);
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentNullException.ThrowIfNull(roots);
        return InvokeAsync<WorkerPreferenceRuleSaveResult>(
            "preference_rule.save",
            new { operationId, ruleId, name, roots, expectedRevision },
            cancellationToken);
    }

    public Task<WorkerPreferencePreviewPage> GetPreferencePreviewAsync(
        PreferencePreviewQuery query,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(query);
        var scope = PreferenceScope(query.Scope);
        return InvokeAsync<WorkerPreferencePreviewPage>(
            "preference_rule.preview",
            new
            {
                runId = query.RunId,
                ruleId = query.RuleId,
                ruleRevision = query.RuleRevision,
                reviewRevision = query.ReviewRevision,
                pageSize = query.PageSize,
                scope,
                cursor = query.Cursor,
            },
            cancellationToken);
    }

    public Task<WorkerPreferenceApplicationResult> ApplyPreferenceRuleAsync(
        string operationId,
        long runId,
        long ruleId,
        long ruleRevision,
        long sourceReviewRevision,
        string previewSignature,
        PreferencePreviewScope scope,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(operationId);
        ArgumentException.ThrowIfNullOrWhiteSpace(previewSignature);
        ArgumentNullException.ThrowIfNull(scope);
        return InvokeAsync<WorkerPreferenceApplicationResult>(
            "preference_rule.apply",
            new
            {
                operationId,
                runId,
                ruleId,
                ruleRevision,
                sourceReviewRevision,
                previewSignature,
                scope = PreferenceScope(scope),
            },
            cancellationToken);
    }

    public Task<WorkerPreferenceApplicationPage> GetPreferenceApplicationsAsync(
        long runId,
        long? ruleId,
        string state,
        int pageSize,
        string? cursor = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(state);
        return InvokeAsync<WorkerPreferenceApplicationPage>(
            "preference_rule.application.page",
            new { runId, ruleId, state, pageSize, cursor },
            cancellationToken);
    }

    public async Task<WorkerPreferenceApplication> GetPreferenceApplicationAsync(
        long runId,
        long applicationId,
        CancellationToken cancellationToken = default)
    {
        var result = await InvokeAsync<PreferenceApplicationResult>(
            "preference_rule.application.get",
            new { runId, applicationId },
            cancellationToken).ConfigureAwait(false);
        return result.Application;
    }

    public Task<WorkerPreferenceReversalResult> ReversePreferenceApplicationAsync(
        string operationId,
        long runId,
        long applicationId,
        long expectedRevision,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(operationId);
        return InvokeAsync<WorkerPreferenceReversalResult>(
            "preference_rule.application.reverse",
            new { operationId, runId, applicationId, expectedRevision },
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

    private static IReadOnlyDictionary<string, object?> PreferenceScope(
        PreferencePreviewScope scope)
    {
        var result = new Dictionary<string, object?>();
        switch (scope.Kind)
        {
            case PreferencePreviewScopeKind.SelectedSets:
                result["kind"] = "selected_sets";
                result["groupIds"] = scope.GroupIds
                    ?? throw new ArgumentException("Selected-set preview requires group IDs.", nameof(scope));
                break;
            case PreferencePreviewScopeKind.CurrentFilter:
                result["kind"] = "current_filter";
                result["filter"] = PreferenceFilter(scope.Filter
                    ?? throw new ArgumentException("Current-filter preview requires a complete filter.", nameof(scope)));
                break;
            case PreferencePreviewScopeKind.CompletedRun:
                result["kind"] = "completed_run";
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(scope));
        }
        return result;
    }

    private static object PreferenceFilter(DuplicateFileGroupFilter filter) => new
    {
        search = filter.Search,
        pathMatch = PathMatch(filter.PathMatch),
        extension = filter.Extension,
        extensionMatch = ExtensionMatch(filter.ExtensionMatch),
        minimumSize = filter.MinimumSize,
        minimumCopyCount = filter.MinimumCopyCount,
        acrossDrives = filter.AcrossDrives,
        selectedRoot = filter.SelectedRoot,
        selectedDrive = filter.SelectedDrive,
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

    private sealed record PreferenceRuleResult(WorkerPreferenceRule Rule);

    private sealed record PreferenceApplicationResult(WorkerPreferenceApplication Application);

    private sealed record PreflightResult(WorkerPreflight? Preflight);

    private sealed record RecycleOperationGetResult(
        WorkerRecycleOperation? Operation,
        bool ExecutorEnabled);

    private sealed record DeleteSessionResult(long SessionId);
}
