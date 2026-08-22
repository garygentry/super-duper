using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class RecycleOperationViewModel : ObservableObject, IDisposable
{
    private const int PageSize = 100;
    private const int MaximumCachedPages = 5;
    private readonly IRecycleOperationWorkerClient? _worker;
    private readonly IRecycleOperationCapabilityExecutor? _executor;
    private readonly BoundedCursorCache<WorkerRecycleOperationItemPage> _cache = new(MaximumCachedPages);
    private readonly List<string?> _history = [];
    private CancellationTokenSource? _lifetime;
    private WorkerRecycleOperation? _operation;
    private string? _cursor;
    private string? _nextCursor;
    private int _pageIndex;
    private long _generation;
    private bool _isLoading;
    private string? _errorMessage;
    private string _announcement = string.Empty;
    private long _announcementVersion;

    public RecycleOperationViewModel(
        IWorkerClient worker,
        IRecycleOperationCapabilityExecutor? executor = null)
    {
        _worker = worker as IRecycleOperationWorkerClient;
        _executor = executor;
        NextPageCommand = new AsyncRelayCommand(NextPageAsync, () => CanMoveNext);
        PreviousPageCommand = new AsyncRelayCommand(PreviousPageAsync, () => CanMovePrevious);
    }

    public ObservableCollection<RecycleOperationItemViewModel> Items { get; } = [];

    public IAsyncRelayCommand NextPageCommand { get; }

    public IAsyncRelayCommand PreviousPageCommand { get; }

    public WorkerRecycleOperation? Operation
    {
        get => _operation;
        private set
        {
            if (SetProperty(ref _operation, value))
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

    public bool HasOperation => Operation is not null;

    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);

    public bool IsExecutorEnabled => _executor?.IsEnabled == true;

    public bool CanSubmit => false;

    public bool CanMoveNext => !IsLoading && !string.IsNullOrEmpty(_nextCursor);

    public bool CanMovePrevious => !IsLoading && _pageIndex > 0;

    public string BoundaryNotice => IsExecutorEnabled
        ? "A separately reviewed executor is present, but this surface still does not submit Shell work."
        : "Recycle Bin execution is disabled. This build can reconstruct durable intent and results, but cannot move files.";

    public string ConfirmationSummary => Operation is null
        ? "No Recycle Bin operation intent is stored for this run."
        : $"Final confirmation would move {Operation.LogicalRemovalCount:N0} reviewed paths "
          + $"({Operation.ShellItemCount:N0} bounded Shell items, {DisplayFormatting.Bytes(Operation.PlannedRemovalBytes)}) "
          + $"to the Windows Recycle Bin across {Operation.AffectedLocationCount:N0} locations. "
          + $"{Operation.ExclusionCount:N0} excluded locations remain untouched. Shell providers can return partial or ambiguous results; no action is enabled here.";

    public string ProgressSummary => Operation is null
        ? "No operation progress is available."
        : $"{StatusText(Operation.Status)}. Recycled {Operation.RecycledCount:N0}; failed {Operation.FailedCount:N0}; "
          + $"cancelled {Operation.CancelledCount:N0}; unknown {Operation.UnknownCount:N0}; pending {Operation.PendingResultCount:N0}.";

    public string CancellationDisclosure => Operation?.Status switch
    {
        "prepared" or "awaiting_confirmation" =>
            "No Shell work has started. Leaving this screen does not move any files.",
        "submitted" =>
            "Cancellation can still stop before a Shell batch starts. It cannot undo an item that Windows has already recycled.",
        "executing" or "cancelling" =>
            "Cancellation is boundary-aware: the current Windows Shell item may finish, then processing stops before the next safe item or batch. Already recycled items are not restored by the app.",
        "recovery_required" =>
            "Do not retry this operation. Windows may have recycled one or more unknown items before reporting was interrupted.",
        "partially_completed" =>
            "Some items were recycled and some were not. Cancellation and failures never restore items that Windows already recycled.",
        "completed" =>
            "Windows reported the operation complete. Restore, if available, is managed independently through the Windows Recycle Bin.",
        "cancelled" =>
            "Processing stopped at a safe boundary. Items recycled before cancellation were not restored by the app.",
        "failed" =>
            "Processing failed. Any item already reported as recycled was not restored by the app.",
        "expired" =>
            "The freshness lease expired before execution; this intent cannot be submitted.",
        _ => "No cancellation boundary is available for this operation state.",
    };

    public string RecoveryGuidance => Operation?.Status == "recovery_required"
        ? "Recovery requires operator review of every unknown item in its source location and in the Windows Recycle Bin. This build preserves the evidence and cannot resolve or replay the operation."
        : string.Empty;

    public bool HasRecoveryGuidance => !string.IsNullOrEmpty(RecoveryGuidance);

    public string RecoveryEvidenceSummary => Operation?.Status == "recovery_required"
        ? $"Operation key: {Operation.OperationId}\n"
          + $"Evidence record: {Operation.Id}\n"
          + $"Run: {Operation.RunId}\n"
          + $"Preflight: {Operation.PreflightId}\n"
          + $"Review revision: {Operation.ReviewRevision}\n"
          + $"Status: {Operation.Status}\n"
          + $"Recycled: {Operation.RecycledCount}; failed: {Operation.FailedCount}; cancelled: {Operation.CancelledCount}; "
          + $"unknown: {Operation.UnknownCount}; pending: {Operation.PendingResultCount}\n"
          + $"Error code: {Operation.ErrorCode ?? "none recorded"}"
        : string.Empty;

    public string RevisionStatus => Operation is null || Operation.IsCurrent
        ? string.Empty
        : $"This operation is bound to review revision {Operation.ReviewRevision:N0}; the current revision is {Operation.CurrentReviewRevision:N0}.";

    public string PageStatus => Items.Count == 0
        ? "No operation item details on this page."
        : $"Operation item page {_pageIndex + 1:N0}, showing {Items.Count:N0} details.";

    public async Task ShowRunAsync(WorkerRun? run, CancellationToken cancellationToken = default)
    {
        CancelLifetime();
        _lifetime = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var token = _lifetime.Token;
        var generation = ++_generation;
        Operation = null;
        ErrorMessage = null;
        ResetPages();
        if (run?.Status != "completed" || _worker is null)
        {
            return;
        }

        IsLoading = true;
        try
        {
            var operation = await _worker.GetLatestRecycleOperationAsync(run.Id, token);
            if (!IsCurrentGeneration(generation, token))
            {
                return;
            }
            Operation = operation;
            if (operation is not null)
            {
                await LoadPageAsync(null, generation, token);
                if (IsCurrentGeneration(generation, token))
                {
                    Announcement = $"Recycle Bin operation reconstructed. {ProgressSummary} {BoundaryNotice}";
                    AnnouncementVersion++;
                }
            }
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (IsCurrentGeneration(generation, token))
            {
                ErrorMessage = exception.Message;
                Announcement = $"Recycle Bin operation error. {exception.Message}";
                AnnouncementVersion++;
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

    public void Dispose()
    {
        CancelLifetime();
        GC.SuppressFinalize(this);
    }

    private async Task NextPageAsync()
    {
        if (_lifetime is null || string.IsNullOrEmpty(_nextCursor))
        {
            return;
        }
        _history.Add(_cursor);
        await LoadPageAsync(_nextCursor, _generation, _lifetime.Token);
    }

    private async Task PreviousPageAsync()
    {
        if (_lifetime is null || _history.Count == 0)
        {
            return;
        }
        var previous = _history[^1];
        _history.RemoveAt(_history.Count - 1);
        await LoadPageAsync(previous, _generation, _lifetime.Token);
    }

    private async Task LoadPageAsync(string? cursor, long generation, CancellationToken token)
    {
        if (_worker is null || Operation is null)
        {
            return;
        }
        var operationId = Operation.Id;
        IsLoading = true;
        try
        {
            if (!_cache.TryGet(cursor, out var page))
            {
                page = await _worker.GetRecycleOperationItemsAsync(
                    new RecycleOperationItemQuery(operationId, PageSize, null, cursor), token);
                if (!IsCurrentGeneration(generation, token) || Operation?.Id != operationId)
                {
                    return;
                }
                _cache.Set(cursor, page);
            }
            if (!IsCurrentGeneration(generation, token) || Operation?.Id != operationId)
            {
                return;
            }
            Items.Clear();
            foreach (var item in page.Items)
            {
                Items.Add(new RecycleOperationItemViewModel(item));
            }
            _cursor = cursor;
            _nextCursor = page.NextCursor;
            _pageIndex = _history.Count;
            NotifyStateChanged();
        }
        finally
        {
            if (generation == _generation)
            {
                IsLoading = false;
            }
        }
    }

    private bool IsCurrentGeneration(long generation, CancellationToken token) =>
        generation == _generation && !token.IsCancellationRequested;

    private void ResetPages()
    {
        _cache.Clear();
        _history.Clear();
        Items.Clear();
        _cursor = null;
        _nextCursor = null;
        _pageIndex = 0;
        NotifyStateChanged();
    }

    private void NotifyStateChanged()
    {
        foreach (var property in new[]
        {
            nameof(HasOperation), nameof(IsExecutorEnabled), nameof(CanSubmit), nameof(CanMoveNext),
            nameof(CanMovePrevious), nameof(BoundaryNotice), nameof(ConfirmationSummary),
            nameof(ProgressSummary), nameof(CancellationDisclosure), nameof(RecoveryGuidance),
            nameof(HasRecoveryGuidance), nameof(RecoveryEvidenceSummary), nameof(RevisionStatus),
            nameof(PageStatus),
        })
        {
            OnPropertyChanged(property);
        }
        NextPageCommand.NotifyCanExecuteChanged();
        PreviousPageCommand.NotifyCanExecuteChanged();
    }

    private void CancelLifetime()
    {
        _lifetime?.Cancel();
        _lifetime?.Dispose();
        _lifetime = null;
    }

    private static string StatusText(string status) => status switch
    {
        "prepared" => "Prepared",
        "awaiting_confirmation" => "Awaiting final confirmation",
        "submitted" => "Submitted",
        "executing" => "Processing",
        "cancelling" => "Cancelling",
        "expired" => "Expired",
        "cancelled" => "Cancelled",
        "completed" => "Completed",
        "partially_completed" => "Partially completed",
        "failed" => "Failed",
        "recovery_required" => "Recovery required",
        _ => status,
    };
}

public sealed class RecycleOperationItemViewModel(WorkerRecycleOperationItem item)
{
    public WorkerRecycleOperationItem Item { get; } = item;

    public string Size => DisplayFormatting.Bytes(Item.PlannedBytes);

    public string Status => Item.ResultStatus == "pending"
        ? Item.EligibilityStatus.Replace('_', ' ')
        : Item.ResultStatus.Replace('_', ' ');

    public string Explanation => Item.ResultCode ?? Item.EligibilityCode ?? "No result has been recorded.";
}
