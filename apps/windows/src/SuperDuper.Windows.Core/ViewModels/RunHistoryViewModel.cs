using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class RunHistoryViewModel : ObservableObject, IDisposable
{
    private const int PageSize = 500;
    public const string HashWarningCode = "hash_recoverable_warning";
    public const int WarningPageSize = 25;
    public const int WarningCachePageLimit = 5;
    private readonly IWorkerClient _workerClient;
    private readonly Func<WorkerRun, CancellationToken, Task>? _navigateToDuplicateSet;
    private long? _sessionId;
    private RunListItemViewModel? _selectedRun;
    private bool _isLoading;
    private string? _errorMessage;
    private CancellationTokenSource? _warningCancellation;
    private long _warningGeneration;
    private bool _isWarningDrilldownOpen;
    private bool _isWarningLoading;
    private string? _warningErrorMessage;
    private string? _warningStatusMessage;
    private CancellationTokenSource? _warningNavigationCancellation;
    private long _warningNavigationGeneration;
    private bool _isWarningNavigationPending;
    private long _warningErrorAnnouncementVersion;
    private string? _nextWarningCursor;
    private long _warningAnnouncementVersion;
    private string _focusTarget = string.Empty;
    private long _focusRequestVersion;
    private RunWarningSortField _warningSortField = RunWarningSortField.OccurrenceCount;
    private WorkerSortDirection _warningSortDirection = WorkerSortDirection.Descending;
    private readonly Dictionary<string, WorkerRunWarningPage> _warningCache = [];
    private readonly Queue<string> _warningCacheOrder = [];

    public RunHistoryViewModel(
        IWorkerClient workerClient,
        Func<WorkerRun, CancellationToken, Task>? navigateToDuplicateSet = null)
    {
        _workerClient = workerClient;
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
    }

    public ObservableCollection<RunListItemViewModel> Runs { get; } = [];

    public ObservableCollection<WorkerRunWarningAggregate> Warnings { get; } = [];

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

    public bool IsWarningDrilldownOpen
    {
        get => _isWarningDrilldownOpen;
        private set
        {
            if (SetProperty(ref _isWarningDrilldownOpen, value))
            {
                OnPropertyChanged(nameof(CanOpenWarnings));
                OpenWarningsCommand.NotifyCanExecuteChanged();
                CloseWarningsCommand.NotifyCanExecuteChanged();
                NavigateWarningCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public bool IsWarningLoading
    {
        get => _isWarningLoading;
        private set
        {
            if (SetProperty(ref _isWarningLoading, value))
            {
                OnPropertyChanged(nameof(CanOpenWarnings));
                OnPropertyChanged(nameof(CanLoadNextWarningPage));
                OpenWarningsCommand.NotifyCanExecuteChanged();
                NextWarningPageCommand.NotifyCanExecuteChanged();
                CancelWarningLoadCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public string? WarningErrorMessage
    {
        get => _warningErrorMessage;
        private set { if (SetProperty(ref _warningErrorMessage, value)) OnPropertyChanged(nameof(HasWarningError)); }
    }

    public string? WarningStatusMessage
    {
        get => _warningStatusMessage;
        private set => SetProperty(ref _warningStatusMessage, value);
    }

    public bool HasWarningError => !string.IsNullOrWhiteSpace(WarningErrorMessage);

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

    public bool CanLoadNextWarningPage => IsWarningDrilldownOpen && !IsWarningLoading && _nextWarningCursor is not null;

    public RunWarningSortField WarningSortField => _warningSortField;

    public WorkerSortDirection WarningSortDirection => _warningSortDirection;

    public long WarningAnnouncementVersion
    {
        get => _warningAnnouncementVersion;
        private set => SetProperty(ref _warningAnnouncementVersion, value);
    }

    public long WarningErrorAnnouncementVersion
    {
        get => _warningErrorAnnouncementVersion;
        private set => SetProperty(ref _warningErrorAnnouncementVersion, value);
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
        if (_warningSortField == field && _warningSortDirection == direction)
        {
            return;
        }
        _warningCancellation?.Cancel();
        CancelWarningNavigation(clearFeedback: true);
        _warningSortField = field;
        _warningSortDirection = direction;
        OnPropertyChanged(nameof(WarningSortField));
        OnPropertyChanged(nameof(WarningSortDirection));
        _nextWarningCursor = null;
        _warningCache.Clear();
        _warningCacheOrder.Clear();
        if (IsWarningDrilldownOpen)
        {
            await LoadWarningPageAsync(null, opening: false);
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

    private Task RefreshAsync() => SessionId is long sessionId
        ? LoadAsync(sessionId)
        : Task.CompletedTask;

    private Task OpenWarningsAsync() => LoadWarningPageAsync(null, opening: true);

    private Task NextWarningPageAsync() => _nextWarningCursor is { } cursor
        ? LoadWarningPageAsync(cursor, opening: false)
        : Task.CompletedTask;

    private async Task LoadWarningPageAsync(string? cursor, bool opening)
    {
        var run = SelectedRun?.Run;
        if (run is null || run.WarningCount <= 0)
        {
            return;
        }
        _warningCancellation?.Cancel();
        CancelWarningNavigation(clearFeedback: true);
        _warningCancellation?.Dispose();
        _warningCancellation = new CancellationTokenSource();
        var token = _warningCancellation.Token;
        var generation = ++_warningGeneration;
        IsWarningLoading = true;
        WarningErrorMessage = null;
        try
        {
            var key = $"{_warningSortField}|{_warningSortDirection}|{cursor ?? string.Empty}";
            if (!_warningCache.TryGetValue(key, out var page))
            {
                page = await _workerClient.GetRunWarningsAsync(
                    new RunWarningQuery(
                        run.Id,
                        WarningPageSize,
                        _warningSortField,
                        _warningSortDirection,
                        cursor),
                    token);
                if (page.ExecutorEnabled
                    || page.WarningCount != page.AccountedWarningCount
                    || page.Warnings.Count > WarningPageSize
                    || page.Total < page.Warnings.Count
                    || page.Warnings.Any(warning => warning.RunId != run.Id))
                {
                    throw new InvalidOperationException("The worker returned an unsafe, unbounded, or incomplete warning accounting page.");
                }
                CacheWarningPage(key, page);
            }
            if (token.IsCancellationRequested || generation != _warningGeneration || SelectedRun?.Id != run.Id)
            {
                return;
            }
            Warnings.Clear();
            foreach (var warning in page.Warnings)
            {
                Warnings.Add(warning);
            }
            NavigateWarningCommand.NotifyCanExecuteChanged();
            _nextWarningCursor = page.NextCursor;
            IsWarningDrilldownOpen = true;
            WarningStatusMessage = page.Total == 0
                ? "No persisted warning aggregates are available."
                : $"Showing {Warnings.Count:N0} of {page.Total:N0} bounded warning aggregates accounting for {page.AccountedWarningCount:N0} warnings, {WarningSortDescription()}.";
            WarningAnnouncementVersion++;
            RequestFocus("warnings");
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception) when (generation == _warningGeneration && SelectedRun?.Id == run.Id)
        {
            WarningErrorMessage = exception.Message;
            if (opening)
            {
                IsWarningDrilldownOpen = true;
            }
        }
        finally
        {
            if (generation == _warningGeneration)
            {
                IsWarningLoading = false;
            }
        }
    }

    private void CacheWarningPage(string key, WorkerRunWarningPage page)
    {
        if (_warningCache.ContainsKey(key)) return;
        _warningCache[key] = page;
        _warningCacheOrder.Enqueue(key);
        while (_warningCacheOrder.Count > WarningCachePageLimit)
        {
            _warningCache.Remove(_warningCacheOrder.Dequeue());
        }
    }

    private void CancelWarningLoad() => _warningCancellation?.Cancel();

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
        WarningErrorMessage = null;
        WarningStatusMessage = $"Opening immutable duplicate-file results for run {runId:N0}…";
        WarningAnnouncementVersion++;

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

            WarningStatusMessage =
                $"Opened immutable duplicate-file results for run {runId:N0}. Persisted warning history was not changed.";
            WarningAnnouncementVersion++;
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception) when (IsCurrentWarningNavigation(generation, warningId, runId, token))
        {
            WarningErrorMessage =
                $"The immutable duplicate-file results for run {runId:N0} are unavailable. " +
                $"Refresh run history before trying this warning again. {exception.Message}";
            WarningErrorAnnouncementVersion++;
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
            WarningErrorMessage = null;
            return;
        }
        if (wasPending)
        {
            WarningStatusMessage = "Warning navigation was cancelled. Persisted warning history was not changed.";
            WarningAnnouncementVersion++;
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
        _warningCancellation?.Cancel();
        _warningCancellation?.Dispose();
        _warningCancellation = null;
        _warningGeneration++;
        IsWarningLoading = false;
        IsWarningDrilldownOpen = false;
        WarningErrorMessage = null;
        WarningStatusMessage = null;
        _nextWarningCursor = null;
        Warnings.Clear();
        NavigateWarningCommand.NotifyCanExecuteChanged();
        _warningCache.Clear();
        _warningCacheOrder.Clear();
        if (restoreFocus) RequestFocus("history");
    }

    private void RequestFocus(string target)
    {
        FocusTarget = target;
        FocusRequestVersion++;
    }

    private string WarningSortDescription() => (_warningSortField, _warningSortDirection) switch
    {
        (RunWarningSortField.OccurrenceCount, WorkerSortDirection.Descending) => "highest count first",
        (RunWarningSortField.OccurrenceCount, _) => "lowest count first",
        (RunWarningSortField.Phase, WorkerSortDirection.Ascending) => "phase A to Z",
        (RunWarningSortField.Phase, _) => "phase Z to A",
        (RunWarningSortField.Message, WorkerSortDirection.Ascending) => "warning text A to Z",
        _ => "warning text Z to A",
    };

    public void Dispose()
    {
        CloseWarnings(restoreFocus: false);
    }
}
