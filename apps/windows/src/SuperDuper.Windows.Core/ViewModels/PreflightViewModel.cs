using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class PreflightViewModel : ObservableObject, IDisposable
{
    private const int PageSize = 100;
    private const int MaximumCachedPages = 5;
    private static readonly TimeSpan PollInterval = TimeSpan.FromMilliseconds(150);

    private readonly IWorkerClient _worker;
    private readonly IUserConfirmationService _confirmation;
    private readonly Dictionary<string, WorkerPreflightItemPage> _pageCache = [];
    private readonly Queue<string> _cacheOrder = [];
    private readonly List<string?> _pageHistory = [];
    private CancellationTokenSource? _lifetime;
    private WorkerRun? _run;
    private WorkerReviewPlanView? _review;
    private WorkerPreflight? _preflight;
    private string? _currentCursor;
    private string? _nextCursor;
    private int _pageIndex;
    private long _generation;
    private bool _isLoading;
    private bool _isStarting;
    private bool _isCancelling;
    private string? _errorMessage;
    private string _announcement = string.Empty;
    private long _announcementVersion;
    private string _errorAnnouncement = string.Empty;
    private long _errorAnnouncementVersion;
    private string _focusTarget = string.Empty;
    private long _focusRequestVersion;
    private bool _disposed;

    public PreflightViewModel(IWorkerClient worker, IUserConfirmationService confirmation)
    {
        _worker = worker;
        _confirmation = confirmation;
        StartCommand = new AsyncRelayCommand(StartAsync, () => CanStart);
        CancelCommand = new AsyncRelayCommand(CancelAsync, () => CanCancel);
        NextPageCommand = new AsyncRelayCommand(NextPageAsync, () => CanMoveNext);
        PreviousPageCommand = new AsyncRelayCommand(PreviousPageAsync, () => CanMovePrevious);
    }

    public ObservableCollection<PreflightItemViewModel> Items { get; } = [];

    public IAsyncRelayCommand StartCommand { get; }

    public IAsyncRelayCommand CancelCommand { get; }

    public IAsyncRelayCommand NextPageCommand { get; }

    public IAsyncRelayCommand PreviousPageCommand { get; }

    public WorkerPreflight? Preflight
    {
        get => _preflight;
        private set
        {
            if (SetProperty(ref _preflight, value))
            {
                NotifyStateChanged();
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
                NotifyStateChanged();
            }
        }
    }

    public bool IsStarting
    {
        get => _isStarting;
        private set
        {
            if (SetProperty(ref _isStarting, value))
            {
                NotifyStateChanged();
            }
        }
    }

    public bool IsCancelling
    {
        get => _isCancelling;
        private set
        {
            if (SetProperty(ref _isCancelling, value))
            {
                NotifyStateChanged();
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

    public string Announcement
    {
        get => _announcement;
        private set => SetProperty(ref _announcement, value);
    }

    public long AnnouncementVersion
    {
        get => _announcementVersion;
        private set => SetProperty(ref _announcementVersion, value);
    }

    public string ErrorAnnouncement
    {
        get => _errorAnnouncement;
        private set => SetProperty(ref _errorAnnouncement, value);
    }

    public long ErrorAnnouncementVersion
    {
        get => _errorAnnouncementVersion;
        private set => SetProperty(ref _errorAnnouncementVersion, value);
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

    public bool HasRun => _run is not null;

    public bool IsRunCompleted => _run?.Status == "completed";

    public bool HasReviewRemovals => _review?.Summary.EffectiveRemovalFileCount > 0;

    public bool HasPreflight => Preflight is not null;

    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);

    public bool IsRunning => Preflight?.Status is "pending" or "running" or "cancelling";

    public bool IsTerminal => Preflight?.Status is "completed" or "cancelled" or "interrupted" or "failed";

    public bool IsCurrent => Preflight?.IsCurrent ?? true;

    public bool CanStart => IsRunCompleted
        && HasReviewRemovals
        && !IsLoading
        && !IsStarting
        && !IsRunning;

    public bool CanCancel => Preflight?.Status is "running" or "pending" && !IsCancelling;

    public bool CanMoveNext => !IsLoading && !string.IsNullOrEmpty(_nextCursor);

    public bool CanMovePrevious => !IsLoading && _pageIndex > 0;

    public double ProgressMaximum => Math.Max(1, Preflight?.TotalItemCount ?? 1);

    public double ProgressValue => Preflight?.ProcessedItemCount ?? 0;

    public string ProgressText => Preflight is null
        ? "Preflight has not run."
        : $"Checked {Preflight.ProcessedItemCount:N0} of {Preflight.TotalItemCount:N0} validation items.";

    public string PlanSummary => _review is null
        ? "Review decisions are unavailable."
        : $"Review revision {_review.Plan.Revision:N0}: {_review.Summary.EffectiveRemovalFileCount:N0} logical removal paths, "
          + $"{_review.Summary.PlannedRemovalPhysicalItemCount:N0} physical items, "
          + $"{DisplayFormatting.Bytes(_review.Summary.PlannedRemovalBytes)} planned bytes.";

    public string StatusSummary => Preflight is null
        ? "No preflight observations are stored for this run."
        : $"{PreflightStatus(Preflight.Status)}. Ready {Preflight.ReadyCount:N0}; "
          + $"changed {Preflight.ChangedCount:N0}; missing {Preflight.MissingCount:N0}; "
          + $"unavailable {Preflight.UnavailableCount:N0}; conflicts {Preflight.ConflictCount:N0}.";

    public string RevisionStatus => Preflight is null || Preflight.IsCurrent
        ? string.Empty
        : $"This preflight is bound to review revision {Preflight.ReviewRevision:N0}; "
          + $"the current review revision is {Preflight.CurrentReviewRevision:N0}. Run preflight again.";

    public string PageStatus => Items.Count == 0
        ? "No observation details on this page."
        : $"Page {_pageIndex + 1:N0}, showing {Items.Count:N0} observation details.";

    public async Task ShowRunAsync(WorkerRun? run, CancellationToken cancellationToken = default)
    {
        CancelLifetime();
        _lifetime = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var token = _lifetime.Token;
        var generation = ++_generation;
        _run = run;
        _review = null;
        Preflight = null;
        ResetPages();
        ErrorMessage = null;
        NotifyStateChanged();
        if (run?.Status != "completed")
        {
            return;
        }
        IsLoading = true;
        try
        {
            var reviewTask = _worker.GetReviewPlanAsync(run.Id, token);
            var preflightTask = _worker.GetLatestPreflightAsync(run.Id, token);
            var review = await reviewTask;
            var preflight = await preflightTask;
            if (generation != _generation || token.IsCancellationRequested)
            {
                return;
            }
            _review = review;
            Preflight = preflight;
            OnPropertyChanged(nameof(PlanSummary));
            if (preflight is not null && IsTerminal)
            {
                await LoadPageAsync(null, generation, token);
            }
            else if (preflight is not null && IsRunning)
            {
                _ = MonitorRecoveredPreflightAsync(preflight.Id, generation, token);
            }
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            PublishError(exception.Message);
        }
        finally
        {
            if (generation == _generation)
            {
                IsLoading = false;
            }
        }
    }

    public async Task RefreshReviewRevisionAsync(long runId, long revision)
    {
        if (_run?.Id != runId || _lifetime is null)
        {
            return;
        }
        try
        {
            _review = await _worker.GetReviewPlanAsync(runId, _lifetime.Token);
            if (Preflight is not null)
            {
                Preflight = await _worker.GetPreflightAsync(Preflight.Id, _lifetime.Token);
            }
            OnPropertyChanged(nameof(PlanSummary));
            NotifyStateChanged();
            if (Preflight is not null && !Preflight.IsCurrent)
            {
                Announcement = RevisionStatus;
                AnnouncementVersion++;
            }
        }
        catch (OperationCanceledException) when (_lifetime.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            PublishError(exception.Message);
        }
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }
        _disposed = true;
        CancelLifetime();
    }

    private async Task StartAsync()
    {
        if (_run is null || _review is null)
        {
            return;
        }
        var confirmed = await _confirmation.ConfirmAsync(
            "Run preflight validation?",
            $"Validate {_review.Summary.EffectiveRemovalFileCount:N0} reviewed removal paths against scan snapshots? "
            + "This reads local metadata and complete file content to calculate hashes. "
            + "Cloud placeholders and excluded locations will not be opened. No files will be deleted.");
        if (!confirmed)
        {
            return;
        }
        IsStarting = true;
        ErrorMessage = null;
        var generation = _generation;
        var token = _lifetime?.Token ?? CancellationToken.None;
        try
        {
            var result = await _worker.StartPreflightAsync(
                Guid.NewGuid().ToString("N"),
                _run.Id,
                _review.Plan.Revision,
                token);
            if (generation != _generation)
            {
                return;
            }
            Preflight = result.Preflight;
            ResetPages();
            RequestFocus("progress");
            Announcement = $"Preflight started. {ProgressText}";
            AnnouncementVersion++;
            await PollUntilTerminalAsync(result.Preflight.Id, generation, token);
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            PublishError(exception.Message);
        }
        finally
        {
            if (generation == _generation)
            {
                IsStarting = false;
            }
        }
    }

    private async Task PollUntilTerminalAsync(
        long preflightId,
        long generation,
        CancellationToken token)
    {
        while (generation == _generation && !token.IsCancellationRequested)
        {
            var current = await _worker.GetPreflightAsync(preflightId, token);
            if (generation != _generation)
            {
                return;
            }
            Preflight = current;
            if (current.Status is "completed" or "cancelled" or "interrupted" or "failed")
            {
                await LoadPageAsync(null, generation, token);
                Announcement = StatusSummary;
                AnnouncementVersion++;
                RequestFocus("summary");
                return;
            }
            await Task.Delay(PollInterval, token);
        }
    }

    private async Task MonitorRecoveredPreflightAsync(
        long preflightId,
        long generation,
        CancellationToken token)
    {
        try
        {
            await PollUntilTerminalAsync(preflightId, generation, token);
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (generation == _generation)
            {
                PublishError(exception.Message);
            }
        }
    }

    private async Task CancelAsync()
    {
        if (Preflight is null)
        {
            return;
        }
        var confirmed = await _confirmation.ConfirmAsync(
            "Cancel preflight?",
            "Stop validating the remaining items? Completed observations will remain available. No files will be deleted.");
        if (!confirmed)
        {
            return;
        }
        IsCancelling = true;
        try
        {
            Preflight = await _worker.CancelPreflightAsync(
                Preflight.Id,
                _lifetime?.Token ?? CancellationToken.None);
            Announcement = "Preflight cancellation requested.";
            AnnouncementVersion++;
        }
        catch (Exception exception)
        {
            PublishError(exception.Message);
        }
        finally
        {
            IsCancelling = false;
        }
    }

    private Task NextPageAsync()
    {
        if (_nextCursor is null || _lifetime is null)
        {
            return Task.CompletedTask;
        }
        _pageHistory.Add(_currentCursor);
        _pageIndex++;
        return LoadPageAsync(_nextCursor, _generation, _lifetime.Token);
    }

    private Task PreviousPageAsync()
    {
        if (_pageIndex <= 0 || _lifetime is null)
        {
            return Task.CompletedTask;
        }
        var cursor = _pageHistory[^1];
        _pageHistory.RemoveAt(_pageHistory.Count - 1);
        _pageIndex--;
        return LoadPageAsync(cursor, _generation, _lifetime.Token);
    }

    private async Task LoadPageAsync(string? cursor, long generation, CancellationToken token)
    {
        if (Preflight is null)
        {
            return;
        }
        IsLoading = true;
        try
        {
            var key = cursor ?? "first";
            if (!_pageCache.TryGetValue(key, out var page))
            {
                page = await _worker.GetPreflightItemsAsync(
                    new PreflightItemQuery(Preflight.Id, PageSize, null, cursor),
                    token);
                if (generation != _generation)
                {
                    return;
                }
                CachePage(key, page);
            }
            _currentCursor = cursor;
            _nextCursor = page.NextCursor;
            Items.Clear();
            foreach (var item in page.Items)
            {
                Items.Add(new PreflightItemViewModel(item));
            }
            OnPropertyChanged(nameof(PageStatus));
            Announcement = PageStatus;
            AnnouncementVersion++;
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            PublishError(exception.Message);
        }
        finally
        {
            if (generation == _generation)
            {
                IsLoading = false;
                NotifyStateChanged();
            }
        }
    }

    private void CachePage(string key, WorkerPreflightItemPage page)
    {
        if (_pageCache.ContainsKey(key))
        {
            return;
        }
        _pageCache[key] = page;
        _cacheOrder.Enqueue(key);
        while (_cacheOrder.Count > MaximumCachedPages)
        {
            _pageCache.Remove(_cacheOrder.Dequeue());
        }
    }

    private void ResetPages()
    {
        Items.Clear();
        _pageCache.Clear();
        _cacheOrder.Clear();
        _pageHistory.Clear();
        _currentCursor = null;
        _nextCursor = null;
        _pageIndex = 0;
        OnPropertyChanged(nameof(PageStatus));
        NotifyStateChanged();
    }

    private void PublishError(string message)
    {
        ErrorMessage = message;
        ErrorAnnouncement = $"Preflight error. {message}";
        ErrorAnnouncementVersion++;
    }

    private void RequestFocus(string target)
    {
        FocusTarget = target;
        FocusRequestVersion++;
    }

    private void NotifyStateChanged()
    {
        foreach (var property in new[]
        {
            nameof(HasRun), nameof(IsRunCompleted), nameof(HasReviewRemovals), nameof(HasPreflight),
            nameof(IsRunning), nameof(IsTerminal), nameof(IsCurrent), nameof(CanStart),
            nameof(CanCancel), nameof(CanMoveNext), nameof(CanMovePrevious), nameof(ProgressMaximum),
            nameof(ProgressValue), nameof(ProgressText), nameof(PlanSummary), nameof(StatusSummary),
            nameof(RevisionStatus), nameof(PageStatus),
        })
        {
            OnPropertyChanged(property);
        }
        StartCommand.NotifyCanExecuteChanged();
        CancelCommand.NotifyCanExecuteChanged();
        NextPageCommand.NotifyCanExecuteChanged();
        PreviousPageCommand.NotifyCanExecuteChanged();
    }

    private void CancelLifetime()
    {
        _lifetime?.Cancel();
        _lifetime?.Dispose();
        _lifetime = null;
    }

    private static string PreflightStatus(string status) => status switch
    {
        "pending" => "Pending",
        "running" => "Running",
        "cancelling" => "Cancelling",
        "completed" => "Completed",
        "cancelled" => "Cancelled",
        "interrupted" => "Interrupted",
        "failed" => "Failed",
        _ => status,
    };
}

public sealed class PreflightItemViewModel
{
    public PreflightItemViewModel(WorkerPreflightItem item)
    {
        Item = item;
    }

    public WorkerPreflightItem Item { get; }

    public string Outcome => char.ToUpperInvariant(Item.Outcome[0]) + Item.Outcome[1..];

    public string Target => $"{(Item.TargetRole == "remove" ? "Removal" : "Survivor")} {Item.TargetKind}";

    public string Path => Item.Path;

    public string Explanation => Item.ReasonCode switch
    {
        "matched_snapshot" => "Identity, size, modified time, and content hash match the scan.",
        "folder_tree_matched" => "The complete folder tree matches the scan snapshot.",
        "path_missing" or "folder_missing" => "The reviewed path is missing.",
        "identity_changed" => "The path now identifies a different physical file.",
        "size_changed" => "The file size changed after the scan.",
        "timestamp_changed" => "The modified time changed after the scan.",
        "content_hash_changed" => "The complete content hash no longer matches the scan.",
        "changed_during_validation" => "The file changed while preflight was reading it.",
        "cloud_placeholder" or "folder_contains_cloud_placeholder" =>
            "A cloud placeholder was not opened or hydrated.",
        "excluded_location" or "folder_contains_excluded_location" =>
            "The path is inside an excluded location and was not opened.",
        "reparse_point" or "folder_reparse_point" or "folder_contains_reparse_point" =>
            "A link or reparse point is not an eligible reviewed target.",
        "survivor_not_ready" => "No independently accessible physical survivor validated successfully.",
        "folder_survivor_not_ready" => "No intact exact-folder survivor validated successfully.",
        "folder_tree_changed" => "The folder contains added, removed, renamed, or type-changed entries.",
        null => "Validation has not recorded an explanation.",
        _ => Item.ReasonCode.Replace('_', ' '),
    };

    public string AutomationName => $"{Outcome}; {Target}; {Path}; {Explanation}";
}
