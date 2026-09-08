using System.ComponentModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class ShellViewModel : ObservableObject, IDisposable
{
    private readonly IWorkerClient _workerClient;
    private readonly IRestartableWorkerClient? _restartableWorkerClient;
    private readonly IReviewLiveStateWorkerClient? _reviewLiveStateWorkerClient;
    private readonly IUiDispatcher _dispatcher;
    private readonly IUserConfirmationService _confirmation;
    private readonly LatestProgressApplicationGate<WorkerRunProgressEventArgs> _progressGate;
    private CancellationTokenSource? _selectionCancellation;
    private CancellationTokenSource? _startCancellation;
    private long _startGeneration;
    private WorkerConnectionState _connectionState = WorkerConnectionState.Starting;
    private string _statusTitle = "Starting worker";
    private string _statusDetail = "Establishing a private connection to the Super Duper engine.";
    private string? _workerVersion;
    private string? _engineVersion;
    private bool _isWorkspaceVisible;
    private bool _isLoadingSession;
    private string _displaySessionName = "Sessions";
    private string? _contentErrorMessage;
    private long? _activeRunId;
    private long? _activeSessionId;
    private WorkspaceDestination _selectedDestination;
    private WorkerRun? _selectedRun;
    private CancellationTokenSource _workspaceCancellation = new();
    private readonly Dictionary<WorkspaceDestination, Task> _paneLoads = [];
    private long _navigationGeneration;
    private WorkspaceDestination _scanDestination = WorkspaceDestination.ScanSetup;
    private WorkspaceDestination _resultsDestination = WorkspaceDestination.FileResults;
    private WorkspaceDestination _historyDestination = WorkspaceDestination.History;
    private bool _suppressSelection;
    private bool _disposed;
    private Task _savedHistoryLoad = Task.CompletedTask;
    private string _focusTarget = string.Empty;
    private long _focusRequestVersion;

    public ShellViewModel(
        IWorkerClient workerClient,
        IFolderPickerService folderPicker,
        IUserConfirmationService confirmation,
        IUiDispatcher dispatcher,
        IClipboardService clipboard,
        IExplorerService explorer,
        ICloudLocationService? cloudLocations = null,
        IRecycleOperationCapabilityExecutor? recycleOperationExecutor = null,
        IRecycleBinService? recycleBin = null)
    {
        _workerClient = workerClient;
        _restartableWorkerClient = workerClient as IRestartableWorkerClient;
        _reviewLiveStateWorkerClient = workerClient as IReviewLiveStateWorkerClient;
        _confirmation = confirmation;
        _dispatcher = dispatcher;

        _progressGate = new LatestProgressApplicationGate<WorkerRunProgressEventArgs>(
            HandleProgress,
            ScheduleProgressApplicationAsync);

        Sessions = new SessionListViewModel(workerClient, BeginNewSessionAsync);
        Setup = new SessionSetupViewModel(
            workerClient,
            folderPicker,
            confirmation,
            sessionId => Sessions.NamesExcept(sessionId),
            cloudLocations);
        Progress = new ScanProgressViewModel(
            workerClient,
            dispatcher,
            runId => _progressGate.MarkCancelling(runId),
            OpenProgressWarningsAsync);
        Summary = new ScanProgressViewModel(workerClient, dispatcher, openWarnings: OpenProgressWarningsAsync);
        Progress.PropertyChanged += OnProgressPropertyChanged;
        History = new RunHistoryViewModel(workerClient, NavigateToWarningDuplicateSetAsync);
        Performance = new PerformanceViewModel(workerClient);
        DuplicateFiles = new DuplicateFilesViewModel(workerClient, clipboard, explorer);
        DuplicateFolders = new DuplicateFoldersViewModel(workerClient, clipboard, explorer);
        Preflight = new PreflightViewModel(
            workerClient,
            confirmation,
            recycleOperationExecutor,
            clipboard,
            recycleBin,
            NavigateToFreshScanAsync);
        DuplicateFiles.ReviewRevisionChanged += OnFileReviewRevisionChanged;
        DuplicateFolders.ReviewRevisionChanged += OnFolderReviewRevisionChanged;

        Sessions.SelectionChanged += OnSessionSelectionChanged;
        Setup.SessionSaved += OnSessionSaved;
        Setup.SessionDeleted += OnSessionDeleted;
        Setup.PropertyChanged += OnSetupPropertyChanged;
        Sessions.PropertyChanged += OnSessionsPropertyChanged;
        History.SelectedRunChanged += OnHistorySelectionChanged;
        History.PropertyChanged += OnHistoryPropertyChanged;
        _workerClient.RunProgress += OnRunProgress;
        _workerClient.RunLifecycleChanged += OnRunLifecycleChanged;
        if (_restartableWorkerClient is not null)
        {
            _restartableWorkerClient.UnexpectedExit += OnUnexpectedWorkerExit;
        }
        if (_reviewLiveStateWorkerClient is not null)
        {
            _reviewLiveStateWorkerClient.ResultStateChanged += OnResultStateChanged;
        }

        StartRunCommand = new AsyncRelayCommand(StartRunAsync, () => CanStartRun);
        RestartWorkerCommand = new AsyncRelayCommand(RestartWorkerAsync, () => CanRestartWorker);
        ClearContentErrorCommand = new RelayCommand(() => ContentErrorMessage = null);
        OpenScanCommand = new RelayCommand(OpenHighlightedScan, () => IsConnected && History.SelectedRun is not null && !History.IsLoading && !IsLoadingSession);
        ViewProgressCommand = new RelayCommand(() =>
        {
            SelectedDestination = WorkspaceDestination.ScanProgress;
            FocusTarget = "scan-navigation";
            FocusRequestVersion++;
        });
    }

    public SessionListViewModel Sessions { get; }

    public SessionSetupViewModel Setup { get; }

    public ScanProgressViewModel Progress { get; }

    public ScanProgressViewModel Summary { get; }

    public RunHistoryViewModel History { get; }

    public PerformanceViewModel Performance { get; }

    public DuplicateFilesViewModel DuplicateFiles { get; }

    public DuplicateFoldersViewModel DuplicateFolders { get; }

    public PreflightViewModel Preflight { get; }

    public WorkerConnectionState ConnectionState
    {
        get => _connectionState;
        private set
        {
            if (SetProperty(ref _connectionState, value))
            {
                OnPropertyChanged(nameof(IsStarting));
                OnPropertyChanged(nameof(IsConnected));
                OnPropertyChanged(nameof(IsFailed));
                OnPropertyChanged(nameof(IsRecoveryRequired));
                OnPropertyChanged(nameof(IsRecoveryScreenVisible));
                OnPropertyChanged(nameof(IsEmptyState));
                OnPropertyChanged(nameof(CanStartRun));
                OnPropertyChanged(nameof(CanRestartWorker));
                StartRunCommand.NotifyCanExecuteChanged();
                RestartWorkerCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public string StatusTitle
    {
        get => _statusTitle;
        private set => SetProperty(ref _statusTitle, value);
    }

    public string StatusDetail
    {
        get => _statusDetail;
        private set => SetProperty(ref _statusDetail, value);
    }

    public string? WorkerVersion
    {
        get => _workerVersion;
        private set => SetProperty(ref _workerVersion, value);
    }

    public string? EngineVersion
    {
        get => _engineVersion;
        private set => SetProperty(ref _engineVersion, value);
    }

    public bool IsWorkspaceVisible
    {
        get => _isWorkspaceVisible;
        private set
        {
            if (SetProperty(ref _isWorkspaceVisible, value))
            {
                OnPropertyChanged(nameof(IsEmptyState));
            }
        }
    }

    public bool IsLoadingSession
    {
        get => _isLoadingSession;
        private set
        {
            if (SetProperty(ref _isLoadingSession, value))
            {
                OnPropertyChanged(nameof(CanStartRun));
                StartRunCommand.NotifyCanExecuteChanged();
                OnPropertyChanged(nameof(IsSetupAvailable));
                OpenScanCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public string DisplaySessionName
    {
        get => _displaySessionName;
        private set
        {
            if (!SetProperty(ref _displaySessionName, value)) return;
            OnPropertyChanged(nameof(WorkspaceSessionName));
            OnPropertyChanged(nameof(HistoryContext));
            OnPropertyChanged(nameof(StartRunLabel));
        }
    }

    public string? ContentErrorMessage
    {
        get => _contentErrorMessage;
        private set
        {
            if (SetProperty(ref _contentErrorMessage, value))
            {
                OnPropertyChanged(nameof(HasContentError));
            }
        }
    }

    public WorkspaceDestination SelectedDestination
    {
        get => _selectedDestination;
        set
        {
            if (!SetProperty(ref _selectedDestination, value)) return;
            _navigationGeneration++;
            switch (value)
            {
                case WorkspaceDestination.ScanSetup or WorkspaceDestination.ScanProgress or WorkspaceDestination.ScanSummary: _scanDestination = value; break;
                case WorkspaceDestination.FileResults or WorkspaceDestination.FolderResults: _resultsDestination = value; break;
                case WorkspaceDestination.History or WorkspaceDestination.Performance: _historyDestination = value; break;
            }
            OnPropertyChanged(nameof(SelectedArea));
            OnPropertyChanged(nameof(ScanDestination));
            OnPropertyChanged(nameof(ResultsDestination));
            OnPropertyChanged(nameof(HistoryDestination));
            _ = EnsurePaneAsync();
        }
    }

    public WorkspaceArea SelectedArea
    {
        get => SelectedDestination switch
        {
            WorkspaceDestination.ScanSetup or WorkspaceDestination.ScanProgress or WorkspaceDestination.ScanSummary => WorkspaceArea.Scan,
            WorkspaceDestination.FileResults or WorkspaceDestination.FolderResults => WorkspaceArea.Results,
            WorkspaceDestination.Review => WorkspaceArea.Review,
            _ => WorkspaceArea.History,
        };
        set => SelectedDestination = value switch
        {
            WorkspaceArea.Scan => _scanDestination,
            WorkspaceArea.Results => _resultsDestination,
            WorkspaceArea.Review => WorkspaceDestination.Review,
            _ => _historyDestination,
        };
    }

    public WorkspaceDestination ScanDestination
    {
        get => _scanDestination;
        set { _scanDestination = value; if (SelectedArea == WorkspaceArea.Scan) SelectedDestination = value; }
    }
    public WorkspaceDestination ResultsDestination
    {
        get => _resultsDestination;
        set { _resultsDestination = value; if (SelectedArea == WorkspaceArea.Results) SelectedDestination = value; }
    }
    public WorkspaceDestination HistoryDestination
    {
        get => _historyDestination;
        set { _historyDestination = value; if (SelectedArea == WorkspaceArea.History) SelectedDestination = value; }
    }

    public WorkerRun? SelectedRun => _selectedRun;
    public string WorkspaceSessionName => SelectedRun is { } run
        ? Sessions.Find(run.SessionId)?.Name ?? "Saved scan" : DisplaySessionName;
    public string StartRunLabel => $"Start scan: {DisplaySessionName}";
    public string HistoryContext => $"History: {DisplaySessionName} \u00b7 Highlight a row, then Open scan to change the workspace.";

    public string SelectedScanContext => SelectedRun is { } run
        ? $"Scan {run.Id} · {(run.StartedAt ?? run.CreatedAt).ToLocalTime():g} · {DisplayFormatting.Status(run.Status)} · {run.Parameters.Roots.Count} locations"
        : History.IsLoading ? "Loading scan history…"
        : History.HasError ? "Scan history unavailable" : "No scan yet";

    public string ActiveScanName => Sessions.Find(_activeSessionId ?? -1)?.Name ?? "Active scan";

    public string ProgressScanContext => Progress.Run is { } run
        ? $"{Sessions.Find(run.SessionId)?.Name ?? "Saved scan"} · Scan {run.Id} · {(run.StartedAt ?? run.CreatedAt).ToLocalTime():g} · {DisplayFormatting.Status(run.Status)}"
        : "No scan to monitor";

    public bool IsSetupAvailable => !IsLoadingSession
        && (Setup.IsNew ? Sessions.SelectedSession is null : Setup.SessionId == Sessions.SelectedSession?.Id);

    public string FocusTarget
    {
        get => _focusTarget;
        private set => SetProperty(ref _focusTarget, value);
    }

    public long FocusRequestVersion
    {
        get => _focusRequestVersion;
        private set => SetProperty(ref _focusRequestVersion, value);
    }

    public bool IsStarting => ConnectionState == WorkerConnectionState.Starting;

    public bool IsConnected => ConnectionState == WorkerConnectionState.Connected;

    public bool IsFailed => ConnectionState == WorkerConnectionState.Failed;

    public bool IsRecoveryRequired => ConnectionState == WorkerConnectionState.RecoveryRequired;

    public bool IsRecoveryScreenVisible => IsFailed || IsRecoveryRequired;

    public bool IsEmptyState => IsConnected && !IsWorkspaceVisible && !Sessions.IsLoading;

    public bool HasContentError => !string.IsNullOrWhiteSpace(ContentErrorMessage);

    public bool HasActiveRun => ActiveRunId is not null;

    public long? ActiveRunId
    {
        get => _activeRunId;
        private set
        {
            if (SetProperty(ref _activeRunId, value))
            {
                OnPropertyChanged(nameof(HasActiveRun));
                OnPropertyChanged(nameof(CanStartRun));
                StartRunCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public bool CanStartRun => IsConnected && IsWorkspaceVisible && IsSetupAvailable
        && !History.IsLoading && !HasActiveRun && Setup.CanStart;

    public bool CanRestartWorker =>
        _restartableWorkerClient is not null
        && ConnectionState is WorkerConnectionState.Failed or WorkerConnectionState.RecoveryRequired;

    public string WorkerExecutablePath => _workerClient.ExecutablePath;

    public string DiagnosticLogPath => _workerClient.DiagnosticLogPath;

    public IAsyncRelayCommand StartRunCommand { get; }

    public IAsyncRelayCommand RestartWorkerCommand { get; }

    public IRelayCommand ClearContentErrorCommand { get; }

    public IRelayCommand ViewProgressCommand { get; }

    public IRelayCommand OpenScanCommand { get; }

    public async Task InitializeAsync(CancellationToken cancellationToken = default)
    {
        ConnectionState = WorkerConnectionState.Starting;
        StatusTitle = "Starting worker";
        StatusDetail = "Establishing a private connection to the Super Duper engine.";
        WorkerVersion = null;
        EngineVersion = null;

        try
        {
            var hello = await _workerClient.ConnectAsync(cancellationToken);
            WorkerVersion = hello.WorkerVersion;
            EngineVersion = hello.EngineVersion;
            StatusTitle = "Worker connected";
            StatusDetail = $"Protocol {hello.ProtocolVersion} · Worker {hello.WorkerVersion} · Engine {hello.EngineVersion}";
            ConnectionState = WorkerConnectionState.Connected;

            _suppressSelection = true;
            try
            {
                await Sessions.LoadAsync(cancellationToken);
            }
            finally
            {
                _suppressSelection = false;
            }

            if (Sessions.HasError)
            {
                ContentErrorMessage = Sessions.ErrorMessage;
            }
            if (Sessions.SelectedSession is { } selected)
            {
                await SelectSessionAsync(selected, cancellationToken);
            }
            else
            {
                ShowEmptyState();
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception)
        {
            StatusTitle = "Worker connection failed";
            StatusDetail = exception.Message;
            ConnectionState = WorkerConnectionState.Failed;
        }
    }

    public async Task<bool> ConfirmCancelAndExitAsync(CancellationToken cancellationToken = default)
    {
        if (ActiveRunId is not long runId)
        {
            if (Preflight.Preflight is not { } preflight || !Preflight.IsRunning)
            {
                return true;
            }
            var preflightConfirmed = await _confirmation.ConfirmAsync(
                "Cancel preflight and exit?",
                "Preflight validation is still running. Cancel it and close Super Duper? No files will be deleted.",
                cancellationToken);
            if (!preflightConfirmed)
            {
                return false;
            }
            try
            {
                await _workerClient.CancelPreflightAsync(preflight.Id, cancellationToken);
            }
            catch
            {
                // Closing stdin remains the bounded last-resort cancellation path during disposal.
            }
            return true;
        }
        var sessionName = Sessions.Find(_activeSessionId ?? -1)?.Name ?? "the active session";
        var confirmed = await _confirmation.ConfirmAsync(
            "Cancel scan and exit?",
            $"'{sessionName}' is still scanning. Cancel the scan and close Super Duper?",
            cancellationToken);
        if (!confirmed)
        {
            return false;
        }
        try
        {
            var cancelling = await _workerClient.CancelRunAsync(runId, cancellationToken);
            HandleLifecycle(cancelling);
        }
        catch
        {
            // Closing stdin remains the bounded last-resort cancellation path during disposal.
        }
        return true;
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }
        _disposed = true;
        _workspaceCancellation.Cancel();
        _workspaceCancellation.Dispose();
        _selectionCancellation?.Cancel();
        _selectionCancellation?.Dispose();
        CancelPendingStart();
        Sessions.SelectionChanged -= OnSessionSelectionChanged;
        Setup.SessionSaved -= OnSessionSaved;
        Setup.SessionDeleted -= OnSessionDeleted;
        Setup.PropertyChanged -= OnSetupPropertyChanged;
        Sessions.PropertyChanged -= OnSessionsPropertyChanged;
        History.SelectedRunChanged -= OnHistorySelectionChanged;
        History.PropertyChanged -= OnHistoryPropertyChanged;
        _workerClient.RunProgress -= OnRunProgress;
        _workerClient.RunLifecycleChanged -= OnRunLifecycleChanged;
        if (_reviewLiveStateWorkerClient is not null)
        {
            _reviewLiveStateWorkerClient.ResultStateChanged -= OnResultStateChanged;
            _reviewLiveStateWorkerClient.ObserveReviewLiveState(null);
        }
        if (_restartableWorkerClient is not null)
        {
            _restartableWorkerClient.UnexpectedExit -= OnUnexpectedWorkerExit;
        }
        DuplicateFiles.ReviewRevisionChanged -= OnFileReviewRevisionChanged;
        DuplicateFolders.ReviewRevisionChanged -= OnFolderReviewRevisionChanged;
        Progress.Dispose();
        Summary.Dispose();
        Progress.PropertyChanged -= OnProgressPropertyChanged;
        _progressGate.Dispose();
        History.Dispose();
        Performance.Dispose();
        DuplicateFiles.Dispose();
        DuplicateFolders.Dispose();
        Preflight.Dispose();
    }

    private void OnFileReviewRevisionChanged(long runId, long revision)
    {
        _ = DuplicateFolders.RefreshReviewRevisionAsync(runId, revision);
        _ = Preflight.RefreshReviewRevisionAsync(runId, revision);
    }

    private void OnFolderReviewRevisionChanged(long runId, long revision)
    {
        _ = DuplicateFiles.RefreshReviewRevisionAsync(runId, revision);
        _ = Preflight.RefreshReviewRevisionAsync(runId, revision);
    }

    private Task BeginNewSessionAsync()
    {
        CancelPendingStart();
        _selectionCancellation?.Cancel();
        _suppressSelection = true;
        Sessions.SelectedSession = null;
        _suppressSelection = false;
        Setup.BeginNew();
        History.Clear();
        SetWorkspaceRun(null);
        if (!HasActiveRun) Progress.ShowRun(null);
        DisplaySessionName = "New session";
        SelectedDestination = WorkspaceDestination.ScanSetup;
        IsLoadingSession = false;
        ContentErrorMessage = null;
        IsWorkspaceVisible = true;
        return Task.CompletedTask;
    }

    private Task NavigateToFreshScanAsync()
    {
        SelectedDestination = WorkspaceDestination.ScanSetup;
        FocusTarget = "start-scan";
        FocusRequestVersion++;
        return Task.CompletedTask;
    }

    private async Task NavigateToWarningDuplicateSetAsync(
        WorkerRun target,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (History.SelectedRun?.Id != target.Id || History.SessionId != target.SessionId)
        {
            cancellationToken.ThrowIfCancellationRequested();
            throw new InvalidOperationException("The warning run is no longer the selected history context.");
        }
        SetWorkspaceRun(target, loadPane: false);
        SelectedDestination = WorkspaceDestination.FileResults;
        var navigation = _navigationGeneration;
        await EnsurePaneAsync();
        if (navigation != _navigationGeneration || SelectedRun?.Id != target.Id) return;
        cancellationToken.ThrowIfCancellationRequested();
        if (History.SelectedRun?.Id != target.Id || DuplicateFiles.Run?.Id != target.Id)
        {
            cancellationToken.ThrowIfCancellationRequested();
            throw new InvalidOperationException("The immutable duplicate-file target did not remain selected.");
        }

        SelectedDestination = WorkspaceDestination.FileResults;
        FocusTarget = "duplicate-file-groups";
        FocusRequestVersion++;
    }

    private async Task OpenProgressWarningsAsync(
        WorkerRun run,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if ((Progress.Run?.Id != run.Id && Summary.Run?.Id != run.Id) || run.WarningCount <= 0)
        {
            return;
        }

        var navigation = _navigationGeneration;
        if (History.SessionId != run.SessionId && Sessions.Find(run.SessionId) is { } session)
        {
            _suppressSelection = true;
            Sessions.SelectedSession = session;
            _suppressSelection = false;
            var selection = SelectSessionAsync(session, cancellationToken, preserveWorkspace: true);
            var selectionCancellation = _selectionCancellation;
            await selection;
            if (_selectionCancellation != selectionCancellation || selectionCancellation?.IsCancellationRequested == true) return;
        }
        cancellationToken.ThrowIfCancellationRequested();
        if (navigation != _navigationGeneration) return;
        SelectedDestination = WorkspaceDestination.History;
        await History.OpenWarningsForRunAsync(run, cancellationToken);
    }

    private async Task SelectSessionAsync(
        SessionListItemViewModel selected,
        CancellationToken cancellationToken = default,
        bool preserveWorkspace = false)
    {
        CancelPendingStart();
        _selectionCancellation?.Cancel();
        _selectionCancellation?.Dispose();
        _selectionCancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var token = _selectionCancellation.Token;

        IsWorkspaceVisible = true;
        IsLoadingSession = true;
        DisplaySessionName = selected.Name;
        ContentErrorMessage = null;
        History.Clear();
        if (!preserveWorkspace) SetWorkspaceRun(null);
        try
        {
            var sessionTask = _workerClient.GetSessionAsync(selected.Id, token);
            var historyTask = History.LoadAsync(selected.Id, token);
            var session = await sessionTask;
            token.ThrowIfCancellationRequested();
            Setup.Load(session);
            DisplaySessionName = session.Name;
            IsLoadingSession = false;
            await historyTask;
            token.ThrowIfCancellationRequested();
            var latest = History.Runs.FirstOrDefault()?.Run;
            if (!preserveWorkspace) SetWorkspaceRun(latest);
            selected.StatusText = latest is null ? "No scans yet" : DisplayFormatting.Status(latest.Status);
            if (latest?.Status is "pending" or "running" or "cancelling")
            {
                SetActiveRun(latest);
            }
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (!token.IsCancellationRequested && !_disposed)
            {
                ContentErrorMessage = exception.Message;
            }
        }
        finally
        {
            if (!token.IsCancellationRequested)
            {
                IsLoadingSession = false;
            }
        }
    }

    private async Task StartRunAsync()
    {
        CancelPendingStart();
        var generation = ++_startGeneration;
        _startCancellation = new CancellationTokenSource();
        var token = _startCancellation.Token;
        var repeatCachePolicy = Setup.RepeatCachePolicy;
        ContentErrorMessage = null;
        try
        {
            var session = await Setup.EnsureSavedAsync(requireReachableRoot: true, token);
            if (session is null || !IsCurrentStart(generation, token))
            {
                return;
            }
            await _savedHistoryLoad;
            token.ThrowIfCancellationRequested();
            if (History.SessionId != session.Id)
            {
                await History.LoadAsync(session.Id, token);
            }
            token.ThrowIfCancellationRequested();
            var run = await _workerClient.StartRunAsync(session.Id, repeatCachePolicy, token);
            if (!IsCurrentStart(generation, token))
            {
                return;
            }
            SetActiveRun(run);
            History.Upsert(run, select: true);
            SetWorkspaceRun(run);
            Progress.ShowRun(run);
            SelectedDestination = WorkspaceDestination.ScanProgress;
            StatusTitle = $"Scanning {session.Name}";
            StatusDetail = "The scan is running in the Rust worker.";

            var durableRun = await _workerClient.GetRunAsync(run.Id, token);
            if (!IsCurrentStart(generation, token))
            {
                return;
            }
            HandleLifecycle(durableRun);
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (IsCurrentStart(generation, token))
            {
                ContentErrorMessage = exception.Message;
            }
        }
        finally
        {
            if (generation == _startGeneration)
            {
                _startCancellation?.Dispose();
                _startCancellation = null;
            }
        }
    }

    private bool IsCurrentStart(long generation, CancellationToken token) =>
        !_disposed && !token.IsCancellationRequested && generation == _startGeneration;

    private void CancelPendingStart()
    {
        _startGeneration++;
        _startCancellation?.Cancel();
        _startCancellation?.Dispose();
        _startCancellation = null;
    }

    private async Task RestartWorkerAsync()
    {
        if (_restartableWorkerClient is null)
        {
            return;
        }

        var selectedSessionId = Sessions.SelectedSession?.Id;
        ConnectionState = WorkerConnectionState.Starting;
        StatusTitle = "Restarting worker";
        StatusDetail = "Starting a fresh private worker and reconciling interrupted scan state.";
        ContentErrorMessage = null;
        try
        {
            var hello = await _restartableWorkerClient.RestartAsync();
            WorkerVersion = hello.WorkerVersion;
            EngineVersion = hello.EngineVersion;
            Setup.CanMutate = true;
            Sessions.CanMutate = true;

            _suppressSelection = true;
            try
            {
                await Sessions.LoadAsync();
                if (selectedSessionId is long sessionId && Sessions.Find(sessionId) is { } selected)
                {
                    Sessions.SelectedSession = selected;
                }
            }
            finally
            {
                _suppressSelection = false;
            }

            ConnectionState = WorkerConnectionState.Connected;
            StatusTitle = "Worker recovered";
            StatusDetail = $"Protocol {hello.ProtocolVersion} · Interrupted work reconciled · Ready for a new scan";
            if (Sessions.SelectedSession is { } session)
            {
                await SelectSessionAsync(session);
            }
            else
            {
                ShowEmptyState();
            }
        }
        catch (Exception exception)
        {
            StatusTitle = "Worker restart failed";
            StatusDetail = exception.Message;
            ConnectionState = WorkerConnectionState.Failed;
        }
    }

    private void OnSessionSelectionChanged(object? sender, SessionListItemViewModel? selected)
    {
        if (_suppressSelection)
        {
            return;
        }
        if (selected is null)
        {
            ShowEmptyState();
            return;
        }
        _ = SelectSessionAsync(selected);
    }

    private void OnSessionSaved(object? sender, WorkerSessionDefinition session)
    {
        DisplaySessionName = session.Name;
        _suppressSelection = true;
        Sessions.Upsert(session, select: true);
        _suppressSelection = false;
        IsWorkspaceVisible = true;
        if (History.SessionId != session.Id)
        {
            _savedHistoryLoad = LoadSavedSessionHistoryAsync(session.Id);
        }
        OnPropertyChanged(nameof(CanStartRun));
        StartRunCommand.NotifyCanExecuteChanged();
    }

    private void OnSessionDeleted(object? sender, long sessionId)
    {
        Sessions.Remove(sessionId);
        if (Sessions.SelectedSession is null)
        {
            ShowEmptyState();
        }
    }

    private void OnHistorySelectionChanged(object? sender, WorkerRun? run) =>
        OpenScanCommand.NotifyCanExecuteChanged();

    private void OpenHighlightedScan()
    {
        if (History.SelectedRun?.Run is not { } run) return;
        SetWorkspaceRun(run, loadPane: false);
        SelectedDestination = run.Status == "completed"
            ? WorkspaceDestination.FileResults
            : run.Status is "pending" or "running" or "cancelling"
                ? WorkspaceDestination.ScanProgress : WorkspaceDestination.ScanSummary;
        _ = EnsurePaneAsync();
        FocusTarget = run.Status == "completed" ? "results-navigation" : "scan-navigation";
        FocusRequestVersion++;
    }

    private void SetWorkspaceRun(WorkerRun? run, bool loadPane = true)
    {
        var changed = _selectedRun?.Id != run?.Id || _selectedRun?.Status != run?.Status;
        _selectedRun = run;
        Summary.ShowRun(run?.Status is "pending" or "running" or "cancelling" ? null : run);
        OnPropertyChanged(nameof(SelectedRun));
        OnPropertyChanged(nameof(SelectedScanContext));
        OnPropertyChanged(nameof(WorkspaceSessionName));
        if (!changed) return;
        _workspaceCancellation.Cancel();
        _workspaceCancellation.Dispose();
        _workspaceCancellation = new();
        _paneLoads.Clear();
        _reviewLiveStateWorkerClient?.ObserveReviewLiveState(null);
        if (!HasActiveRun || Progress.Run?.Id != ActiveRunId) Progress.ShowRun(run);
        _ = Performance.ShowRunAsync(null);
        _ = DuplicateFiles.ShowRunAsync(null);
        _ = DuplicateFolders.ShowRunAsync(null);
        _ = Preflight.ShowRunAsync(null);
        if (loadPane) _ = EnsurePaneAsync();
    }

    // Only one set of bounded pane state is retained, for the opened workspace run.
    private Task EnsurePaneAsync()
    {
        if (SelectedRun is not { } run) return Task.CompletedTask;
        if (_paneLoads.TryGetValue(SelectedDestination, out var pending)) return pending;
        var token = _workspaceCancellation.Token;
        var load = SelectedDestination switch
        {
            WorkspaceDestination.FileResults => DuplicateFiles.ShowRunAsync(run, token),
            WorkspaceDestination.FolderResults => DuplicateFolders.ShowRunAsync(run, token),
            WorkspaceDestination.Review => Preflight.ShowRunAsync(run, token),
            WorkspaceDestination.Performance => Performance.ShowRunAsync(run, token),
            _ => Task.CompletedTask,
        };
        if (SelectedDestination == WorkspaceDestination.FileResults)
            _reviewLiveStateWorkerClient?.ObserveReviewLiveState(run);
        _paneLoads[SelectedDestination] = load;
        return load;
    }

    private void OnHistoryPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(RunHistoryViewModel.IsLoading) or nameof(RunHistoryViewModel.ErrorMessage))
        {
            OnPropertyChanged(nameof(SelectedScanContext));
            OnPropertyChanged(nameof(CanStartRun));
            StartRunCommand.NotifyCanExecuteChanged();
            OpenScanCommand.NotifyCanExecuteChanged();
        }
    }

    private void OnProgressPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(ScanProgressViewModel.Run))
        {
            OnPropertyChanged(nameof(ProgressScanContext));
        }
    }

    private void OnSetupPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(SessionSetupViewModel.SessionId))
        {
            OnPropertyChanged(nameof(IsSetupAvailable));
        }
        if (e.PropertyName is nameof(SessionSetupViewModel.CanStart)
            or nameof(SessionSetupViewModel.IsBusy)
            or nameof(SessionSetupViewModel.Name))
        {
            if (Setup.IsNew)
            {
                DisplaySessionName = string.IsNullOrWhiteSpace(Setup.Name) ? "New session" : Setup.Name.Trim();
            }
            OnPropertyChanged(nameof(CanStartRun));
            StartRunCommand.NotifyCanExecuteChanged();
        }
    }

    private void OnSessionsPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(SessionListViewModel.IsLoading) or nameof(SessionListViewModel.IsEmpty))
        {
            OnPropertyChanged(nameof(IsEmptyState));
        }
    }

    private void OnRunProgress(object? sender, WorkerRunProgressEventArgs progress)
    {
        if (_disposed
            || !WorkerProgressContract.TryValidate(progress, out _)
            || !WorkerProgressContract.TryGetCumulativeValues(
                progress,
                out var counters,
                out _))
        {
            return;
        }
        _progressGate.Offer(new ProgressApplicationEnvelope<WorkerRunProgressEventArgs>(
            progress.RunId,
            progress.Sequence,
            progress.Progress.Revision,
            progress.Status,
            counters,
            progress));
    }

    private void OnRunLifecycleChanged(object? sender, WorkerRunLifecycleEventArgs lifecycle)
    {
        ObserveProgressLifecycle(lifecycle.Run);
        _dispatcher.Post(() => HandleLifecycle(lifecycle.Run, progressLifecycleObserved: true));
    }

    private void OnResultStateChanged(object? sender, WorkerResultStateChangedEventArgs stateChanged) =>
        _dispatcher.Post(() => DuplicateFiles.ApplyLiveStateChanged(stateChanged));

    private void OnUnexpectedWorkerExit(object? sender, WorkerUnexpectedExitEventArgs exit)
    {
        _progressGate.Reset();
        _dispatcher.Post(() => HandleUnexpectedWorkerExit(exit));
    }

    private void HandleUnexpectedWorkerExit(WorkerUnexpectedExitEventArgs exit)
    {
        if (_disposed)
        {
            return;
        }

        ConnectionState = WorkerConnectionState.RecoveryRequired;
        StatusTitle = "Worker exited unexpectedly";
        StatusDetail = $"{exit.Message} Restart the worker to reconcile interrupted work. Diagnostics: {exit.DiagnosticLogPath}";
        ContentErrorMessage = null;

        if (Progress.Run is { } run && run.Status is "pending" or "running" or "cancelling")
        {
            var unavailable = run with
            {
                Status = "interrupted",
                CompletedAt = DateTimeOffset.UtcNow,
                ErrorMessage = "The worker exited before this run finished. Restart the worker to reconcile durable state.",
            };
            History.Upsert(unavailable, select: false);
            if (SelectedRun?.Id == unavailable.Id) SetWorkspaceRun(unavailable);
            OnPropertyChanged(nameof(SelectedScanContext));
            Progress.ApplyLifecycle(unavailable);

            var session = Sessions.Find(unavailable.SessionId);
            if (session is not null)
            {
                session.StatusText = "Recovery required";
            }
        }

        Setup.CanMutate = false;
        Sessions.CanMutate = false;
        ActiveRunId = null;
        _activeSessionId = null;
    }

    private void HandleProgress(WorkerRunProgressEventArgs progress)
    {
        if (_disposed)
        {
            return;
        }
        if (!Progress.ApplyProgress(progress))
        {
            return;
        }
        Performance.ObserveProgress(progress.RunId, progress.Sequence);
        if (progress.RunId == ActiveRunId)
        {
            StatusTitle = DisplayFormatting.Phase(progress.Phase);
            StatusDetail = progress.Message ?? progress.CurrentPath ?? $"{progress.FilesDiscovered:N0} files discovered";
        }
    }

    private void HandleLifecycle(WorkerRun run, bool progressLifecycleObserved = false)
    {
        if (_disposed)
        {
            return;
        }
        if (!progressLifecycleObserved)
        {
            ObserveProgressLifecycle(run);
        }
        History.Upsert(run, select: false);
        if (SelectedRun?.Id == run.Id) SetWorkspaceRun(run);
        OnPropertyChanged(nameof(SelectedScanContext));
        Performance.ObserveLifecycle(run);
        Progress.ApplyLifecycle(run);

        var session = Sessions.Find(run.SessionId);
        if (session is not null)
        {
            session.StatusText = DisplayFormatting.Status(run.Status);
        }
        if (run.Status is "pending" or "running" or "cancelling")
        {
            SetActiveRun(run);
        }
        else if (ActiveRunId == run.Id)
        {
            Setup.CanMutate = true;
            Sessions.CanMutate = true;
            ActiveRunId = null;
            _activeSessionId = null;
            StatusTitle = run.Status == "completed" ? "Scan complete" : DisplayFormatting.Status(run.Status);
            StatusDetail = run.ErrorMessage
                ?? $"{run.FilesDiscovered:N0} files · {run.DuplicateFileGroups:N0} duplicate groups";
            OnPropertyChanged(nameof(CanStartRun));
            StartRunCommand.NotifyCanExecuteChanged();
        }
    }

    private void SetActiveRun(WorkerRun run)
    {
        _progressGate.BeginRun(run.Id);
        if (run.Status == "cancelling")
        {
            _progressGate.MarkCancelling(run.Id);
        }
        ActiveRunId = run.Id;
        _activeSessionId = run.SessionId;
        OnPropertyChanged(nameof(ActiveScanName));
        if (Progress.Run?.Id != run.Id) Progress.ShowRun(run);
        Setup.CanMutate = false;
        Sessions.CanMutate = false;
        var session = Sessions.Find(run.SessionId);
        if (session is not null)
        {
            session.StatusText = DisplayFormatting.Status(run.Status);
        }
    }

    private void ObserveProgressLifecycle(WorkerRun run)
    {
        if (run.Status is "pending" or "running" or "cancelling")
        {
            _progressGate.BeginRun(run.Id);
            if (run.Status == "cancelling")
            {
                _progressGate.MarkCancelling(run.Id);
            }
        }
        else
        {
            _progressGate.MarkTerminal(run.Id);
        }
    }

    private async Task ScheduleProgressApplicationAsync(
        TimeSpan delay,
        CancellationToken cancellationToken,
        Action callback)
    {
        await Task.Delay(delay, cancellationToken).ConfigureAwait(false);
        var completion = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        try
        {
            _dispatcher.Post(() =>
            {
                if (cancellationToken.IsCancellationRequested)
                {
                    completion.TrySetCanceled(cancellationToken);
                    return;
                }
                try
                {
                    callback();
                    completion.TrySetResult();
                }
                catch (Exception exception)
                {
                    completion.TrySetException(exception);
                }
            });
        }
        catch (Exception exception)
        {
            completion.TrySetException(exception);
        }
        await completion.Task.WaitAsync(cancellationToken).ConfigureAwait(false);
    }

    private void ShowEmptyState()
    {
        CancelPendingStart();
        _selectionCancellation?.Cancel();
        IsWorkspaceVisible = false;
        IsLoadingSession = false;
        DisplaySessionName = "Sessions";
        History.Clear();
        SetWorkspaceRun(null);
        if (!HasActiveRun) Progress.ShowRun(null);
        OnPropertyChanged(nameof(IsEmptyState));
    }

    private async Task LoadSavedSessionHistoryAsync(long sessionId)
    {
        try
        {
            await History.LoadAsync(sessionId);
        }
        catch (Exception exception)
        {
            ContentErrorMessage = exception.Message;
        }
    }
}
