using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class RecoveryReviewViewModel : ObservableObject, IDisposable
{
    private const int PageSize = 100;
    private const int MaximumCachedPages = 5;
    private readonly IRecycleOperationWorkerClient? _worker;
    private readonly IClipboardService? _clipboard;
    private readonly IRecycleBinService? _recycleBin;
    private readonly Func<Task>? _navigateToFreshScan;
    private readonly BoundedCursorCache<WorkerRecoveryReviewObservationPage> _cache = new(MaximumCachedPages);
    private readonly List<string?> _history = [];
    private CancellationTokenSource? _lifetime;
    private WorkerRecycleOperation? _operation;
    private WorkerRecoveryReview? _review;
    private RecycleOperationItemViewModel? _selectedUnknownItem;
    private RecoveryReviewObservationViewModel? _selectedHistoryObservation;
    private RecoveryReviewObservationViewModel? _supersededObservation;
    private RecoveryReviewObservationViewModel? _lastRecordedObservation;
    private RecoveryReviewObservationChoice? _selectedObservationChoice;
    private string _note = string.Empty;
    private string _correctionReason = string.Empty;
    private string? _cursor;
    private string? _nextCursor;
    private string? _failedCursor;
    private int? _failedPageIndex;
    private RecoveryReviewObservationRecord? _failedMutation;
    private int _pageIndex;
    private long _totalCount;
    private long _generation;
    private bool _isLoading;
    private bool _isMutating;
    private string? _readErrorMessage;
    private string? _mutationErrorMessage;
    private string _announcement = string.Empty;
    private long _announcementVersion;
    private string _errorAnnouncement = string.Empty;
    private long _errorAnnouncementVersion;
    private string _focusTarget = string.Empty;
    private long _focusRequestVersion;

    public RecoveryReviewViewModel(
        IWorkerClient worker,
        IClipboardService? clipboard = null,
        IRecycleBinService? recycleBin = null,
        Func<Task>? navigateToFreshScan = null)
    {
        _worker = worker as IRecycleOperationWorkerClient;
        _clipboard = clipboard;
        _recycleBin = recycleBin;
        _navigateToFreshScan = navigateToFreshScan;
        RecordObservationCommand = new AsyncRelayCommand(RecordObservationAsync, () => CanRecordObservation);
        RetryReadCommand = new AsyncRelayCommand(RetryReadAsync, () => CanRetryRead);
        RetryMutationCommand = new AsyncRelayCommand(RetryMutationAsync, () => CanRetryMutation);
        NextHistoryPageCommand = new AsyncRelayCommand(NextHistoryPageAsync, () => CanMoveHistoryNext);
        PreviousHistoryPageCommand = new AsyncRelayCommand(PreviousHistoryPageAsync, () => CanMoveHistoryPrevious);
        BeginCorrectionCommand = new RelayCommand(BeginCorrection, () => CanBeginCorrection);
        CancelCorrectionCommand = new RelayCommand(CancelCorrection, () => IsCorrection);
        CopyEvidenceCommand = new RelayCommand(CopyEvidence, () => CanCopySelectedItem);
        CopyPathCommand = new RelayCommand(CopyPath, () => CanCopySelectedItem);
        CopyReviewSummaryCommand = new RelayCommand(CopyReviewSummary, () => HasReview);
        OpenRecycleBinCommand = new AsyncRelayCommand(OpenRecycleBinAsync, () => CanOpenRecycleBin);
        NavigateToFreshScanCommand = new AsyncRelayCommand(NavigateToFreshScanAsync, () => IsVisible);
    }

    public static IReadOnlyList<RecoveryReviewObservationChoice> ObservationChoices { get; } =
    [
        new("observed_in_recycle_bin", "Observed in Recycle Bin", "A corresponding Recycle Bin entry was identified manually."),
        new("observed_at_source", "Observed at source", "The expected source item was identified manually."),
        new("observed_in_both", "Observed in both", "Both were observed; replacement, copy, or alias ambiguity remains."),
        new("observed_in_neither", "Observed in neither", "Neither was observed; this does not prove deletion or recycling."),
        new("deferred_unresolved", "Deferred unresolved", "Inspection was unavailable or deliberately deferred."),
    ];

    public ObservableCollection<RecoveryReviewObservationViewModel> Observations { get; } = [];

    public IAsyncRelayCommand RecordObservationCommand { get; }

    public IAsyncRelayCommand RetryReadCommand { get; }

    public IAsyncRelayCommand RetryMutationCommand { get; }

    public IAsyncRelayCommand NextHistoryPageCommand { get; }

    public IAsyncRelayCommand PreviousHistoryPageCommand { get; }

    public IRelayCommand BeginCorrectionCommand { get; }

    public IRelayCommand CancelCorrectionCommand { get; }

    public IRelayCommand CopyEvidenceCommand { get; }

    public IRelayCommand CopyPathCommand { get; }

    public IRelayCommand CopyReviewSummaryCommand { get; }

    public IAsyncRelayCommand OpenRecycleBinCommand { get; }

    public IAsyncRelayCommand NavigateToFreshScanCommand { get; }

    public WorkerRecoveryReview? Review
    {
        get => _review;
        private set
        {
            if (SetProperty(ref _review, value))
            {
                NotifyStateChanged();
            }
        }
    }

    public RecycleOperationItemViewModel? SelectedUnknownItem
    {
        get => _selectedUnknownItem;
        set
        {
            if (SetProperty(ref _selectedUnknownItem, value))
            {
                if (IsCorrection && value?.Item.Id != SupersededObservation?.Observation.ItemId)
                {
                    CancelCorrection();
                }
                NotifyStateChanged();
            }
        }
    }

    public RecoveryReviewObservationViewModel? SelectedHistoryObservation
    {
        get => _selectedHistoryObservation;
        set
        {
            if (SetProperty(ref _selectedHistoryObservation, value))
            {
                NotifyStateChanged();
            }
        }
    }

    public RecoveryReviewObservationViewModel? SupersededObservation
    {
        get => _supersededObservation;
        private set
        {
            if (SetProperty(ref _supersededObservation, value))
            {
                NotifyStateChanged();
            }
        }
    }

    public RecoveryReviewObservationViewModel? LastRecordedObservation
    {
        get => _lastRecordedObservation;
        private set
        {
            if (SetProperty(ref _lastRecordedObservation, value))
            {
                NotifyStateChanged();
            }
        }
    }

    public RecoveryReviewObservationChoice? SelectedObservationChoice
    {
        get => _selectedObservationChoice;
        set
        {
            if (SetProperty(ref _selectedObservationChoice, value))
            {
                NotifyStateChanged();
            }
        }
    }

    public string Note
    {
        get => _note;
        set
        {
            if (SetProperty(ref _note, value))
            {
                NotifyStateChanged();
            }
        }
    }

    public string CorrectionReason
    {
        get => _correctionReason;
        set
        {
            if (SetProperty(ref _correctionReason, value))
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

    public bool IsMutating
    {
        get => _isMutating;
        private set
        {
            if (SetProperty(ref _isMutating, value))
            {
                NotifyStateChanged();
            }
        }
    }

    public string? ReadErrorMessage
    {
        get => _readErrorMessage;
        private set
        {
            if (SetProperty(ref _readErrorMessage, value))
            {
                NotifyStateChanged();
            }
        }
    }

    public string? MutationErrorMessage
    {
        get => _mutationErrorMessage;
        private set
        {
            if (SetProperty(ref _mutationErrorMessage, value))
            {
                NotifyStateChanged();
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

    public bool IsVisible => _operation?.Status == "recovery_required";

    public bool HasReview => Review is not null;

    public bool HasReadError => !string.IsNullOrWhiteSpace(ReadErrorMessage);

    public bool HasMutationError => !string.IsNullOrWhiteSpace(MutationErrorMessage);

    public bool IsCorrection => SupersededObservation is not null;

    public bool HasLastRecordedObservation => LastRecordedObservation is not null;

    public bool CanEdit => IsVisible
        && HasReview
        && !HasReadError
        && !IsLoading
        && !IsMutating
        && _failedMutation is null;

    public bool CanRecordObservation => CanEdit
        && SelectedUnknownItem is not null
        && SelectedObservationChoice is not null
        && Note.Length <= 1_000
        && (!IsCorrection || !string.IsNullOrWhiteSpace(CorrectionReason))
        && CorrectionReason.Length <= 500;

    public bool CanBeginCorrection => CanEdit
        && SelectedHistoryObservation?.Observation.IsCurrent == true
        && SelectedUnknownItem?.Item.Id == SelectedHistoryObservation.Observation.ItemId;

    public bool CanRetryRead => IsVisible && !IsLoading && (_failedPageIndex is not null || Review is null);

    public bool CanRetryMutation => IsVisible && !IsMutating && _failedMutation is not null;

    public bool CanMoveHistoryNext => !IsLoading && !string.IsNullOrEmpty(_nextCursor);

    public bool CanMoveHistoryPrevious => !IsLoading && _pageIndex > 0;

    public bool CanCopySelectedItem => SelectedUnknownItem is not null && _clipboard is not null;

    public bool CanOpenRecycleBin => IsVisible && _recycleBin is not null;

    public string ReviewStatus => Review is null
        ? "Recovery review status is unavailable."
        : Review.State switch
        {
            "not_started" => $"Not started. 0 of {Review.UnknownItemCount:N0} unknown items have a current operator observation.",
            "in_progress" => $"In progress. {Review.ObservedItemCount:N0} of {Review.UnknownItemCount:N0} unknown items have a current operator observation.",
            "review_complete_with_unresolved_evidence" => $"Review complete with unresolved evidence. All {Review.UnknownItemCount:N0} unknown items have a current operator observation; original Shell evidence remains unknown.",
            _ => Review.State,
        };

    public string ReviewBoundary =>
        "Record only what you independently observed. The app does not inspect the source, provider, content, or Recycle Bin; observations never change the original unknown, ambiguous, or recovery-required evidence.";

    public string HistoryPageStatus => Observations.Count == 0
        ? "No recovery-review observation history on this page."
        : $"Observation history page {_pageIndex + 1:N0}, showing records {PageStart:N0}-{PageEnd:N0} of {_totalCount:N0}.";

    public string CorrectionSummary => SupersededObservation is null
        ? string.Empty
        : $"Append a correction for observation {SupersededObservation.Observation.Id}: {SupersededObservation.DisplayObservation}. The prior record will remain visible.";

    public string LastRecordedSummary => LastRecordedObservation is null
        ? string.Empty
        : $"Last appended record: {LastRecordedObservation.AutomationName}";

    public string SelectedEvidence => SelectedUnknownItem?.EvidenceDetails ?? string.Empty;

    private long PageStart => ((long)_pageIndex * PageSize) + 1;

    private long PageEnd => PageStart + Observations.Count - 1;

    public async Task ShowOperationAsync(
        WorkerRecycleOperation? operation,
        CancellationToken cancellationToken = default)
    {
        CancelLifetime();
        _lifetime = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var token = _lifetime.Token;
        var generation = ++_generation;
        _operation = operation?.Status == "recovery_required" ? operation : null;
        Review = null;
        SelectedUnknownItem = null;
        SelectedHistoryObservation = null;
        SupersededObservation = null;
        LastRecordedObservation = null;
        SelectedObservationChoice = null;
        Note = string.Empty;
        CorrectionReason = string.Empty;
        ReadErrorMessage = null;
        MutationErrorMessage = null;
        Announcement = string.Empty;
        ErrorAnnouncement = string.Empty;
        _failedMutation = null;
        IsLoading = false;
        IsMutating = false;
        ResetHistory();
        NotifyStateChanged();
        if (_operation is null || _worker is null)
        {
            return;
        }
        await LoadReviewAsync(_operation.Id, generation, token);
    }

    public void Dispose()
    {
        CancelLifetime();
        GC.SuppressFinalize(this);
    }

    private async Task LoadReviewAsync(long operationId, long generation, CancellationToken token)
    {
        IsLoading = true;
        try
        {
            var result = await _worker!.GetRecoveryReviewAsync(operationId, token);
            if (!IsCurrent(generation, token, operationId))
            {
                return;
            }
            Review = result.Review;
            ReadErrorMessage = null;
            ErrorAnnouncement = string.Empty;
            if (await TryLoadHistoryPageAsync(null, 0, generation, token))
            {
                PublishSuccess($"Recovery review reconstructed. {ReviewStatus} {HistoryPageStatus}");
            }
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (IsCurrent(generation, token, operationId))
            {
                PublishReadError(exception.Message);
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

    private async Task<bool> TryLoadHistoryPageAsync(
        string? cursor,
        int pageIndex,
        long generation,
        CancellationToken token)
    {
        if (_operation is null || _worker is null)
        {
            return false;
        }
        var operationId = _operation.Id;
        IsLoading = true;
        try
        {
            if (!_cache.TryGet(cursor, out var page))
            {
                page = await _worker.GetRecoveryReviewObservationsAsync(
                    new RecoveryReviewObservationQuery(operationId, PageSize, false, cursor), token);
                if (!IsCurrent(generation, token, operationId))
                {
                    return false;
                }
                _cache.Set(cursor, page);
            }
            if (!IsCurrent(generation, token, operationId))
            {
                return false;
            }
            Observations.Clear();
            foreach (var observation in page.Observations)
            {
                Observations.Add(new RecoveryReviewObservationViewModel(observation));
            }
            _cursor = cursor;
            _nextCursor = page.NextCursor;
            _pageIndex = pageIndex;
            _totalCount = page.Total;
            _failedCursor = null;
            _failedPageIndex = null;
            ReadErrorMessage = null;
            ErrorAnnouncement = string.Empty;
            NotifyStateChanged();
            return true;
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
            return false;
        }
        catch (Exception exception)
        {
            if (IsCurrent(generation, token, operationId))
            {
                _failedCursor = cursor;
                _failedPageIndex = pageIndex;
                PublishReadError(exception.Message);
            }
            return false;
        }
        finally
        {
            if (generation == _generation)
            {
                IsLoading = false;
            }
        }
    }

    private async Task NextHistoryPageAsync()
    {
        if (_lifetime is null || string.IsNullOrEmpty(_nextCursor))
        {
            return;
        }
        var committedCursor = _cursor;
        if (await TryLoadHistoryPageAsync(_nextCursor, _pageIndex + 1, _generation, _lifetime.Token))
        {
            _history.Add(committedCursor);
            PublishSuccess($"Recovery review history loaded. {HistoryPageStatus}");
            RequestFocus("history");
        }
    }

    private async Task PreviousHistoryPageAsync()
    {
        if (_lifetime is null || _history.Count == 0)
        {
            return;
        }
        var previous = _history[^1];
        if (await TryLoadHistoryPageAsync(previous, _pageIndex - 1, _generation, _lifetime.Token))
        {
            _history.RemoveAt(_history.Count - 1);
            PublishSuccess($"Recovery review history loaded. {HistoryPageStatus}");
            RequestFocus("history");
        }
    }

    private async Task RetryReadAsync()
    {
        if (_operation is null || _lifetime is null)
        {
            return;
        }
        if (Review is null)
        {
            await LoadReviewAsync(_operation.Id, _generation, _lifetime.Token);
            return;
        }
        if (_failedPageIndex is not int failedPageIndex)
        {
            return;
        }
        var failedCursor = _failedCursor;
        var committedCursor = _cursor;
        var committedPageIndex = _pageIndex;
        if (await TryLoadHistoryPageAsync(failedCursor, failedPageIndex, _generation, _lifetime.Token))
        {
            if (failedPageIndex == committedPageIndex + 1)
            {
                _history.Add(committedCursor);
            }
            else if (failedPageIndex == committedPageIndex - 1 && _history.Count > 0)
            {
                _history.RemoveAt(_history.Count - 1);
            }
            PublishSuccess($"Recovery review history loaded. {HistoryPageStatus}");
            RequestFocus("history");
        }
    }

    private async Task RecordObservationAsync()
    {
        if (_operation is null || SelectedUnknownItem is null || SelectedObservationChoice is null)
        {
            return;
        }
        var record = new RecoveryReviewObservationRecord(
            Guid.NewGuid().ToString("N"),
            _operation.Id,
            SelectedUnknownItem.Item.Id,
            SelectedObservationChoice.Value,
            DateTimeOffset.UtcNow.ToString("O"),
            NormalizeOptional(Note),
            1,
            SupersededObservation?.Observation.Id,
            IsCorrection ? NormalizeOptional(CorrectionReason) : null);
        await SubmitMutationAsync(record);
    }

    private async Task RetryMutationAsync()
    {
        if (_failedMutation is not null)
        {
            await SubmitMutationAsync(_failedMutation);
        }
    }

    private async Task SubmitMutationAsync(RecoveryReviewObservationRecord record)
    {
        if (_worker is null || _lifetime is null || _operation is null)
        {
            return;
        }
        var operationId = _operation.Id;
        var generation = _generation;
        var token = _lifetime.Token;
        IsMutating = true;
        try
        {
            var result = await _worker.RecordRecoveryReviewObservationAsync(record, token);
            if (!IsCurrent(generation, token, operationId))
            {
                return;
            }
            Review = result.Review;
            LastRecordedObservation = new RecoveryReviewObservationViewModel(result.Observation);
            _failedMutation = null;
            MutationErrorMessage = null;
            ErrorAnnouncement = string.Empty;
            var previousId = result.Observation.SupersedesObservationId;
            SelectedObservationChoice = null;
            Note = string.Empty;
            CorrectionReason = string.Empty;
            SupersededObservation = null;
            _cache.Clear();
            await TryLoadHistoryPageAsync(_cursor, _pageIndex, generation, token);
            PublishSuccess(previousId is long prior
                ? $"Recovery observation {result.Observation.Id:N0} appended as a correction to observation {prior:N0}. {ReviewStatus}"
                : $"Recovery observation {result.Observation.Id:N0} recorded. {ReviewStatus}");
            RequestFocus("status");
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (IsCurrent(generation, token, operationId))
            {
                _failedMutation = record;
                MutationErrorMessage = exception.Message;
                PublishError($"Recovery review mutation error. {exception.Message}");
            }
        }
        finally
        {
            if (generation == _generation)
            {
                IsMutating = false;
            }
        }
    }

    private void BeginCorrection()
    {
        if (SelectedHistoryObservation?.Observation.IsCurrent != true)
        {
            return;
        }
        SupersededObservation = SelectedHistoryObservation;
        SelectedObservationChoice = null;
        CorrectionReason = string.Empty;
        RequestFocus("observation-kind");
    }

    private void CancelCorrection()
    {
        SupersededObservation = null;
        CorrectionReason = string.Empty;
    }

    private void CopyEvidence()
    {
        if (SelectedUnknownItem is null || _clipboard is null)
        {
            return;
        }
        _clipboard.CopyText(SelectedUnknownItem.EvidenceDetails);
        PublishSuccess($"Durable evidence for operation item {SelectedUnknownItem.Item.Id:N0} copied.");
    }

    private void CopyPath()
    {
        if (SelectedUnknownItem is null || _clipboard is null)
        {
            return;
        }
        _clipboard.CopyText(SelectedUnknownItem.Item.Path);
        PublishSuccess($"Stored source path for operation item {SelectedUnknownItem.Item.Id:N0} copied for independent inspection.");
    }

    private void CopyReviewSummary()
    {
        if (_clipboard is null || Review is null)
        {
            return;
        }
        _clipboard.CopyText($"Recovery operation {_operation?.Id}; {ReviewStatus} {ReviewBoundary}");
        PublishSuccess("Recovery review status copied.");
    }

    private async Task OpenRecycleBinAsync()
    {
        if (_recycleBin is null)
        {
            return;
        }
        try
        {
            await _recycleBin.OpenAsync(_lifetime?.Token ?? CancellationToken.None);
            PublishSuccess("Windows Recycle Bin opened for independent manual inspection.");
        }
        catch (OperationCanceledException) when (_lifetime?.IsCancellationRequested == true)
        {
        }
        catch (Exception exception)
        {
            PublishError($"Recycle Bin navigation error. {exception.Message}");
        }
    }

    private async Task NavigateToFreshScanAsync()
    {
        if (_navigateToFreshScan is null)
        {
            return;
        }
        await _navigateToFreshScan();
        PublishSuccess("Navigated to start a fresh scan. No prior operation work was retried or copied forward.");
    }

    private void PublishReadError(string message)
    {
        ReadErrorMessage = message;
        PublishError($"Recovery review read error. {message}");
    }

    private void PublishSuccess(string message)
    {
        Announcement = message;
        AnnouncementVersion++;
    }

    private void PublishError(string message)
    {
        ErrorAnnouncement = message;
        ErrorAnnouncementVersion++;
    }

    private void RequestFocus(string target)
    {
        FocusTarget = target;
        FocusRequestVersion++;
    }

    private bool IsCurrent(long generation, CancellationToken token, long operationId) =>
        generation == _generation && !token.IsCancellationRequested && _operation?.Id == operationId;

    private void ResetHistory()
    {
        _cache.Clear();
        _history.Clear();
        Observations.Clear();
        _cursor = null;
        _nextCursor = null;
        _failedCursor = null;
        _failedPageIndex = null;
        _pageIndex = 0;
        _totalCount = 0;
        NotifyStateChanged();
    }

    private void NotifyStateChanged()
    {
        foreach (var property in new[]
        {
            nameof(IsVisible), nameof(HasReview), nameof(HasReadError), nameof(HasMutationError),
            nameof(IsCorrection), nameof(HasLastRecordedObservation), nameof(CanEdit), nameof(CanRecordObservation),
            nameof(CanBeginCorrection), nameof(CanRetryRead), nameof(CanRetryMutation),
            nameof(CanMoveHistoryNext), nameof(CanMoveHistoryPrevious), nameof(CanCopySelectedItem),
            nameof(CanOpenRecycleBin), nameof(ReviewStatus), nameof(HistoryPageStatus),
            nameof(CorrectionSummary), nameof(LastRecordedSummary), nameof(SelectedEvidence),
        })
        {
            OnPropertyChanged(property);
        }
        RecordObservationCommand.NotifyCanExecuteChanged();
        RetryReadCommand.NotifyCanExecuteChanged();
        RetryMutationCommand.NotifyCanExecuteChanged();
        NextHistoryPageCommand.NotifyCanExecuteChanged();
        PreviousHistoryPageCommand.NotifyCanExecuteChanged();
        BeginCorrectionCommand.NotifyCanExecuteChanged();
        CancelCorrectionCommand.NotifyCanExecuteChanged();
        CopyEvidenceCommand.NotifyCanExecuteChanged();
        CopyPathCommand.NotifyCanExecuteChanged();
        CopyReviewSummaryCommand.NotifyCanExecuteChanged();
        OpenRecycleBinCommand.NotifyCanExecuteChanged();
        NavigateToFreshScanCommand.NotifyCanExecuteChanged();
    }

    private void CancelLifetime()
    {
        _lifetime?.Cancel();
        _lifetime?.Dispose();
        _lifetime = null;
    }

    private static string? NormalizeOptional(string value) =>
        string.IsNullOrWhiteSpace(value) ? null : value.Trim();
}

public sealed record RecoveryReviewObservationChoice(string Value, string Label, string Explanation)
{
    public string AutomationName => $"{Label}. {Explanation}";
}

public sealed class RecoveryReviewObservationViewModel
{
    public RecoveryReviewObservationViewModel(WorkerRecoveryReviewObservation observation)
    {
        Observation = observation;
    }

    public WorkerRecoveryReviewObservation Observation { get; }

    public string DisplayObservation => Observation.Observation switch
    {
        "observed_in_recycle_bin" => "Observed in Recycle Bin",
        "observed_at_source" => "Observed at source",
        "observed_in_both" => "Observed in both",
        "observed_in_neither" => "Observed in neither",
        "deferred_unresolved" => "Deferred unresolved",
        _ => Observation.Observation,
    };

    public string CurrentStatus => Observation.IsCurrent
        ? "Current observation"
        : $"Superseded by observation {Observation.SupersededByObservationId?.ToString() ?? "unknown"}";

    public string Correction => Observation.SupersedesObservationId is long prior
        ? $"Corrects observation {prior}; reason: {Observation.CorrectionReason ?? "none recorded"}."
        : "Initial observation.";

    public string AutomationName =>
        $"Observation {Observation.Id}; operation item {Observation.ItemId}; {DisplayObservation}; {CurrentStatus}; "
        + $"observed at {Observation.ObservedAt}; {Correction} Note: {Observation.Note ?? "none recorded"}.";
}
