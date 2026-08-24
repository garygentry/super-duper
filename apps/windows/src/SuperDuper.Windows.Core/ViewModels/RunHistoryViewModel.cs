using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class RunHistoryViewModel : ObservableObject, IDisposable
{
    private const int PageSize = 500;
    public const int WarningPageSize = 25;
    public const int WarningCachePageLimit = 5;
    private readonly IWorkerClient _workerClient;
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
    private string? _nextWarningCursor;
    private long _warningAnnouncementVersion;
    private string _focusTarget = string.Empty;
    private long _focusRequestVersion;
    private readonly Dictionary<string, WorkerRunWarningPage> _warningCache = [];
    private readonly Queue<string> _warningCacheOrder = [];

    public RunHistoryViewModel(IWorkerClient workerClient)
    {
        _workerClient = workerClient;
        RefreshCommand = new AsyncRelayCommand(RefreshAsync, () => SessionId is not null && !IsLoading);
        OpenWarningsCommand = new AsyncRelayCommand(OpenWarningsAsync, () => CanOpenWarnings);
        NextWarningPageCommand = new AsyncRelayCommand(NextWarningPageAsync, () => CanLoadNextWarningPage);
        CancelWarningLoadCommand = new RelayCommand(CancelWarningLoad, () => IsWarningLoading);
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

    public bool CanOpenWarnings => SelectedRun?.Run.WarningCount > 0 && !IsWarningLoading && !IsWarningDrilldownOpen;

    public bool CanLoadNextWarningPage => IsWarningDrilldownOpen && !IsWarningLoading && _nextWarningCursor is not null;

    public long WarningAnnouncementVersion
    {
        get => _warningAnnouncementVersion;
        private set => SetProperty(ref _warningAnnouncementVersion, value);
    }

    public string FocusTarget { get => _focusTarget; private set => SetProperty(ref _focusTarget, value); }

    public long FocusRequestVersion { get => _focusRequestVersion; private set => SetProperty(ref _focusRequestVersion, value); }

    public IAsyncRelayCommand RefreshCommand { get; }

    public IAsyncRelayCommand OpenWarningsCommand { get; }

    public IAsyncRelayCommand NextWarningPageCommand { get; }

    public IRelayCommand CancelWarningLoadCommand { get; }

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
        _warningCancellation?.Dispose();
        _warningCancellation = new CancellationTokenSource();
        var token = _warningCancellation.Token;
        var generation = ++_warningGeneration;
        IsWarningLoading = true;
        WarningErrorMessage = null;
        try
        {
            var key = cursor ?? string.Empty;
            if (!_warningCache.TryGetValue(key, out var page))
            {
                page = await _workerClient.GetRunWarningsAsync(run.Id, WarningPageSize, cursor, token);
                if (page.ExecutorEnabled || page.WarningCount != page.AccountedWarningCount)
                {
                    throw new InvalidOperationException("The worker returned an unsafe or incomplete warning accounting page.");
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
            _nextWarningCursor = page.NextCursor;
            IsWarningDrilldownOpen = true;
            WarningStatusMessage = page.Total == 0
                ? "No persisted warning aggregates are available."
                : $"Showing {Warnings.Count:N0} of {page.Total:N0} bounded warning aggregates accounting for {page.AccountedWarningCount:N0} warnings.";
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

    private void CloseWarnings() => CloseWarnings(restoreFocus: true);

    private void CloseWarnings(bool restoreFocus)
    {
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
        _warningCache.Clear();
        _warningCacheOrder.Clear();
        if (restoreFocus) RequestFocus("history");
    }

    private void RequestFocus(string target)
    {
        FocusTarget = target;
        FocusRequestVersion++;
    }

    public void Dispose()
    {
        CloseWarnings(restoreFocus: false);
    }
}
