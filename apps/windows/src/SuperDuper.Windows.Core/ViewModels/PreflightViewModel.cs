using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class PreflightViewModel : ObservableObject, IDisposable
{
    public const int ReviewPageSize = 200;
    public const int ReviewCacheCapacity = 5;
    private const int PageSize = 100;
    private const int MaximumCachedPages = 5;
    private static readonly TimeSpan PollInterval = TimeSpan.FromMilliseconds(150);

    private readonly IWorkerClient _worker;
    private readonly IUserConfirmationService _confirmation;
    private readonly Func<ReviewResultTarget, Task>? _navigateToResult;
    private readonly BoundedCursorCache<WorkerReviewGroupPage> _fileReviewPageCache = new(ReviewCacheCapacity);
    private readonly BoundedCursorCache<WorkerReviewFolderGroupPage> _folderReviewPageCache = new(ReviewCacheCapacity);
    private readonly Dictionary<string, WorkerPreflightItemPage> _pageCache = [];
    private readonly Queue<string> _cacheOrder = [];
    private readonly List<string?> _pageHistory = [];
    private CancellationTokenSource? _lifetime;
    private WorkerRun? _run;
    private WorkerReviewPlanView? _review;
    private WorkerPreflight? _preflight;
    private IReadOnlyList<ReviewFileGroupListItemViewModel> _fileReviewGroups = [];
    private IReadOnlyList<ReviewFolderGroupListItemViewModel> _folderReviewGroups = [];
    private readonly List<string?> _fileReviewPageHistory = [];
    private readonly List<string?> _folderReviewPageHistory = [];
    private string? _fileReviewCursor;
    private string? _nextFileReviewCursor;
    private string? _folderReviewCursor;
    private string? _nextFolderReviewCursor;
    private int _fileReviewPageIndex;
    private int _folderReviewPageIndex;
    private long _fileReviewTotal;
    private long _folderReviewTotal;
    private bool _isFileReviewLoading;
    private bool _isFolderReviewLoading;
    private string? _fileReviewErrorMessage;
    private string? _folderReviewErrorMessage;
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

    public PreflightViewModel(
        IWorkerClient worker,
        IUserConfirmationService confirmation,
        IRecycleOperationCapabilityExecutor? recycleOperationExecutor = null,
        IClipboardService? clipboard = null,
        IRecycleBinService? recycleBin = null,
        Func<Task>? navigateToFreshScan = null,
        Func<ReviewResultTarget, Task>? navigateToResult = null,
        PreferenceRulesViewModel? preferenceRules = null)
    {
        _worker = worker;
        _confirmation = confirmation;
        _navigateToResult = navigateToResult;
        PreferenceRules = preferenceRules;
        Operation = new RecycleOperationViewModel(
            worker,
            recycleOperationExecutor,
            clipboard,
            recycleBin,
            navigateToFreshScan);
        StartCommand = new AsyncRelayCommand(StartAsync, () => CanStart);
        CancelCommand = new AsyncRelayCommand(CancelAsync, () => CanCancel);
        NextPageCommand = new AsyncRelayCommand(NextPageAsync, () => CanMoveNext);
        PreviousPageCommand = new AsyncRelayCommand(PreviousPageAsync, () => CanMovePrevious);
        NextFileReviewPageCommand = new AsyncRelayCommand(NextFileReviewPageAsync, () => CanMoveFileReviewNext);
        PreviousFileReviewPageCommand = new AsyncRelayCommand(PreviousFileReviewPageAsync, () => CanMoveFileReviewPrevious);
        NextFolderReviewPageCommand = new AsyncRelayCommand(NextFolderReviewPageAsync, () => CanMoveFolderReviewNext);
        PreviousFolderReviewPageCommand = new AsyncRelayCommand(PreviousFolderReviewPageAsync, () => CanMoveFolderReviewPrevious);
        OpenReviewResultCommand = new AsyncRelayCommand<ReviewResultTarget>(OpenReviewResultAsync);
        OpenPreflightResultCommand = new AsyncRelayCommand<PreflightItemViewModel>(OpenPreflightResultAsync);
    }

    public ObservableCollection<PreflightItemViewModel> Items { get; } = [];

    public PreferenceRulesViewModel? PreferenceRules { get; }

    public RecycleOperationViewModel Operation { get; }

    public IAsyncRelayCommand StartCommand { get; }

    public IAsyncRelayCommand CancelCommand { get; }

    public IAsyncRelayCommand NextPageCommand { get; }

    public IAsyncRelayCommand PreviousPageCommand { get; }

    public IAsyncRelayCommand NextFileReviewPageCommand { get; }

    public IAsyncRelayCommand PreviousFileReviewPageCommand { get; }

    public IAsyncRelayCommand NextFolderReviewPageCommand { get; }

    public IAsyncRelayCommand PreviousFolderReviewPageCommand { get; }

    public IAsyncRelayCommand<ReviewResultTarget> OpenReviewResultCommand { get; }

    public IAsyncRelayCommand<PreflightItemViewModel> OpenPreflightResultCommand { get; }

    public IReadOnlyList<ReviewFileGroupListItemViewModel> FileReviewGroups
    {
        get => _fileReviewGroups;
        private set
        {
            if (SetProperty(ref _fileReviewGroups, value))
            {
                OnPropertyChanged(nameof(HasFileReviewGroups));
                OnPropertyChanged(nameof(FileReviewPageStatus));
            }
        }
    }

    public IReadOnlyList<ReviewFolderGroupListItemViewModel> FolderReviewGroups
    {
        get => _folderReviewGroups;
        private set
        {
            if (SetProperty(ref _folderReviewGroups, value))
            {
                OnPropertyChanged(nameof(HasFolderReviewGroups));
                OnPropertyChanged(nameof(FolderReviewPageStatus));
            }
        }
    }

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

    public long? SelectedRunId => _run?.Id;

    public bool IsRunCompleted => _run?.Status == "completed";

    public bool HasReviewRemovals => _review?.Summary.EffectiveRemovalFileCount > 0;

    public bool HasReview => _review is not null;

    public bool HasFileReviewGroups => FileReviewGroups.Count > 0;

    public bool HasFolderReviewGroups => FolderReviewGroups.Count > 0;

    public bool IsFileReviewLoading
    {
        get => _isFileReviewLoading;
        private set
        {
            if (SetProperty(ref _isFileReviewLoading, value))
            {
                NotifyReviewPagingChanged();
            }
        }
    }

    public bool IsFolderReviewLoading
    {
        get => _isFolderReviewLoading;
        private set
        {
            if (SetProperty(ref _isFolderReviewLoading, value))
            {
                NotifyReviewPagingChanged();
            }
        }
    }

    public string? FileReviewErrorMessage
    {
        get => _fileReviewErrorMessage;
        private set
        {
            if (SetProperty(ref _fileReviewErrorMessage, value))
            {
                OnPropertyChanged(nameof(HasFileReviewError));
            }
        }
    }

    public string? FolderReviewErrorMessage
    {
        get => _folderReviewErrorMessage;
        private set
        {
            if (SetProperty(ref _folderReviewErrorMessage, value))
            {
                OnPropertyChanged(nameof(HasFolderReviewError));
            }
        }
    }

    public bool HasFileReviewError => !string.IsNullOrWhiteSpace(FileReviewErrorMessage);

    public bool HasFolderReviewError => !string.IsNullOrWhiteSpace(FolderReviewErrorMessage);

    public bool CanMoveFileReviewNext => !IsFileReviewLoading && _nextFileReviewCursor is not null;

    public bool CanMoveFileReviewPrevious => !IsFileReviewLoading && _fileReviewPageIndex > 0;

    public bool CanMoveFolderReviewNext => !IsFolderReviewLoading && _nextFolderReviewCursor is not null;

    public bool CanMoveFolderReviewPrevious => !IsFolderReviewLoading && _folderReviewPageIndex > 0;

    public int CachedFileReviewPageCount => _fileReviewPageCache.Count;

    public int CachedFolderReviewPageCount => _folderReviewPageCache.Count;

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
        : $"Plan revision {_review.Plan.Revision:N0} · {FormatMarkedCopies(_review.Summary.RemoveCount, "file")} · "
          + $"{FormatMarkedCopies(_review.Summary.FolderRemoveCount, "folder")}.";

    public string SelectedRunContext => _run is null
        ? "No scan selected"
        : $"Scan {_run.Id:N0} · {(_run.StartedAt ?? _run.CreatedAt).ToLocalTime():g} · "
          + $"{DisplayFormatting.Status(_run.Status)}";

    public string CombinedRemovalSummary => _review is null
        ? "Combined plan totals are unavailable."
        : $"{_review.Summary.EffectiveRemovalFileCount:N0} distinct affected files · "
          + $"{_review.Summary.PlannedRemovalPhysicalItemCount:N0} physical items · "
          + $"{DisplayFormatting.Bytes(_review.Summary.PlannedRemovalBytes)} planned.";

    public string MarkedRemovalSummary => _review is null
        ? "Marked totals are unavailable."
        : $"{_review.Summary.EffectiveRemovalFileCount:N0} {(_review.Summary.EffectiveRemovalFileCount == 1 ? "file" : "files")} marked · {DisplayFormatting.Bytes(_review.Summary.PlannedRemovalBytes)} planned";

    public string CombinedRemovalExplanation =>
        "Whole-plan totals come from the worker. File/folder overlap and hard-link aliases are counted once in the distinct and physical totals.";

    public string FileReviewPageStatus => _fileReviewTotal == 0
        ? "No file sets in this selected scan."
        : $"Files page {_fileReviewPageIndex + 1:N0} · showing {FileReviewGroups.Count:N0} of {_fileReviewTotal:N0} review sets.";

    public string FolderReviewPageStatus => _folderReviewTotal == 0
        ? "No folder sets in this selected scan."
        : $"Folders page {_folderReviewPageIndex + 1:N0} · showing {FolderReviewGroups.Count:N0} of {_folderReviewTotal:N0} review sets.";

    public string StatusSummary => Preflight is null
        ? "No preflight observations are stored for this run."
        : $"{PreflightStatus(Preflight.Status)}. Ready {Preflight.ReadyCount:N0}; "
          + $"changed {Preflight.ChangedCount:N0}; missing {Preflight.MissingCount:N0}; "
          + $"unavailable {Preflight.UnavailableCount:N0}; conflicts {Preflight.ConflictCount:N0}.";

    private bool HasNewerFileEvidence => Preflight is { IsCurrent: false }
        && Preflight.ReviewRevision == Preflight.CurrentReviewRevision;

    public string RevisionStatus => Preflight is null || Preflight.IsCurrent
        ? string.Empty
        : HasNewerFileEvidence
        ? "Files may have changed since this check. Check marked copies and the copies you are keeping again."
        : $"Plan changed — check again. The saved check is for plan revision {Preflight.ReviewRevision:N0}; "
          + $"the current review revision is {Preflight.CurrentReviewRevision:N0}.";

    public string ValidationFreshnessTitle => Preflight switch
    {
        null => "Plan has not been checked",
        { IsCurrent: false } when HasNewerFileEvidence => "Copies need another check",
        { IsCurrent: false } => "Plan changed — check again",
        _ => "Checked against your current decisions",
    };

    public string ValidationOutcomeTitle => Preflight switch
    {
        null => "Needs review",
        { IsCurrent: false } => "Needs review",
        { Status: "pending" or "running" or "cancelling" } => "Checking marked copies",
        { Status: "completed", ConflictCount: > 0 } => "Blocked",
        { Status: "completed", ChangedCount: > 0 } => "Needs review",
        { Status: "completed", MissingCount: > 0 } => "Needs review",
        { Status: "completed", UnavailableCount: > 0 } => "Needs review",
        { Status: "completed" } => "Ready",
        _ => "Needs review",
    };

    public string ValidationOutcomeExplanation => Preflight switch
    {
        null => "Check marked copies and the copies you are keeping. No files are deleted.",
        { IsCurrent: false } when HasNewerFileEvidence =>
            "Files may have changed since this check. Check marked copies and the copies you are keeping again.",
        { IsCurrent: false } => "Your decisions changed after the last check. Check the updated plan again.",
        { Status: "pending" or "running" or "cancelling" } => ProgressText,
        { Status: "completed", ConflictCount: > 0 } =>
            "A set has no safe copy to keep. Review it and keep at least one independently accessible copy.",
        { Status: "completed", ChangedCount: > 0 } =>
            "One or more marked or retained copies changed after the scan. Review the affected sets and check the updated plan again.",
        { Status: "completed", MissingCount: > 0 } =>
            "One or more marked or retained copies are missing. Review the affected sets and check the updated plan again.",
        { Status: "completed", UnavailableCount: > 0 } =>
            "One or more marked or retained copies could not be checked. Resolve access and check the same current plan again.",
        { Status: "completed" } =>
            "Marked copies and the copies you are keeping passed the check. Nothing has been deleted.",
        { Status: "cancelled" } => "The whole-plan check was cancelled before it could establish a current result.",
        { Status: "interrupted" } => "The worker stopped before the whole-plan check completed.",
        { Status: "failed" } => "The whole-plan check failed. Review its details, then check the current plan again.",
        _ => "The whole-plan check does not currently establish readiness.",
    };

    public string CheckMarkedCopiesLabel => IsStarting ? "Starting check…" : "Check marked copies";

    public string BuildBoundaryNotice =>
        "This build can review and check a removal plan. Moving files to the Recycle Bin is not available.";

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
        IsLoading = false;
        _review = null;
        Preflight = null;
        ResetPages();
        ResetReviewPages();
        ErrorMessage = null;
        NotifyStateChanged();
        var operationTask = Operation.ShowRunAsync(run, token);
        var preferenceTask = PreferenceRules?.EnsureRunAsync(run, token) ?? Task.CompletedTask;
        if (run?.Status != "completed")
        {
            await Task.WhenAll(operationTask, preferenceTask);
            return;
        }
        IsLoading = true;
        try
        {
            var preflightTask = _worker.GetLatestPreflightAsync(run.Id, token);
            var review = await _worker.GetReviewPlanAsync(run.Id, token);
            if (generation != _generation || token.IsCancellationRequested)
            {
                return;
            }
            _review = review;
            await preferenceTask;
            PreferenceRules?.SynchronizeReviewRevision(review.Plan.Revision);
            NotifyReviewOverviewChanged();
            var fileGroupsTask = LoadFileReviewPageAsync(null, generation, token);
            var folderGroupsTask = LoadFolderReviewPageAsync(null, generation, token);
            var preflight = await preflightTask;
            await Task.WhenAll(fileGroupsTask, folderGroupsTask);
            await operationTask;
            if (generation != _generation || token.IsCancellationRequested)
            {
                return;
            }
            Preflight = preflight;
            NotifyReviewOverviewChanged();
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
            if (generation == _generation && !token.IsCancellationRequested) PublishError(exception.Message);
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
        var generation = _generation;
        var token = _lifetime.Token;
        var previousPreflight = Preflight;
        try
        {
            var review = await _worker.GetReviewPlanAsync(runId, token);
            var preflight = previousPreflight is null ? null
                : await _worker.GetPreflightAsync(previousPreflight.Id, token);
            if (generation != _generation || token.IsCancellationRequested) return;
            _review = review;
            PreferenceRules?.SynchronizeReviewRevision(review.Plan.Revision);
            Preflight = preflight;
            ResetReviewPages();
            await Task.WhenAll(
                LoadFileReviewPageAsync(null, generation, token),
                LoadFolderReviewPageAsync(null, generation, token));
            if (generation != _generation || token.IsCancellationRequested) return;
            NotifyReviewOverviewChanged();
            NotifyStateChanged();
            if (Preflight is not null && !Preflight.IsCurrent)
            {
                Announcement = $"{RevisionStatus} Run preflight again using Check marked copies. {ValidationOutcomeTitle}.";
                AnnouncementVersion++;
            }
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (generation == _generation && !token.IsCancellationRequested) PublishError(exception.Message);
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
        Operation.Dispose();
    }

    private async Task StartAsync()
    {
        if (_run is null || _review is null)
        {
            return;
        }
        var confirmed = await _confirmation.ConfirmAsync(
            "Check marked copies?",
            $"Check {_review.Summary.EffectiveRemovalFileCount:N0} marked removal paths against scan snapshots? "
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
            Announcement = $"Plan check started. {ProgressText}";
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

    private async Task NextFileReviewPageAsync()
    {
        if (_nextFileReviewCursor is null || _lifetime is null)
        {
            return;
        }
        var nextCursor = _nextFileReviewCursor;
        var previousCursor = _fileReviewCursor;
        if (await LoadFileReviewPageAsync(nextCursor, _generation, _lifetime.Token))
        {
            _fileReviewPageHistory.Add(previousCursor);
            _fileReviewPageIndex++;
            NotifyReviewPagingChanged();
        }
    }

    private async Task PreviousFileReviewPageAsync()
    {
        if (_fileReviewPageIndex <= 0 || _lifetime is null)
        {
            return;
        }
        var cursor = _fileReviewPageHistory[^1];
        if (await LoadFileReviewPageAsync(cursor, _generation, _lifetime.Token))
        {
            _fileReviewPageHistory.RemoveAt(_fileReviewPageHistory.Count - 1);
            _fileReviewPageIndex--;
            NotifyReviewPagingChanged();
        }
    }

    private async Task NextFolderReviewPageAsync()
    {
        if (_nextFolderReviewCursor is null || _lifetime is null)
        {
            return;
        }
        var nextCursor = _nextFolderReviewCursor;
        var previousCursor = _folderReviewCursor;
        if (await LoadFolderReviewPageAsync(nextCursor, _generation, _lifetime.Token))
        {
            _folderReviewPageHistory.Add(previousCursor);
            _folderReviewPageIndex++;
            NotifyReviewPagingChanged();
        }
    }

    private async Task PreviousFolderReviewPageAsync()
    {
        if (_folderReviewPageIndex <= 0 || _lifetime is null)
        {
            return;
        }
        var cursor = _folderReviewPageHistory[^1];
        if (await LoadFolderReviewPageAsync(cursor, _generation, _lifetime.Token))
        {
            _folderReviewPageHistory.RemoveAt(_folderReviewPageHistory.Count - 1);
            _folderReviewPageIndex--;
            NotifyReviewPagingChanged();
        }
    }

    private async Task<bool> LoadFileReviewPageAsync(string? cursor, long generation, CancellationToken token)
    {
        if (_run is null || _review is null)
        {
            return false;
        }
        IsFileReviewLoading = true;
        FileReviewErrorMessage = null;
        try
        {
            if (!_fileReviewPageCache.TryGet(cursor, out var page))
            {
                page = await _worker.GetReviewGroupsAsync(_run.Id, ReviewPageSize, cursor, token);
                if (generation != _generation || token.IsCancellationRequested)
                {
                    return false;
                }
                if (page.Revision != _review.Plan.Revision || page.PlanId != _review.Plan.Id)
                {
                    throw new InvalidOperationException(
                        "The review plan changed while the Files page was loading. Reopen Review to load one current revision.");
                }
                _fileReviewPageCache.Set(cursor, page);
            }
            _fileReviewCursor = cursor;
            _nextFileReviewCursor = page.NextCursor;
            _fileReviewTotal = page.Total;
            FileReviewGroups = page.Groups.Select(group => new ReviewFileGroupListItemViewModel(group)).ToArray();
            OnPropertyChanged(nameof(FileReviewPageStatus));
            return true;
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
            return false;
        }
        catch (Exception exception)
        {
            if (generation == _generation && !token.IsCancellationRequested)
            {
                FileReviewErrorMessage = exception.Message;
            }
            return false;
        }
        finally
        {
            if (generation == _generation)
            {
                IsFileReviewLoading = false;
            }
        }
    }

    private async Task<bool> LoadFolderReviewPageAsync(string? cursor, long generation, CancellationToken token)
    {
        if (_run is null || _review is null)
        {
            return false;
        }
        IsFolderReviewLoading = true;
        FolderReviewErrorMessage = null;
        try
        {
            if (!_folderReviewPageCache.TryGet(cursor, out var page))
            {
                page = await _worker.GetReviewFolderGroupsAsync(_run.Id, ReviewPageSize, cursor, token);
                if (generation != _generation || token.IsCancellationRequested)
                {
                    return false;
                }
                if (page.Revision != _review.Plan.Revision || page.PlanId != _review.Plan.Id)
                {
                    throw new InvalidOperationException(
                        "The review plan changed while the Folders page was loading. Reopen Review to load one current revision.");
                }
                _folderReviewPageCache.Set(cursor, page);
            }
            _folderReviewCursor = cursor;
            _nextFolderReviewCursor = page.NextCursor;
            _folderReviewTotal = page.Total;
            FolderReviewGroups = page.Groups.Select(group => new ReviewFolderGroupListItemViewModel(group)).ToArray();
            OnPropertyChanged(nameof(FolderReviewPageStatus));
            return true;
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
            return false;
        }
        catch (Exception exception)
        {
            if (generation == _generation && !token.IsCancellationRequested)
            {
                FolderReviewErrorMessage = exception.Message;
            }
            return false;
        }
        finally
        {
            if (generation == _generation)
            {
                IsFolderReviewLoading = false;
            }
        }
    }

    private async Task OpenReviewResultAsync(ReviewResultTarget? target)
    {
        if (target is null || _navigateToResult is null)
        {
            return;
        }
        try
        {
            ErrorMessage = null;
            await _navigateToResult(target);
        }
        catch (Exception exception)
        {
            PublishError(exception.Message);
        }
    }

    private Task OpenPreflightResultAsync(PreflightItemViewModel? item)
    {
        if (item?.ResultTarget is not { } target)
        {
            return Task.CompletedTask;
        }
        return OpenReviewResultAsync(target);
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

    private void ResetReviewPages()
    {
        _fileReviewPageCache.Clear();
        _folderReviewPageCache.Clear();
        _fileReviewPageHistory.Clear();
        _folderReviewPageHistory.Clear();
        _fileReviewCursor = null;
        _nextFileReviewCursor = null;
        _folderReviewCursor = null;
        _nextFolderReviewCursor = null;
        _fileReviewPageIndex = 0;
        _folderReviewPageIndex = 0;
        _fileReviewTotal = 0;
        _folderReviewTotal = 0;
        FileReviewGroups = [];
        FolderReviewGroups = [];
        FileReviewErrorMessage = null;
        FolderReviewErrorMessage = null;
        NotifyReviewPagingChanged();
    }

    private void PublishError(string message)
    {
        ErrorMessage = message;
        ErrorAnnouncement = $"Review error. {message}";
        ErrorAnnouncementVersion++;
    }

    private static string FormatMarkedCopies(long count, string kind) =>
        $"{count:N0} {kind} {(count == 1 ? "copy" : "copies")} marked";

    private void RequestFocus(string target)
    {
        FocusTarget = target;
        FocusRequestVersion++;
    }

    private void NotifyStateChanged()
    {
        foreach (var property in new[]
        {
            nameof(HasRun), nameof(IsRunCompleted), nameof(HasReview), nameof(HasReviewRemovals), nameof(HasPreflight),
            nameof(IsRunning), nameof(IsTerminal), nameof(IsCurrent), nameof(CanStart),
            nameof(CanCancel), nameof(CanMoveNext), nameof(CanMovePrevious), nameof(ProgressMaximum),
            nameof(ProgressValue), nameof(ProgressText), nameof(PlanSummary), nameof(StatusSummary),
            nameof(SelectedRunContext), nameof(CombinedRemovalSummary), nameof(MarkedRemovalSummary), nameof(RevisionStatus),
            nameof(ValidationFreshnessTitle), nameof(ValidationOutcomeTitle),
            nameof(ValidationOutcomeExplanation), nameof(CheckMarkedCopiesLabel), nameof(PageStatus),
        })
        {
            OnPropertyChanged(property);
        }
        StartCommand.NotifyCanExecuteChanged();
        CancelCommand.NotifyCanExecuteChanged();
        NextPageCommand.NotifyCanExecuteChanged();
        PreviousPageCommand.NotifyCanExecuteChanged();
    }

    private void NotifyReviewOverviewChanged()
    {
        OnPropertyChanged(nameof(HasReview));
        OnPropertyChanged(nameof(HasReviewRemovals));
        OnPropertyChanged(nameof(PlanSummary));
        OnPropertyChanged(nameof(CombinedRemovalSummary));
        OnPropertyChanged(nameof(MarkedRemovalSummary));
        OnPropertyChanged(nameof(SelectedRunContext));
        NotifyStateChanged();
    }

    private void NotifyReviewPagingChanged()
    {
        OnPropertyChanged(nameof(CanMoveFileReviewNext));
        OnPropertyChanged(nameof(CanMoveFileReviewPrevious));
        OnPropertyChanged(nameof(CanMoveFolderReviewNext));
        OnPropertyChanged(nameof(CanMoveFolderReviewPrevious));
        OnPropertyChanged(nameof(FileReviewPageStatus));
        OnPropertyChanged(nameof(FolderReviewPageStatus));
        NextFileReviewPageCommand.NotifyCanExecuteChanged();
        PreviousFileReviewPageCommand.NotifyCanExecuteChanged();
        NextFolderReviewPageCommand.NotifyCanExecuteChanged();
        PreviousFolderReviewPageCommand.NotifyCanExecuteChanged();
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

    /// <summary>Plain spelling of <see cref="Path"/> for visible text.</summary>
    public string DisplayPath => DisplayPaths.Plain(Item.Path);

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

    public string AutomationName => $"{Outcome}; {Target}; {DisplayPath}; {Explanation}";

    public ReviewResultTarget? ResultTarget => Item switch
    {
        { GroupId: { } groupId } => new ReviewResultTarget(ReviewResultKind.File, groupId, Item.SnapshotFileId),
        { FolderGroupId: { } groupId } => new ReviewResultTarget(ReviewResultKind.Folder, groupId, Item.FolderMemberId),
        _ => null,
    };

    public bool CanOpenResult => ResultTarget is not null;
}
