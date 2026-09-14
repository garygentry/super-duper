using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class RunHistoryViewModel : ObservableObject, IDisposable
{
    public const int HistoryPageSize = 500;
    public const string HashWarningCode = "hash_recoverable_warning";
    public const int WarningPageSize = RunWarningDrilldownViewModel.PageSize;
    public const int WarningCachePageLimit = RunWarningDrilldownViewModel.CachePageLimit;
    private readonly IWorkerClient _workerClient;
    private readonly RunWarningDrilldownViewModel _warningDrilldown;
    private readonly Func<WorkerRun, CancellationToken, Task>? _navigateToDuplicateSet;
    private readonly Func<WorkerRun?> _workspaceRun;
    private readonly Func<WorkerRun?> _activeRun;
    private readonly Func<long, string?> _sessionName;
    private readonly Action<WarningReturnDestination>? _returnFromWarnings;
    private readonly Func<WorkerRun, CancellationToken, Task>? _openPerformance;
    private readonly HashSet<long> _knownRunIds = [];
    private long? _sessionId;
    private RunListItemViewModel? _selectedRun;
    private bool _isLoading;
    private string? _errorMessage;
    private CancellationTokenSource? _warningNavigationCancellation;
    private long _warningNavigationGeneration;
    private bool _isWarningNavigationPending;
    private string _focusTarget = string.Empty;
    private long _focusRequestVersion;
    private long _loadGeneration;
    private long _historyOffset;
    private long _historyTotal;
    private WarningReturnDestination _warningReturnDestination;

    public RunHistoryViewModel(
        IWorkerClient workerClient,
        Func<WorkerRun, CancellationToken, Task>? navigateToDuplicateSet = null,
        Func<WorkerRun?>? workspaceRun = null,
        Func<WorkerRun?>? activeRun = null,
        Func<long, string?>? sessionName = null,
        Action<WarningReturnDestination>? returnFromWarnings = null,
        Func<WorkerRun, CancellationToken, Task>? openPerformance = null)
    {
        _workerClient = workerClient;
        _warningDrilldown = new RunWarningDrilldownViewModel(workerClient);
        _warningDrilldown.PropertyChanged += WarningDrilldownPropertyChanged;
        _navigateToDuplicateSet = navigateToDuplicateSet;
        _workspaceRun = workspaceRun ?? (() => null);
        _activeRun = activeRun ?? (() => null);
        _sessionName = sessionName ?? (_ => null);
        _returnFromWarnings = returnFromWarnings;
        _openPerformance = openPerformance;
        RefreshCommand = new AsyncRelayCommand(RefreshAsync, () => SessionId is not null && !IsLoading);
        PreviousHistoryPageCommand = new AsyncRelayCommand(PreviousHistoryPageAsync, () => CanLoadPreviousHistoryPage);
        NextHistoryPageCommand = new AsyncRelayCommand(NextHistoryPageAsync, () => CanLoadNextHistoryPage);
        OpenWarningsCommand = new AsyncRelayCommand(OpenWarningsAsync, () => CanOpenWarnings);
        OpenPerformanceCommand = new AsyncRelayCommand(OpenPerformanceAsync, () => CanOpenPerformance);
        RefreshWarningsCommand = new AsyncRelayCommand(RefreshWarningsAsync, () => CanRefreshWarnings);
        NextWarningPageCommand = new AsyncRelayCommand(NextWarningPageAsync, () => CanLoadNextWarningPage);
        CancelWarningLoadCommand = new RelayCommand(CancelWarningLoad, () => IsWarningLoading);
        NavigateWarningCommand = new AsyncRelayCommand<WorkerRunWarningAggregate>(
            NavigateWarningAsync,
            CanNavigateWarning);
        CancelWarningNavigationCommand = new RelayCommand(
            CancelWarningNavigation,
            () => IsWarningNavigationPending);
        CloseWarningsCommand = new RelayCommand(CloseWarnings, () => IsWarningDrilldownOpen);
        _warningDrilldown.Warnings.CollectionChanged += (_, _) =>
            NavigateWarningCommand.NotifyCanExecuteChanged();
    }

    public ObservableCollection<RunListItemViewModel> Runs { get; } = [];

    public ObservableCollection<WorkerRunWarningAggregate> Warnings => _warningDrilldown.Warnings;

    internal RunWarningDrilldownViewModel WarningDrilldown => _warningDrilldown;

    public long? SessionId
    {
        get => _sessionId;
        private set
        {
            if (SetProperty(ref _sessionId, value))
            {
                RefreshCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public RunListItemViewModel? SelectedRun
    {
        get => _selectedRun;
        set
        {
            if (SetProperty(ref _selectedRun, value))
            {
                CloseWarnings(restoreFocus: false);
                SelectedRunChanged?.Invoke(this, value?.Run);
                RaiseSelectedRunContext();
                OnPropertyChanged(nameof(CanOpenWarnings));
                OnPropertyChanged(nameof(CanOpenPerformance));
                OpenWarningsCommand.NotifyCanExecuteChanged();
                OpenPerformanceCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public bool IsLoading
    {
        get => _isLoading;
        private set
        {
            if (SetProperty(ref _isLoading, value))
            {
                OnPropertyChanged(nameof(IsEmpty));
                RefreshCommand.NotifyCanExecuteChanged();
                OnPropertyChanged(nameof(CanOpenPerformance));
                OpenPerformanceCommand.NotifyCanExecuteChanged();
                RaiseHistoryPagingState();
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

    public bool IsEmpty => !IsLoading && Runs.Count == 0;

    public bool HasSelectedRun => SelectedRun is not null;

    public long HistoryTotal => _historyTotal;

    public bool CanLoadPreviousHistoryPage => !IsLoading && _historyOffset > 0;

    public bool CanLoadNextHistoryPage => !IsLoading && _historyOffset + Runs.Count < _historyTotal;

    public string HistoryPageStatus => _historyTotal == 0
        ? "No scan runs"
        : $"Showing scans {_historyOffset + 1:N0}–{_historyOffset + Runs.Count:N0} of {_historyTotal:N0}; newest first.";

    public string SelectedRunIdentity => SelectedRun is { Run: { } run }
        ? $"{RunSessionName(run)} · Scan {run.Id:N0} · {RunDate(run)} · {DisplayFormatting.Status(run.Status)}"
        : "No scan highlighted";

    public string SelectedRunRelationship
    {
        get
        {
            if (SelectedRun?.Run is not { } highlighted) return "Highlight a scan to inspect its recorded settings.";
            if (_workspaceRun() is not { } opened)
                return "Highlighted only. No scan is open in the Results and Review workspace.";
            return opened.Id == highlighted.Id
                ? "This highlighted scan is open in the Results and Review workspace."
                : $"Highlighted only. The workspace remains on {RunSessionName(opened)} · Scan {opened.Id:N0} · {RunDate(opened)} · {DisplayFormatting.Status(opened.Status)}.";
        }
    }

    public string SelectedRunParameters => SelectedRun?.Run is { } run
        ? $"Recorded settings for this immutable run: {FormatCount(run.Parameters.Roots.Count, "location")}; "
          + $"{RepeatPolicy(run.Parameters.RepeatCachePolicy)}; {FormatCount(run.Parameters.IgnorePatterns.Count, "ignore pattern")}; "
          + $"{FormatCount(run.Parameters.ManualLocationExclusions.Count + run.Parameters.RegisteredCloudLocations.Count, "recorded exclusion")}. "
          + "Editing the saved scan cannot change these settings, results, or decisions."
        : string.Empty;

    public IReadOnlyList<string> SelectedRunRoots => SelectedRun?.Run.Parameters.Roots ?? [];

    public bool HasActiveRunContext => _activeRun() is not null;

    public string ActiveRunContext => _activeRun() is { } run
        ? $"Active scan is separate: {RunSessionName(run)} · Scan {run.Id:N0} · {RunDate(run)} · {DisplayFormatting.Status(run.Status)}."
        : "No active scan.";

    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);

    public bool IsWarningDrilldownOpen => _warningDrilldown.IsOpen;

    public bool IsWarningLoading => _warningDrilldown.IsLoading;

    public string? WarningErrorMessage => _warningDrilldown.ErrorMessage;

    public string? WarningStatusMessage => _warningDrilldown.StatusMessage;

    public string WarningDiagnosticLogStatus => _warningDrilldown.DiagnosticLogStatus;

    public string? WarningDiagnosticLogPath => _warningDrilldown.DiagnosticLogPath;

    public string WarningDiagnosticLogAutomationName => _warningDrilldown.DiagnosticLogAutomationName;

    public bool HasWarningError => _warningDrilldown.HasError;

    public bool IsWarningNavigationPending
    {
        get => _isWarningNavigationPending;
        private set
        {
            if (SetProperty(ref _isWarningNavigationPending, value))
            {
                NavigateWarningCommand.NotifyCanExecuteChanged();
                CancelWarningNavigationCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public bool CanOpenWarnings => SelectedRun?.Run.WarningCount > 0 && !IsWarningLoading && !IsWarningDrilldownOpen;

    public bool CanOpenPerformance => SelectedRun is not null && !IsLoading && _openPerformance is not null;

    public bool CanRefreshWarnings => IsWarningDrilldownOpen && !IsWarningLoading && _warningDrilldown.IsActiveSnapshot;

    public string WarningContextHeading => _warningDrilldown.IsActiveSnapshot
        ? "Warnings · active scan"
        : "Warnings · selected scan";

    public string WarningContextIdentity => WarningRun() is { } run
        ? $"{RunSessionName(run)} · Scan {run.Id:N0} · {RunDate(run)} · {DisplayFormatting.Status(run.Status)}"
        : "Warning run unavailable";

    public string WarningSnapshotBoundary => _warningDrilldown.SnapshotRevision is not long revision
        ? "Loading the worker-owned warning revision…"
        : _warningDrilldown.IsTerminalSnapshot
            ? $"Terminal warning revision {revision:N0}. This accepted historical warning snapshot cannot return to an active state."
            : $"Current warning revision {revision:N0}. Refresh replaces this page as one revision and never combines pages from different revisions.";

    public string WarningReturnLabel => _warningReturnDestination switch
    {
        WarningReturnDestination.ScanProgress => "_Return to progress",
        WarningReturnDestination.ScanSummary => "_Return to scan summary",
        _ => "_Return to run history",
    };

    public string WarningReturnAutomationName => _warningReturnDestination switch
    {
        WarningReturnDestination.ScanProgress => "Close warning details and return focus to the active scan warning entry",
        WarningReturnDestination.ScanSummary => "Close warning details and return focus to the selected scan summary warning entry",
        _ => "Close warning details and return focus to the highlighted run in history",
    };

    public bool CanLoadNextWarningPage => _warningDrilldown.CanLoadNextPage;

    public RunWarningSortField WarningSortField => _warningDrilldown.SortField;

    public WorkerSortDirection WarningSortDirection => _warningDrilldown.SortDirection;

    public long WarningAnnouncementVersion
    {
        get => _warningDrilldown.AnnouncementVersion;
    }

    public long WarningErrorAnnouncementVersion
    {
        get => _warningDrilldown.ErrorAnnouncementVersion;
    }

    public string FocusTarget { get => _focusTarget; private set => SetProperty(ref _focusTarget, value); }

    public long FocusRequestVersion { get => _focusRequestVersion; private set => SetProperty(ref _focusRequestVersion, value); }

    public IAsyncRelayCommand RefreshCommand { get; }

    public IAsyncRelayCommand PreviousHistoryPageCommand { get; }

    public IAsyncRelayCommand NextHistoryPageCommand { get; }

    public IAsyncRelayCommand OpenWarningsCommand { get; }

    public IAsyncRelayCommand OpenPerformanceCommand { get; }

    public IAsyncRelayCommand RefreshWarningsCommand { get; }

    public IAsyncRelayCommand NextWarningPageCommand { get; }

    public IRelayCommand CancelWarningLoadCommand { get; }

    public IAsyncRelayCommand<WorkerRunWarningAggregate> NavigateWarningCommand { get; }

    public IRelayCommand CancelWarningNavigationCommand { get; }

    public IRelayCommand CloseWarningsCommand { get; }

    public event EventHandler<WorkerRun?>? SelectedRunChanged;

    public async Task LoadAsync(long sessionId, CancellationToken cancellationToken = default)
    {
        if (SessionId != sessionId)
        {
            Runs.Clear();
            SelectedRun = null;
            _historyOffset = 0;
            _historyTotal = 0;
            _knownRunIds.Clear();
        }
        SessionId = sessionId;
        await LoadHistoryPageAsync(sessionId, 0, preserveDisplayedRuns: false, cancellationToken);
    }

    private async Task LoadHistoryPageAsync(
        long sessionId,
        long offset,
        bool preserveDisplayedRuns,
        CancellationToken cancellationToken = default)
    {
        var generation = ++_loadGeneration;
        IsLoading = true;
        ErrorMessage = null;
        try
        {
            var page = await _workerClient.ListRunsAsync(sessionId, offset, HistoryPageSize, cancellationToken);
            if (generation != _loadGeneration || cancellationToken.IsCancellationRequested) return;
            if (page.Total < 0
                || page.Runs.Count > HistoryPageSize
                || offset > page.Total
                || offset + page.Runs.Count > page.Total
                || (offset < page.Total && page.Runs.Count == 0))
                throw new InvalidOperationException("The worker returned an unsafe or unbounded run-history page.");
            if (page.Runs.Any(run => run.SessionId != sessionId)
                || page.Runs.Select(run => run.Id).Distinct().Count() != page.Runs.Count)
                throw new InvalidOperationException("The worker mixed saved scans or duplicate runs into one history page.");

            var selectedId = SelectedRun?.Id;
            Runs.Clear();
            foreach (var run in page.Runs)
            {
                _knownRunIds.Add(run.Id);
                Runs.Add(new RunListItemViewModel(run));
            }
            _historyOffset = offset;
            _historyTotal = page.Total;
            SelectedRun = selectedId is long id
                ? Runs.FirstOrDefault(item => item.Id == id) ?? Runs.FirstOrDefault()
                : Runs.FirstOrDefault();
            OnPropertyChanged(nameof(IsEmpty));
            RaiseHistoryPagingState();
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            // Cancellation only dismisses this history request; a newer context owns the view.
        }
        catch (Exception exception)
        {
            if (generation == _loadGeneration && !cancellationToken.IsCancellationRequested)
            {
                ErrorMessage = preserveDisplayedRuns && Runs.Count > 0
                    ? $"Run history could not be replaced. Retained the accepted page. {exception.Message}"
                    : exception.Message;
            }
        }
        finally
        {
            if (generation == _loadGeneration) IsLoading = false;
        }
    }

    public void Clear()
    {
        ++_loadGeneration;
        IsLoading = false;
        CloseWarnings(restoreFocus: false);
        SessionId = null;
        _historyOffset = 0;
        _historyTotal = 0;
        _knownRunIds.Clear();
        Runs.Clear();
        SelectedRun = null;
        ErrorMessage = null;
        OnPropertyChanged(nameof(IsEmpty));
        RaiseHistoryPagingState();
        NotifyExternalContextChanged();
    }

    public async Task ApplyWarningSortAsync(
        RunWarningSortField field,
        WorkerSortDirection direction)
    {
        CancelWarningNavigation(clearFeedback: true);
        await _warningDrilldown.ApplySortAsync(field, direction);
        if (IsWarningDrilldownOpen && !HasWarningError)
        {
            RequestFocus("warnings");
        }
    }

    public void Upsert(WorkerRun run, bool select)
    {
        if (SessionId != run.SessionId)
        {
            return;
        }
        var item = Runs.FirstOrDefault(existing => existing.Id == run.Id);
        if (item is null)
        {
            item = new RunListItemViewModel(run);
            if (_knownRunIds.Add(run.Id))
                _historyTotal = Math.Max(_historyTotal + 1, Runs.Count + 1);
            if (_historyOffset == 0 || select)
            {
                if (select && _historyOffset > 0)
                {
                    _historyOffset = 0;
                    Runs.Clear();
                }
                Runs.Insert(0, item);
                while (Runs.Count > HistoryPageSize) Runs.RemoveAt(Runs.Count - 1);
            }
        }
        else
        {
            item.Update(run);
            if (ReferenceEquals(SelectedRun, item)) RaiseSelectedRunContext();
        }
        RaiseHistoryPagingState();
        RaiseWarningContext();
        NavigateWarningCommand.NotifyCanExecuteChanged();
        if (select)
        {
            SelectedRun = item;
            if (SessionId is long sessionId && Runs.Count < Math.Min(HistoryPageSize, _historyTotal))
                _ = LoadHistoryPageAsync(sessionId, 0, preserveDisplayedRuns: true);
        }
        OnPropertyChanged(nameof(IsEmpty));
    }

    public async Task OpenWarningsForRunAsync(
        WorkerRun run,
        WarningReturnDestination returnDestination = WarningReturnDestination.History,
        CancellationToken cancellationToken = default)
    {
        Upsert(run, select: true);
        if (SelectedRun?.Id != run.Id)
        {
            throw new InvalidOperationException(
                "The current warning run is not available in the selected session history.");
        }
        _warningReturnDestination = returnDestination;
        RaiseWarningContext();
        await LoadWarningPageAsync(opening: true, cancellationToken);
    }

    private Task RefreshAsync() => SessionId is long sessionId
        ? LoadHistoryPageAsync(sessionId, _historyOffset, preserveDisplayedRuns: true)
        : Task.CompletedTask;

    private async Task PreviousHistoryPageAsync()
    {
        if (SessionId is not long sessionId || !CanLoadPreviousHistoryPage) return;
        await LoadHistoryPageAsync(sessionId, Math.Max(0, _historyOffset - HistoryPageSize), preserveDisplayedRuns: true);
        if (!HasError) RequestFocus("history");
    }

    private async Task NextHistoryPageAsync()
    {
        if (SessionId is not long sessionId || !CanLoadNextHistoryPage) return;
        await LoadHistoryPageAsync(sessionId, _historyOffset + Runs.Count, preserveDisplayedRuns: true);
        if (!HasError) RequestFocus("history");
    }

    private Task OpenWarningsAsync()
    {
        _warningReturnDestination = WarningReturnDestination.History;
        RaiseWarningContext();
        return LoadWarningPageAsync(opening: true);
    }

    private Task OpenPerformanceAsync(CancellationToken cancellationToken) =>
        SelectedRun?.Run is { } run && _openPerformance is not null
            ? _openPerformance(run, cancellationToken)
            : Task.CompletedTask;

    private async Task RefreshWarningsAsync()
    {
        await _warningDrilldown.RefreshAsync();
        if (IsWarningDrilldownOpen && !HasWarningError) RequestFocus("warnings");
    }

    private Task NextWarningPageAsync() => LoadWarningPageAsync(opening: false);

    private async Task LoadWarningPageAsync(
        bool opening,
        CancellationToken cancellationToken = default)
    {
        var run = SelectedRun?.Run;
        if (run is null || run.WarningCount <= 0)
        {
            return;
        }
        CancelWarningNavigation(clearFeedback: true);
        if (opening)
        {
            await _warningDrilldown.OpenAsync(run.Id, cancellationToken);
        }
        else
        {
            await _warningDrilldown.LoadNextPageAsync(cancellationToken);
        }
        if (SelectedRun?.Id == run.Id && IsWarningDrilldownOpen && !HasWarningError)
        {
            RequestFocus("warnings");
        }
    }

    private void CancelWarningLoad() => _warningDrilldown.CancelLoad();

    private bool CanNavigateWarning(WorkerRunWarningAggregate? warning) =>
        _navigateToDuplicateSet is not null
        && IsWarningDrilldownOpen
        && !IsWarningNavigationPending
        && warning is not null
        && warning.Category == "scan"
        && warning.Code == HashWarningCode
        && SelectedRun?.Run.Status == "completed"
        && SelectedRun?.Id == warning.RunId
        && Warnings.Any(current => current.Id == warning.Id && current.RunId == warning.RunId);

    private async Task NavigateWarningAsync(WorkerRunWarningAggregate? warning)
    {
        if (warning is null || !CanNavigateWarning(warning) || SelectedRun?.Run is not { } selectedRun)
        {
            return;
        }

        _warningNavigationCancellation?.Cancel();
        _warningNavigationCancellation?.Dispose();
        _warningNavigationCancellation = new CancellationTokenSource();
        var token = _warningNavigationCancellation.Token;
        var generation = ++_warningNavigationGeneration;
        var warningId = warning.Id;
        var runId = warning.RunId;
        IsWarningNavigationPending = true;
        _warningDrilldown.ClearError();
        _warningDrilldown.ReportStatus($"Opening immutable duplicate-file results for run {runId:N0}…");

        try
        {
            var target = await _workerClient.GetRunAsync(runId, token);
            if (target.Id != runId
                || target.SessionId != selectedRun.SessionId
                || target.Status != "completed")
            {
                throw new InvalidOperationException(
                    "The warning target is not an immutable result set owned by the selected run.");
            }
            if (!IsCurrentWarningNavigation(generation, warningId, runId, token))
            {
                return;
            }

            await _navigateToDuplicateSet!(target, token);
            if (!IsCurrentWarningNavigation(generation, warningId, runId, token))
            {
                return;
            }

            _warningDrilldown.ReportStatus(
                $"Opened immutable duplicate-file results for run {runId:N0}. Persisted warning history was not changed.");
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception) when (IsCurrentWarningNavigation(generation, warningId, runId, token))
        {
            _warningDrilldown.ReportError(
                $"The immutable duplicate-file results for run {runId:N0} are unavailable. " +
                $"Refresh run history before trying this warning again. {exception.Message}");
            RequestFocus($"warning-action:{warningId}");
        }
        finally
        {
            if (generation == _warningNavigationGeneration)
            {
                IsWarningNavigationPending = false;
            }
        }
    }

    private bool IsCurrentWarningNavigation(
        long generation,
        long warningId,
        long runId,
        CancellationToken token) =>
        !token.IsCancellationRequested
        && generation == _warningNavigationGeneration
        && IsWarningDrilldownOpen
        && SelectedRun?.Id == runId
        && Warnings.Any(warning => warning.Id == warningId && warning.RunId == runId);

    private void CancelWarningNavigation() => CancelWarningNavigation(clearFeedback: false);

    private void CancelWarningNavigation(bool clearFeedback)
    {
        var warningId = Warnings.FirstOrDefault(warning =>
            warning.Category == "scan" && warning.Code == HashWarningCode)?.Id;
        var wasPending = IsWarningNavigationPending;
        _warningNavigationCancellation?.Cancel();
        _warningNavigationCancellation?.Dispose();
        _warningNavigationCancellation = null;
        _warningNavigationGeneration++;
        IsWarningNavigationPending = false;
        if (clearFeedback)
        {
            _warningDrilldown.ClearError();
            return;
        }
        if (wasPending)
        {
            _warningDrilldown.ReportStatus(
                "Warning navigation was cancelled. Persisted warning history was not changed.");
            if (warningId is long id)
            {
                RequestFocus($"warning-action:{id}");
            }
        }
    }

    private void CloseWarnings() => CloseWarnings(restoreFocus: true);

    private void CloseWarnings(bool restoreFocus)
    {
        var returnDestination = _warningReturnDestination;
        CancelWarningNavigation(clearFeedback: true);
        _warningDrilldown.Close();
        _warningReturnDestination = WarningReturnDestination.History;
        RaiseWarningContext();
        NavigateWarningCommand.NotifyCanExecuteChanged();
        if (!restoreFocus) return;
        if (returnDestination == WarningReturnDestination.History || _returnFromWarnings is null)
            RequestFocus("history");
        else
            _returnFromWarnings(returnDestination);
    }

    private void RequestFocus(string target)
    {
        FocusTarget = target;
        FocusRequestVersion++;
    }

    private void WarningDrilldownPropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs args)
    {
        var propertyName = args.PropertyName switch
        {
            nameof(RunWarningDrilldownViewModel.IsOpen) => nameof(IsWarningDrilldownOpen),
            nameof(RunWarningDrilldownViewModel.IsLoading) => nameof(IsWarningLoading),
            nameof(RunWarningDrilldownViewModel.ErrorMessage) => nameof(WarningErrorMessage),
            nameof(RunWarningDrilldownViewModel.HasError) => nameof(HasWarningError),
            nameof(RunWarningDrilldownViewModel.StatusMessage) => nameof(WarningStatusMessage),
            nameof(RunWarningDrilldownViewModel.DiagnosticLogStatus) => nameof(WarningDiagnosticLogStatus),
            nameof(RunWarningDrilldownViewModel.DiagnosticLogPath) => nameof(WarningDiagnosticLogPath),
            nameof(RunWarningDrilldownViewModel.DiagnosticLogAutomationName) => nameof(WarningDiagnosticLogAutomationName),
            nameof(RunWarningDrilldownViewModel.CanLoadNextPage) => nameof(CanLoadNextWarningPage),
            nameof(RunWarningDrilldownViewModel.SortField) => nameof(WarningSortField),
            nameof(RunWarningDrilldownViewModel.SortDirection) => nameof(WarningSortDirection),
            nameof(RunWarningDrilldownViewModel.AnnouncementVersion) => nameof(WarningAnnouncementVersion),
            nameof(RunWarningDrilldownViewModel.ErrorAnnouncementVersion) => nameof(WarningErrorAnnouncementVersion),
            _ => null,
        };
        if (propertyName is not null)
        {
            OnPropertyChanged(propertyName);
        }
        OnPropertyChanged(nameof(CanOpenWarnings));
        OnPropertyChanged(nameof(CanOpenPerformance));
        RaiseWarningContext();
        OpenWarningsCommand.NotifyCanExecuteChanged();
        OpenPerformanceCommand.NotifyCanExecuteChanged();
        RefreshWarningsCommand.NotifyCanExecuteChanged();
        NextWarningPageCommand.NotifyCanExecuteChanged();
        CancelWarningLoadCommand.NotifyCanExecuteChanged();
        CloseWarningsCommand.NotifyCanExecuteChanged();
        NavigateWarningCommand.NotifyCanExecuteChanged();
    }

    public void NotifyExternalContextChanged()
    {
        RaiseSelectedRunContext();
        OnPropertyChanged(nameof(HasActiveRunContext));
        OnPropertyChanged(nameof(ActiveRunContext));
    }

    private void RaiseSelectedRunContext()
    {
        OnPropertyChanged(nameof(HasSelectedRun));
        OnPropertyChanged(nameof(SelectedRunIdentity));
        OnPropertyChanged(nameof(SelectedRunRelationship));
        OnPropertyChanged(nameof(SelectedRunParameters));
        OnPropertyChanged(nameof(SelectedRunRoots));
    }

    private void RaiseHistoryPagingState()
    {
        OnPropertyChanged(nameof(HistoryTotal));
        OnPropertyChanged(nameof(HistoryPageStatus));
        OnPropertyChanged(nameof(CanLoadPreviousHistoryPage));
        OnPropertyChanged(nameof(CanLoadNextHistoryPage));
        PreviousHistoryPageCommand.NotifyCanExecuteChanged();
        NextHistoryPageCommand.NotifyCanExecuteChanged();
    }

    private void RaiseWarningContext()
    {
        OnPropertyChanged(nameof(CanRefreshWarnings));
        OnPropertyChanged(nameof(WarningContextHeading));
        OnPropertyChanged(nameof(WarningContextIdentity));
        OnPropertyChanged(nameof(WarningSnapshotBoundary));
        OnPropertyChanged(nameof(WarningReturnLabel));
        OnPropertyChanged(nameof(WarningReturnAutomationName));
    }

    private WorkerRun? WarningRun()
    {
        if (_warningDrilldown.RunId is not long runId) return null;
        return SelectedRun?.Run.Id == runId ? SelectedRun.Run
            : Runs.FirstOrDefault(item => item.Id == runId)?.Run
            ?? (_activeRun()?.Id == runId ? _activeRun() : null)
            ?? (_workspaceRun()?.Id == runId ? _workspaceRun() : null);
    }

    private string RunSessionName(WorkerRun run) => _sessionName(run.SessionId) ?? "Saved scan";

    private static string RunDate(WorkerRun run) => (run.StartedAt ?? run.CreatedAt).ToLocalTime().ToString("g");

    private static string RepeatPolicy(string policy) => policy == RepeatCachePolicyNames.RevalidateContent
        ? "Re-read candidate content"
        : "Reuse verified hashes";

    private static string FormatCount(int count, string noun) => $"{count:N0} {noun}{(count == 1 ? string.Empty : "s")}";

    public void Dispose()
    {
        ++_loadGeneration;
        _warningDrilldown.PropertyChanged -= WarningDrilldownPropertyChanged;
        CloseWarnings(restoreFocus: false);
        _warningDrilldown.Dispose();
    }
}

public enum WarningReturnDestination
{
    History,
    ScanProgress,
    ScanSummary,
}
