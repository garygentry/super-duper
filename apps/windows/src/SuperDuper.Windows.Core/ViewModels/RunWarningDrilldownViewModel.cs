using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class RunWarningDrilldownViewModel : ObservableObject, IDisposable
{
    public const int PageSize = 25;
    public const int CachePageLimit = 5;
    private const string DiagnosticRelationship = "supplemental_diagnostics_not_durable_warning_truth";
    private readonly IWorkerClient _workerClient;
    private readonly BoundedCursorCache<WorkerRunWarningPage> _cache = new(CachePageLimit);
    private CancellationTokenSource? _loadCancellation;
    private long _generation;
    private long? _runId;
    private bool _isOpen;
    private bool _isLoading;
    private string? _errorMessage;
    private string? _statusMessage;
    private string? _nextCursor;
    private RunWarningSortField _sortField = RunWarningSortField.OccurrenceCount;
    private WorkerSortDirection _sortDirection = WorkerSortDirection.Descending;
    private WarningSnapshotIdentity? _snapshotIdentity;
    private bool _terminalSnapshotAccepted;
    private long _announcementVersion;
    private long _errorAnnouncementVersion;

    public RunWarningDrilldownViewModel(IWorkerClient workerClient)
    {
        _workerClient = workerClient;
    }

    public ObservableCollection<WorkerRunWarningAggregate> Warnings { get; } = [];

    public long? RunId { get => _runId; private set => SetProperty(ref _runId, value); }

    public bool IsOpen { get => _isOpen; private set => SetProperty(ref _isOpen, value); }

    public bool IsLoading
    {
        get => _isLoading;
        private set
        {
            if (SetProperty(ref _isLoading, value))
            {
                OnPropertyChanged(nameof(CanLoadNextPage));
            }
        }
    }

    public string? ErrorMessage
    {
        get => _errorMessage;
        private set
        {
            if (SetProperty(ref _errorMessage, value))
            {
                OnPropertyChanged(nameof(HasError));
            }
        }
    }

    public string? StatusMessage { get => _statusMessage; private set => SetProperty(ref _statusMessage, value); }

    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);

    public bool CanLoadNextPage => IsOpen && !IsLoading && _nextCursor is not null;

    public RunWarningSortField SortField => _sortField;

    public WorkerSortDirection SortDirection => _sortDirection;

    public long? SnapshotRevision => _snapshotIdentity?.Revision;

    public string? SnapshotState => _snapshotIdentity?.State;

    public string? RunStatus => _snapshotIdentity?.RunStatus;

    public bool IsActiveSnapshot => SnapshotState == "active";

    public bool IsTerminalSnapshot => SnapshotState == "terminal";

    public long WarningCount { get; private set; }

    public long AccountedWarningCount { get; private set; }

    public WorkerDiagnosticLogMetadata? DiagnosticLog { get; private set; }

    public long AnnouncementVersion
    {
        get => _announcementVersion;
        private set => SetProperty(ref _announcementVersion, value);
    }

    public long ErrorAnnouncementVersion
    {
        get => _errorAnnouncementVersion;
        private set => SetProperty(ref _errorAnnouncementVersion, value);
    }

    internal int CachedPageCount => _cache.Count;

    public Task OpenAsync(long runId, CancellationToken cancellationToken = default)
    {
        if (runId <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(runId));
        }
        if (RunId != runId)
        {
            ResetRun(runId);
        }
        return LoadPageAsync(null, opening: true, bypassCache: !IsTerminalSnapshot, cancellationToken);
    }

    public Task RefreshAsync(CancellationToken cancellationToken = default) => RunId is null
        ? Task.CompletedTask
        : LoadPageAsync(null, opening: !IsOpen, bypassCache: !IsTerminalSnapshot, cancellationToken);

    public Task LoadNextPageAsync(CancellationToken cancellationToken = default) => _nextCursor is { } cursor
        ? LoadPageAsync(cursor, opening: false, bypassCache: false, cancellationToken)
        : Task.CompletedTask;

    public async Task ApplySortAsync(
        RunWarningSortField field,
        WorkerSortDirection direction,
        CancellationToken cancellationToken = default)
    {
        if (_sortField == field && _sortDirection == direction)
        {
            return;
        }
        CancelLoad();
        _sortField = field;
        _sortDirection = direction;
        OnPropertyChanged(nameof(SortField));
        OnPropertyChanged(nameof(SortDirection));
        _nextCursor = null;
        _cache.Clear();
        if (IsOpen && RunId is not null)
        {
            await LoadPageAsync(null, opening: false, bypassCache: true, cancellationToken);
        }
    }

    public void CancelLoad() => _loadCancellation?.Cancel();

    public void Close()
    {
        CancelAndInvalidateLoad();
        IsOpen = false;
        ErrorMessage = null;
        StatusMessage = null;
        _nextCursor = null;
        Warnings.Clear();
        _cache.Clear();
        ClearSnapshot();
        RunId = null;
    }

    internal void ClearError() => ErrorMessage = null;

    internal void ReportStatus(string message)
    {
        StatusMessage = message;
        AnnouncementVersion++;
    }

    internal void ReportError(string message)
    {
        ErrorMessage = message;
        ErrorAnnouncementVersion++;
    }

    private async Task LoadPageAsync(
        string? cursor,
        bool opening,
        bool bypassCache,
        CancellationToken cancellationToken)
    {
        if (RunId is not long runId)
        {
            return;
        }
        _loadCancellation?.Cancel();
        _loadCancellation?.Dispose();
        _loadCancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var token = _loadCancellation.Token;
        var generation = ++_generation;
        IsLoading = true;
        ErrorMessage = null;
        try
        {
            if (bypassCache || !_cache.TryGet(cursor, out var page))
            {
                page = await _workerClient.GetRunWarningsAsync(
                    new RunWarningQuery(runId, PageSize, _sortField, _sortDirection, cursor),
                    token);
            }
            if (!IsCurrent(generation, runId, token))
            {
                return;
            }
            ValidatePage(page, runId);
            var identity = new WarningSnapshotIdentity(
                page.SnapshotRevision,
                page.SnapshotState,
                page.RunStatus);
            AcceptIdentity(identity, cursor);
            if (!IsCurrent(generation, runId, token))
            {
                return;
            }
            _cache.Set(cursor, page);
            Warnings.Clear();
            foreach (var warning in page.Warnings)
            {
                Warnings.Add(warning);
            }
            _nextCursor = page.NextCursor;
            WarningCount = page.WarningCount;
            AccountedWarningCount = page.AccountedWarningCount;
            DiagnosticLog = page.DiagnosticLog;
            OnPropertyChanged(nameof(WarningCount));
            OnPropertyChanged(nameof(AccountedWarningCount));
            OnPropertyChanged(nameof(DiagnosticLog));
            OnPropertyChanged(nameof(CanLoadNextPage));
            IsOpen = true;
            StatusMessage = BuildStatus(page);
            AnnouncementVersion++;
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception) when (generation == _generation && RunId == runId)
        {
            ErrorMessage = exception.Message;
            ErrorAnnouncementVersion++;
            if (opening)
            {
                IsOpen = true;
            }
        }
        finally
        {
            if (generation == _generation)
            {
                IsLoading = false;
            }
        }
    }

    private void AcceptIdentity(WarningSnapshotIdentity identity, string? cursor)
    {
        if (_terminalSnapshotAccepted && identity.State != "terminal")
        {
            throw new InvalidOperationException("A terminal warning snapshot cannot return to an active state.");
        }
        if (_snapshotIdentity is { } current)
        {
            if (current.State == "terminal" && identity != current)
            {
                throw new InvalidOperationException("Completed warning history changed after its terminal snapshot was accepted.");
            }
            if (identity.Revision < current.Revision)
            {
                throw new InvalidOperationException("The warning snapshot revision moved backwards.");
            }
            if (current.State == "active" && current.RunStatus == "cancelling" && identity.RunStatus == "running")
            {
                throw new InvalidOperationException("A cancelling warning snapshot cannot return to running.");
            }
            if (cursor is not null && identity != current)
            {
                throw new InvalidOperationException("The warning page changed snapshot revision or state while paging.");
            }
            if (identity != current)
            {
                _cache.Clear();
            }
        }
        _snapshotIdentity = identity;
        _terminalSnapshotAccepted |= identity.State == "terminal";
        OnPropertyChanged(nameof(SnapshotRevision));
        OnPropertyChanged(nameof(SnapshotState));
        OnPropertyChanged(nameof(RunStatus));
        OnPropertyChanged(nameof(IsActiveSnapshot));
        OnPropertyChanged(nameof(IsTerminalSnapshot));
    }

    private static void ValidatePage(WorkerRunWarningPage page, long runId)
    {
        var stateMatchesStatus = page.SnapshotState switch
        {
            "active" => page.RunStatus is "running" or "cancelling",
            "terminal" => page.RunStatus is "completed" or "cancelled" or "failed" or "interrupted",
            "pending" => page.RunStatus == "pending",
            _ => false,
        };
        var diagnosticMetadataValid = page.DiagnosticLog is { } diagnosticLog
            && diagnosticLog.Relationship == DiagnosticRelationship
            && (diagnosticLog.State switch
            {
                "available" => diagnosticLog.LocationKind == "local_file"
                    && !string.IsNullOrWhiteSpace(diagnosticLog.Path)
                    && diagnosticLog.Reason is null,
                "unavailable" => diagnosticLog.LocationKind is null
                    && diagnosticLog.Path is null
                    && !string.IsNullOrWhiteSpace(diagnosticLog.Reason),
                _ => false,
            });
        if (page.ExecutorEnabled
            || page.SnapshotRevision < 0
            || !stateMatchesStatus
            || !diagnosticMetadataValid
            || page.WarningCount < 0
            || page.AccountedWarningCount < 0
            || page.WarningCount != page.AccountedWarningCount
            || page.Total < 0
            || page.Warnings.Count > PageSize
            || page.Total < page.Warnings.Count
            || page.Warnings.Any(warning => warning.RunId != runId || warning.OccurrenceCount <= 0))
        {
            throw new InvalidOperationException(
                "The worker returned an unsafe, unbounded, or incomplete warning accounting page.");
        }
    }

    private string BuildStatus(WorkerRunWarningPage page)
    {
        var lifecycle = page.SnapshotState switch
        {
            "active" => $"Live {page.RunStatus}",
            "terminal" => $"Terminal {page.RunStatus}",
            _ => "Pending",
        };
        var rows = page.Total == 0
            ? "No persisted warning aggregates are available."
            : $"Showing {page.Warnings.Count:N0} of {page.Total:N0} bounded warning aggregates, {SortDescription()}.";
        return $"{lifecycle} snapshot revision {page.SnapshotRevision:N0}: "
            + $"{page.AccountedWarningCount:N0} of {page.WarningCount:N0} warnings durably accounted. {rows}";
    }

    private string SortDescription() => (_sortField, _sortDirection) switch
    {
        (RunWarningSortField.OccurrenceCount, WorkerSortDirection.Descending) => "highest count first",
        (RunWarningSortField.OccurrenceCount, _) => "lowest count first",
        (RunWarningSortField.Phase, WorkerSortDirection.Ascending) => "phase A to Z",
        (RunWarningSortField.Phase, _) => "phase Z to A",
        (RunWarningSortField.Message, WorkerSortDirection.Ascending) => "warning text A to Z",
        _ => "warning text Z to A",
    };

    private bool IsCurrent(long generation, long runId, CancellationToken token) =>
        !token.IsCancellationRequested && generation == _generation && RunId == runId;

    private void ResetRun(long runId)
    {
        CancelAndInvalidateLoad();
        RunId = runId;
        IsOpen = false;
        ErrorMessage = null;
        StatusMessage = null;
        _nextCursor = null;
        Warnings.Clear();
        _cache.Clear();
        ClearSnapshot();
    }

    private void ClearSnapshot()
    {
        _snapshotIdentity = null;
        _terminalSnapshotAccepted = false;
        WarningCount = 0;
        AccountedWarningCount = 0;
        DiagnosticLog = null;
        OnPropertyChanged(nameof(SnapshotRevision));
        OnPropertyChanged(nameof(SnapshotState));
        OnPropertyChanged(nameof(RunStatus));
        OnPropertyChanged(nameof(IsActiveSnapshot));
        OnPropertyChanged(nameof(IsTerminalSnapshot));
        OnPropertyChanged(nameof(WarningCount));
        OnPropertyChanged(nameof(AccountedWarningCount));
        OnPropertyChanged(nameof(DiagnosticLog));
    }

    private void CancelAndInvalidateLoad()
    {
        _loadCancellation?.Cancel();
        _loadCancellation?.Dispose();
        _loadCancellation = null;
        _generation++;
        IsLoading = false;
    }

    public void Dispose() => Close();

    private sealed record WarningSnapshotIdentity(
        long Revision,
        string State,
        string RunStatus);
}
