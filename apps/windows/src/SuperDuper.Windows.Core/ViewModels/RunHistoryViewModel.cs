using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class RunHistoryViewModel : ObservableObject, IDisposable
{
    private const int PageSize = 500;
    public const string HashWarningCode = "hash_recoverable_warning";
    public const int WarningPageSize = RunWarningDrilldownViewModel.PageSize;
    public const int WarningCachePageLimit = RunWarningDrilldownViewModel.CachePageLimit;
    private readonly IWorkerClient _workerClient;
    private readonly RunWarningDrilldownViewModel _warningDrilldown;
    private readonly Func<WorkerRun, CancellationToken, Task>? _navigateToDuplicateSet;
    private long? _sessionId;
    private RunListItemViewModel? _selectedRun;
    private bool _isLoading;
    private string? _errorMessage;
    private CancellationTokenSource? _warningNavigationCancellation;
    private long _warningNavigationGeneration;
    private bool _isWarningNavigationPending;
    private string _focusTarget = string.Empty;
    private long _focusRequestVersion;

    public RunHistoryViewModel(
        IWorkerClient workerClient,
        Func<WorkerRun, CancellationToken, Task>? navigateToDuplicateSet = null)
    {
        _workerClient = workerClient;
        _warningDrilldown = new RunWarningDrilldownViewModel(workerClient);
        _warningDrilldown.PropertyChanged += WarningDrilldownPropertyChanged;
        _navigateToDuplicateSet = navigateToDuplicateSet;
        RefreshCommand = new AsyncRelayCommand(RefreshAsync, () => SessionId is not null && !IsLoading);
        OpenWarningsCommand = new AsyncRelayCommand(OpenWarningsAsync, () => CanOpenWarnings);
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
                OnPropertyChanged(nameof(CanOpenWarnings));
                OpenWarningsCommand.NotifyCanExecuteChanged();
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

    public IAsyncRelayCommand OpenWarningsCommand { get; }

    public IAsyncRelayCommand NextWarningPageCommand { get; }

    public IRelayCommand CancelWarningLoadCommand { get; }

    public IAsyncRelayCommand<WorkerRunWarningAggregate> NavigateWarningCommand { get; }

    public IRelayCommand CancelWarningNavigationCommand { get; }

    public IRelayCommand CloseWarningsCommand { get; }

    public event EventHandler<WorkerRun?>? SelectedRunChanged;

    public async Task LoadAsync(long sessionId, CancellationToken cancellationToken = default)
    {
        SessionId = sessionId;
        IsLoading = true;
        ErrorMessage = null;
        try
        {
            var runs = new List<WorkerRun>();
            long offset = 0;
            while (true)
            {
                var page = await _workerClient.ListRunsAsync(sessionId, offset, PageSize, cancellationToken);
                runs.AddRange(page.Runs);
                offset += page.Runs.Count;
                if (offset >= page.Total || page.Runs.Count == 0)
                {
                    break;
                }
            }

            var selectedId = SelectedRun?.Id;
            Runs.Clear();
            foreach (var run in runs)
            {
                Runs.Add(new RunListItemViewModel(run));
            }
            SelectedRun = selectedId is long id
                ? Runs.FirstOrDefault(item => item.Id == id) ?? Runs.FirstOrDefault()
                : Runs.FirstOrDefault();
            OnPropertyChanged(nameof(IsEmpty));
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception)
        {
            ErrorMessage = exception.Message;
        }
        finally
        {
            IsLoading = false;
        }
    }

    public void Clear()
    {
        CloseWarnings(restoreFocus: false);
        SessionId = null;
        Runs.Clear();
        SelectedRun = null;
        ErrorMessage = null;
        OnPropertyChanged(nameof(IsEmpty));
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
            Runs.Insert(0, item);
        }
        else
        {
            item.Update(run);
        }
        NavigateWarningCommand.NotifyCanExecuteChanged();
        if (select)
        {
            SelectedRun = item;
        }
        OnPropertyChanged(nameof(IsEmpty));
    }

    public async Task OpenWarningsForRunAsync(
        WorkerRun run,
        CancellationToken cancellationToken = default)
    {
        Upsert(run, select: true);
        if (SelectedRun?.Id != run.Id)
        {
            throw new InvalidOperationException(
                "The current warning run is not available in the selected session history.");
        }
        await LoadWarningPageAsync(opening: true, cancellationToken);
    }

    private Task RefreshAsync() => SessionId is long sessionId
        ? LoadAsync(sessionId)
        : Task.CompletedTask;

    private Task OpenWarningsAsync() => LoadWarningPageAsync(opening: true);

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
        CancelWarningNavigation(clearFeedback: true);
        _warningDrilldown.Close();
        NavigateWarningCommand.NotifyCanExecuteChanged();
        if (restoreFocus) RequestFocus("history");
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
        OpenWarningsCommand.NotifyCanExecuteChanged();
        NextWarningPageCommand.NotifyCanExecuteChanged();
        CancelWarningLoadCommand.NotifyCanExecuteChanged();
        CloseWarningsCommand.NotifyCanExecuteChanged();
        NavigateWarningCommand.NotifyCanExecuteChanged();
    }

    public void Dispose()
    {
        _warningDrilldown.PropertyChanged -= WarningDrilldownPropertyChanged;
        CloseWarnings(restoreFocus: false);
        _warningDrilldown.Dispose();
    }
}
