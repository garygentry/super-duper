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
    private readonly IUiDispatcher _dispatcher;
    private readonly IUserConfirmationService _confirmation;
    private CancellationTokenSource? _selectionCancellation;
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
    private int _selectedTabIndex;
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
        _confirmation = confirmation;
        _dispatcher = dispatcher;

        Sessions = new SessionListViewModel(workerClient, BeginNewSessionAsync);
        Setup = new SessionSetupViewModel(
            workerClient,
            folderPicker,
            confirmation,
            sessionId => Sessions.NamesExcept(sessionId),
            cloudLocations);
        Progress = new ScanProgressViewModel(workerClient, dispatcher);
        History = new RunHistoryViewModel(workerClient);
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
        History.SelectedRunChanged += OnSelectedRunChanged;
        _workerClient.RunProgress += OnRunProgress;
        _workerClient.RunLifecycleChanged += OnRunLifecycleChanged;
        if (_restartableWorkerClient is not null)
        {
            _restartableWorkerClient.UnexpectedExit += OnUnexpectedWorkerExit;
        }

        StartRunCommand = new AsyncRelayCommand(StartRunAsync, () => CanStartRun);
        RestartWorkerCommand = new AsyncRelayCommand(RestartWorkerAsync, () => CanRestartWorker);
        ClearContentErrorCommand = new RelayCommand(() => ContentErrorMessage = null);
    }

    public SessionListViewModel Sessions { get; }

    public SessionSetupViewModel Setup { get; }

    public ScanProgressViewModel Progress { get; }

    public RunHistoryViewModel History { get; }

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
        private set => SetProperty(ref _isLoadingSession, value);
    }

    public string DisplaySessionName
    {
        get => _displaySessionName;
        private set => SetProperty(ref _displaySessionName, value);
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

    public int SelectedTabIndex
    {
        get => _selectedTabIndex;
        set => SetProperty(ref _selectedTabIndex, value);
    }

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

    public bool CanStartRun => IsConnected && IsWorkspaceVisible && !HasActiveRun && Setup.CanStart;

    public bool CanRestartWorker =>
        _restartableWorkerClient is not null
        && ConnectionState is WorkerConnectionState.Failed or WorkerConnectionState.RecoveryRequired;

    public string WorkerExecutablePath => _workerClient.ExecutablePath;

    public string DiagnosticLogPath => _workerClient.DiagnosticLogPath;

    public IAsyncRelayCommand StartRunCommand { get; }

    public IAsyncRelayCommand RestartWorkerCommand { get; }

    public IRelayCommand ClearContentErrorCommand { get; }

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
        _selectionCancellation?.Cancel();
        _selectionCancellation?.Dispose();
        Sessions.SelectionChanged -= OnSessionSelectionChanged;
        Setup.SessionSaved -= OnSessionSaved;
        Setup.SessionDeleted -= OnSessionDeleted;
        Setup.PropertyChanged -= OnSetupPropertyChanged;
        Sessions.PropertyChanged -= OnSessionsPropertyChanged;
        History.SelectedRunChanged -= OnSelectedRunChanged;
        _workerClient.RunProgress -= OnRunProgress;
        _workerClient.RunLifecycleChanged -= OnRunLifecycleChanged;
        if (_restartableWorkerClient is not null)
        {
            _restartableWorkerClient.UnexpectedExit -= OnUnexpectedWorkerExit;
        }
        DuplicateFiles.ReviewRevisionChanged -= OnFileReviewRevisionChanged;
        DuplicateFolders.ReviewRevisionChanged -= OnFolderReviewRevisionChanged;
        Progress.Dispose();
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
        _selectionCancellation?.Cancel();
        _suppressSelection = true;
        Sessions.SelectedSession = null;
        _suppressSelection = false;
        Setup.BeginNew();
        History.Clear();
        Progress.ShowRun(null);
        _ = DuplicateFiles.ShowRunAsync(null);
        _ = DuplicateFolders.ShowRunAsync(null);
        _ = Preflight.ShowRunAsync(null);
        DisplaySessionName = "New session";
        SelectedTabIndex = 0;
        ContentErrorMessage = null;
        IsWorkspaceVisible = true;
        return Task.CompletedTask;
    }

    private Task NavigateToFreshScanAsync()
    {
        SelectedTabIndex = 0;
        FocusTarget = "start-scan";
        FocusRequestVersion++;
        return Task.CompletedTask;
    }

    private async Task SelectSessionAsync(
        SessionListItemViewModel selected,
        CancellationToken cancellationToken = default)
    {
        _selectionCancellation?.Cancel();
        _selectionCancellation?.Dispose();
        _selectionCancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var token = _selectionCancellation.Token;

        IsWorkspaceVisible = true;
        IsLoadingSession = true;
        DisplaySessionName = selected.Name;
        ContentErrorMessage = null;
        try
        {
            var sessionTask = _workerClient.GetSessionAsync(selected.Id, token);
            var historyTask = History.LoadAsync(selected.Id, token);
            var session = await sessionTask;
            await historyTask;
            token.ThrowIfCancellationRequested();

            Setup.Load(session);
            DisplaySessionName = session.Name;
            var latest = History.Runs.FirstOrDefault()?.Run;
            selected.StatusText = latest is null ? "No scans yet" : DisplayFormatting.Status(latest.Status);
            Progress.ShowRun(History.SelectedRun?.Run);
            await DuplicateFiles.ShowRunAsync(History.SelectedRun?.Run, token);
            await DuplicateFolders.ShowRunAsync(History.SelectedRun?.Run, token);
            await Preflight.ShowRunAsync(History.SelectedRun?.Run, token);
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
            ContentErrorMessage = exception.Message;
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
        ContentErrorMessage = null;
        var session = await Setup.EnsureSavedAsync(requireReachableRoot: true);
        if (session is null)
        {
            return;
        }
        try
        {
            await _savedHistoryLoad;
            if (History.SessionId != session.Id)
            {
                await History.LoadAsync(session.Id);
            }
            var run = await _workerClient.StartRunAsync(session.Id);
            SetActiveRun(run);
            History.Upsert(run, select: true);
            Progress.ShowRun(run);
            SelectedTabIndex = 1;
            StatusTitle = $"Scanning {session.Name}";
            StatusDetail = "The scan is running in the Rust worker.";

            var durableRun = await _workerClient.GetRunAsync(run.Id);
            HandleLifecycle(durableRun);
        }
        catch (Exception exception)
        {
            ContentErrorMessage = exception.Message;
        }
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

    private void OnSelectedRunChanged(object? sender, WorkerRun? run)
    {
        Progress.ShowRun(run);
        _ = DuplicateFiles.ShowRunAsync(run);
        _ = DuplicateFolders.ShowRunAsync(run);
        _ = Preflight.ShowRunAsync(run);
    }

    private void OnSetupPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
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

    private void OnRunProgress(object? sender, WorkerRunProgressEventArgs progress) =>
        _dispatcher.Post(() => HandleProgress(progress));

    private void OnRunLifecycleChanged(object? sender, WorkerRunLifecycleEventArgs lifecycle) =>
        _dispatcher.Post(() => HandleLifecycle(lifecycle.Run));

    private void OnUnexpectedWorkerExit(object? sender, WorkerUnexpectedExitEventArgs exit) =>
        _dispatcher.Post(() => HandleUnexpectedWorkerExit(exit));

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
            History.Upsert(unavailable, select: true);
            Progress.ApplyLifecycle(unavailable);
            DuplicateFiles.ApplyLifecycle(unavailable);
            DuplicateFolders.ApplyLifecycle(unavailable);
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
        Progress.ApplyProgress(progress);
        if (progress.RunId == ActiveRunId)
        {
            StatusTitle = DisplayFormatting.Phase(progress.Phase);
            StatusDetail = progress.Message ?? progress.CurrentPath ?? $"{progress.FilesDiscovered:N0} files discovered";
        }
    }

    private void HandleLifecycle(WorkerRun run)
    {
        if (_disposed)
        {
            return;
        }
        History.Upsert(run, select: run.Id == ActiveRunId || run.Status == "running");
        Progress.ApplyLifecycle(run);
        DuplicateFiles.ApplyLifecycle(run);
        DuplicateFolders.ApplyLifecycle(run);
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
        ActiveRunId = run.Id;
        _activeSessionId = run.SessionId;
        Setup.CanMutate = false;
        Sessions.CanMutate = false;
        var session = Sessions.Find(run.SessionId);
        if (session is not null)
        {
            session.StatusText = DisplayFormatting.Status(run.Status);
        }
    }

    private void ShowEmptyState()
    {
        IsWorkspaceVisible = false;
        IsLoadingSession = false;
        DisplaySessionName = "Sessions";
        History.Clear();
        Progress.ShowRun(null);
        _ = DuplicateFiles.ShowRunAsync(null);
        _ = DuplicateFolders.ShowRunAsync(null);
        _ = Preflight.ShowRunAsync(null);
        OnPropertyChanged(nameof(IsEmptyState));
    }

    private async Task LoadSavedSessionHistoryAsync(long sessionId)
    {
        try
        {
            await History.LoadAsync(sessionId);
            Progress.ShowRun(History.SelectedRun?.Run);
            await DuplicateFiles.ShowRunAsync(History.SelectedRun?.Run);
            await DuplicateFolders.ShowRunAsync(History.SelectedRun?.Run);
            await Preflight.ShowRunAsync(History.SelectedRun?.Run);
        }
        catch (Exception exception)
        {
            ContentErrorMessage = exception.Message;
        }
    }
}
