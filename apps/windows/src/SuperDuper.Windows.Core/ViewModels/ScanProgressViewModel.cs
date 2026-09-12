using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class ScanProgressViewModel : ObservableObject, IDisposable
{
    private const ulong ProgressAnnouncementIntervalNanos = 5_000_000_000;
    private readonly IWorkerClient _workerClient;
    private readonly IUiDispatcher _dispatcher;
    private readonly Action<long>? _onCancelling;
    private readonly Func<WorkerRun, CancellationToken, Task>? _openWarnings;
    private readonly TimeProvider _clock;
    private readonly ITimer _elapsedTimer;
    private long? _lastUpdateTimestamp;
    private DateTimeOffset? _stoppedAt;
    private int _clockRefreshPending;
    private bool _disposed;
    private bool _timerActive;
    private WorkerRun? _run;
    private ulong _lastSequence;
    private ulong _lastProgressRevision;
    private IReadOnlyList<ulong>? _lastCumulativeValues;
    private WorkerScanProgressSnapshot? _progressSnapshot;
    private WorkerFolderAnalysisProgress? _folderAnalysis;
    private ulong? _lastAnnouncementMonotonicNanos;
    private string? _lastAnnouncementStatus;
    private string? _lastAnnouncementPhase;
    private long _progressAnnouncementVersion;
    private string? _currentPath;
    private string? _message;
    private string? _errorMessage;
    private bool _cancelRequestPending;

    public ScanProgressViewModel(
        IWorkerClient workerClient,
        IUiDispatcher dispatcher,
        Action<long>? onCancelling = null,
        Func<WorkerRun, CancellationToken, Task>? openWarnings = null,
        TimeProvider? clock = null)
    {
        _workerClient = workerClient;
        _dispatcher = dispatcher;
        _onCancelling = onCancelling;
        _openWarnings = openWarnings;
        _clock = clock ?? TimeProvider.System;
        _elapsedTimer = _clock.CreateTimer(
            _ => QueueClockRefresh(),
            null,
            Timeout.InfiniteTimeSpan,
            Timeout.InfiniteTimeSpan);
        CancelCommand = new AsyncRelayCommand(CancelAsync, () => CanCancel);
        OpenWarningsCommand = new AsyncRelayCommand(OpenWarningsAsync, () => CanOpenWarnings);
    }

    public WorkerRun? Run
    {
        get => _run;
        private set
        {
            if (SetProperty(ref _run, value))
            {
                RaiseRunProperties();
            }
        }
    }

    public string? CurrentPath
    {
        get => _currentPath;
        private set => SetProperty(ref _currentPath, value);
    }

    public string? Message
    {
        get => _message;
        private set => SetProperty(ref _message, value);
    }

    public string? ErrorMessage
    {
        get => _errorMessage;
        private set
        {
            if (SetProperty(ref _errorMessage, value))
            {
                OnPropertyChanged(nameof(HasError));
                OnPropertyChanged(nameof(DisplayErrorMessage));
            }
        }
    }

    public bool HasRun => Run is not null;

    public bool IsActive => Run?.Status is "pending" or "running" or "cancelling";

    public bool IsCancelling => Run?.Status == "cancelling" || _cancelRequestPending;

    public bool CanCancel => Run?.Status == "running" && !_cancelRequestPending;

    public string CancelButtonText => IsCancelling ? "Cancelling…" : "_Cancel scan";

    public string CancelAutomationName => IsCancelling
        ? "Scan cancellation requested"
        : "Cancel scan; access key Alt+C";

    public bool IsIndeterminate => IsActive;

    public string ActivityHeading => IsActive ? "Current activity" : "Last reported activity";

    public string ActivityPathAutomationName => IsActive ? "Current scan path" : "Last reported scan path";

    public string MetricsContext => IsActive
        ? "Measured values from the last accepted worker update. The path is sampled activity; an unchanged path can be a large file still being read."
        : "Historical metrics from the last accepted update; they may precede the final run totals. This scan has stopped.";

    public string ElapsedLabel => !IsActive && Run is { CompletedAt: null }
        ? "Elapsed at last observation" : "Run elapsed";

    public string UpdateFreshness
    {
        get
        {
            if (Run is null) return "No scan selected";
            if (!IsActive) return "No live updates — scan has stopped";
            if (_lastUpdateTimestamp is not { } received) return "Waiting for the first accepted worker update";
            var age = _clock.GetElapsedTime(received);
            if (age < TimeSpan.Zero) age = TimeSpan.Zero;
            var text = $"Last update {DisplayFormatting.Duration(age)} ago";
            return age >= TimeSpan.FromSeconds(30)
                ? text + ". No recent worker update; this alone does not indicate failure."
                : text;
        }
    }

    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage) || !string.IsNullOrWhiteSpace(Run?.ErrorMessage);

    public string? DisplayErrorMessage => ErrorMessage ?? Run?.ErrorMessage;

    public string Status => Run is null ? "No run selected" : DisplayFormatting.Status(Run.Status);

    public string Phase => Run switch
    {
        null => "—",
        { Status: "pending" or "running" or "cancelling" } run => DisplayFormatting.Phase(run.Phase),
        { } run => $"Last phase: {DisplayFormatting.Phase(run.Phase)}",
    };

    public string FilesDiscovered => (Run?.FilesDiscovered ?? 0).ToString("N0");

    public string BytesDiscovered => DisplayFormatting.Bytes(Run?.BytesDiscovered ?? "0");

    public string FilesHashed => (Run?.FilesHashed ?? 0).ToString("N0");

    public string WarningCount => (Run?.WarningCount ?? 0).ToString("N0");

    public bool CanOpenWarnings => Run?.WarningCount > 0 && _openWarnings is not null;

    public string WarningAutomationName => Run is { WarningCount: > 0 } run
        ? $"Review {run.WarningCount:N0} current warnings in bounded run history; access key Alt+W"
        : "No current warnings to review; access key Alt+W";

    public WorkerScanProgressSnapshot? ProgressSnapshot
    {
        get => _progressSnapshot;
        private set
        {
            if (SetProperty(ref _progressSnapshot, value))
            {
                RaiseProgressProperties();
            }
        }
    }

    public IReadOnlyList<ScanProgressStage> Stages =>
        ScanProgressProjection.Stages(ProgressSnapshot?.Funnel);

    public bool HasDetailedProgress => ProgressSnapshot is not null;

    public string DetailedProgressUnavailableMessage => Run?.Status switch
    {
        "completed" => "Live detailed progress is unavailable for this completed scan.",
        "cancelled" => "Live detailed progress is unavailable because the scan was cancelled.",
        "failed" or "interrupted" =>
            "Live detailed progress is unavailable because the scan ended before completion.",
        _ => "Waiting for the first detailed progress snapshot.",
    };

    public string ProgressPhaseElapsed => ScanProgressProjection.PhaseElapsed(ProgressSnapshot);

    public string FolderAnalysisProgress => ScanProgressProjection.FolderAnalysis(_folderAnalysis);

    public bool IsFolderAnalysis => Run?.Phase == "analyzing_folders";

    public string PartialRecentRate =>
        ScanProgressProjection.Rate(ProgressSnapshot?.PartialReadRates.Recent);

    public string PartialCumulativeRate =>
        ScanProgressProjection.Rate(ProgressSnapshot?.PartialReadRates.Cumulative);

    public string FullRecentRate =>
        ScanProgressProjection.Rate(ProgressSnapshot?.FullReadRates.Recent);

    public string FullCumulativeRate =>
        ScanProgressProjection.Rate(ProgressSnapshot?.FullReadRates.Cumulative);

    public string CacheEffectiveness =>
        ScanProgressProjection.Cache(ProgressSnapshot?.CacheHitRateBasisPoints);

    public string ActiveDevices => Run?.Status switch
    {
        "cancelling" => "Unavailable — cancellation is in progress",
        "completed" => "Unavailable — no active scan I/O",
        "cancelled" => "Unavailable — scan was cancelled",
        "failed" or "interrupted" => "Unavailable — scan ended before completion",
        _ => ScanProgressProjection.Devices(ProgressSnapshot?.ActiveDevices),
    };

    public string RemainingWork => Run?.Status switch
    {
        "cancelling" => "Unavailable — cancellation is in progress",
        "completed" => "Complete",
        "cancelled" => "Unavailable — scan was cancelled",
        "failed" or "interrupted" => "Unavailable — scan ended before completion",
        _ => ScanProgressProjection.Remaining(ProgressSnapshot?.RemainingKnownWork),
    };

    public string HashPipelineCandidateContext =>
        ScanProgressProjection.CandidateContext(ProgressSnapshot?.Funnel);

    public string EstimatedTimeRemaining => Run?.Status switch
    {
        "cancelling" => "Unavailable — cancellation is in progress",
        "completed" => "Complete",
        "cancelled" => "Unavailable — scan was cancelled",
        "failed" or "interrupted" => "Unavailable — scan ended before completion",
        _ => ScanProgressProjection.Eta(ProgressSnapshot?.Eta),
    };

    public string ProgressAnnouncement => !IsActive && Run is not null
        ? $"Scan {Status}. {Phase}. {WarningCount} warnings. {DisplayErrorMessage}"
        : ProgressSnapshot is not { } snapshot
        ? (IsCancelling ? "Scan cancellation requested. Waiting for the worker to stop."
            : Run is null ? string.Empty : $"Scan {Status}. {Phase}.")
        : $"Scan progress. {Status}. {Phase}. "
            + (IsFolderAnalysis ? $"{FolderAnalysisProgress}. " : string.Empty)
            + $"{snapshot.Funnel.Discovered.Files:N0} discovered; "
            + $"{snapshot.Funnel.PartialScreened.Files:N0} partial screened of "
            + $"{snapshot.Funnel.HashPipelineCandidates.Files:N0} hash candidates. "
            + $"{RemainingWork}. ETA: {EstimatedTimeRemaining}. {WarningCount} warnings.";

    public long ProgressAnnouncementVersion => _progressAnnouncementVersion;

    public string ExcludedSubtreeCount => (Run?.ExcludedSubtreeCount ?? 0).ToString("N0");

    public string Elapsed
    {
        get
        {
            if (Run is null)
            {
                return "—";
            }
            var started = Run.StartedAt ?? Run.CreatedAt;
            var end = Run.CompletedAt ?? _stoppedAt ?? _clock.GetUtcNow();
            var elapsed = end > started ? end - started : TimeSpan.Zero;
            return DisplayFormatting.Duration(elapsed);
        }
    }

    public IAsyncRelayCommand CancelCommand { get; }

    public IAsyncRelayCommand OpenWarningsCommand { get; }

    public void ShowRun(WorkerRun? run)
    {
        _lastSequence = 0;
        _lastProgressRevision = 0;
        _lastCumulativeValues = null;
        _lastAnnouncementMonotonicNanos = null;
        _lastAnnouncementStatus = null;
        _lastAnnouncementPhase = null;
        _folderAnalysis = null;
        _lastUpdateTimestamp = null;
        _stoppedAt = run is { Status: not ("pending" or "running" or "cancelling") }
            ? run.CompletedAt ?? _clock.GetUtcNow() : null;
        ProgressSnapshot = null;
        CurrentPath = null;
        Message = null;
        ErrorMessage = null;
        _cancelRequestPending = false;
        Run = run;
        RaiseClockProperties();
        UpdateTimer();
    }

    public bool ApplyProgress(WorkerRunProgressEventArgs progress, long? receivedTimestamp = null)
    {
        if (Run?.Id != progress.RunId
            || Run.Status is not ("pending" or "running" or "cancelling")
            || progress.Sequence <= _lastSequence
            || (Run.Status == "cancelling" && progress.Status == "running")
            || !WorkerProgressContract.TryValidate(progress, out _)
            || progress.Progress.Revision < _lastProgressRevision
            || !WorkerProgressContract.TryGetCumulativeValues(
                progress,
                out var cumulativeValues,
                out _)
            || (progress.Progress.Revision == _lastProgressRevision
                && (!FolderProgressAdvances(_folderAnalysis, progress.FolderAnalysis)
                    || !SameCumulativeValues(_lastCumulativeValues, cumulativeValues)))
            || HasRegression(_lastCumulativeValues, cumulativeValues))
        {
            return false;
        }
        _lastSequence = progress.Sequence;
        _lastProgressRevision = progress.Progress.Revision;
        _lastCumulativeValues = cumulativeValues.ToArray();
        _folderAnalysis = progress.FolderAnalysis;
        _lastUpdateTimestamp = receivedTimestamp ?? _clock.GetTimestamp();
        var phaseChanged = Run.Phase != progress.Phase
            || (ProgressSnapshot is { } previous && previous.Phase != progress.Progress.Phase);
        // The worker may retain a sampled path at a phase transition. Do not imply that the
        // next phase is still processing that file; a later sample can establish activity again.
        var path = phaseChanged && progress.CurrentPath == CurrentPath ? null : progress.CurrentPath;
        Run = Run with
        {
            Status = progress.Status,
            Phase = progress.Phase,
            FilesDiscovered = progress.FilesDiscovered,
            BytesDiscovered = progress.BytesDiscovered,
            FilesHashed = progress.FilesHashed,
            WarningCount = progress.WarningCount,
        };
        CurrentPath = path;
        Message = progress.Message;
        ProgressSnapshot = progress.Progress;
        OnPropertyChanged(nameof(FolderAnalysisProgress));
        OnPropertyChanged(nameof(IsFolderAnalysis));
        UpdateProgressAnnouncement(progress);
        RaiseClockProperties();
        UpdateTimer();
        return true;
    }

    public void ApplyLifecycle(WorkerRun run, long? receivedTimestamp = null, bool workerUpdate = true)
    {
        if (Run?.Id != run.Id
            || (!IsActive && run.Status is "pending" or "running" or "cancelling")
            || (Run.Status == "cancelling" && run.Status is "pending" or "running"))
        {
            return;
        }
        var changed = Run.Status != run.Status || Run.Phase != run.Phase;
        if (workerUpdate) _lastUpdateTimestamp = receivedTimestamp ?? _clock.GetTimestamp();
        if (Run.Phase != run.Phase)
        {
            CurrentPath = null;
            Message = null;
        }
        if (run.Status is not ("pending" or "running" or "cancelling"))
            _stoppedAt = run.CompletedAt ?? _stoppedAt ?? _clock.GetUtcNow();
        _cancelRequestPending = false;
        Run = run;
        ErrorMessage = run.ErrorMessage;
        RaiseClockProperties();
        if (changed) AnnounceLifecycle();
        UpdateTimer();
    }

    public void Dispose()
    {
        _disposed = true;
        _elapsedTimer.Dispose();
    }

    private void QueueClockRefresh()
    {
        if (_disposed || Interlocked.Exchange(ref _clockRefreshPending, 1) != 0) return;
        _dispatcher.Post(() =>
        {
            Interlocked.Exchange(ref _clockRefreshPending, 0);
            if (!_disposed && IsActive) RaiseClockProperties();
        });
    }

    private void RaiseClockProperties()
    {
        OnPropertyChanged(nameof(Elapsed));
        OnPropertyChanged(nameof(UpdateFreshness));
    }

    private void AnnounceLifecycle()
    {
        _lastAnnouncementStatus = Run?.Status;
        _lastAnnouncementPhase = ProgressSnapshot?.Phase;
        OnPropertyChanged(nameof(ProgressAnnouncement));
        AdvanceAnnouncementVersion();
    }

    private static bool HasRegression(
        IReadOnlyList<ulong>? previous,
        IReadOnlyList<ulong> proposed)
    {
        if (previous is null)
        {
            return false;
        }
        if (previous.Count != proposed.Count)
        {
            return true;
        }
        for (var index = 0; index < previous.Count; index++)
        {
            if (proposed[index] < previous[index])
            {
                return true;
            }
        }
        return false;
    }

    private static bool SameCumulativeValues(
        IReadOnlyList<ulong>? previous,
        IReadOnlyList<ulong> proposed) =>
        previous is not null && previous.SequenceEqual(proposed);

    private static bool FolderProgressAdvances(
        WorkerFolderAnalysisProgress? previous,
        WorkerFolderAnalysisProgress? proposed)
    {
        if (previous is null || proposed is null)
        {
            return previous is null && proposed is not null;
        }
        var previousRank = FolderSubstageRank(previous.Substage);
        var proposedRank = FolderSubstageRank(proposed.Substage);
        return proposedRank > previousRank
            || (proposedRank == previousRank
                && proposed.Total == previous.Total
                && proposed.Completed > previous.Completed);
    }

    private static int FolderSubstageRank(string substage) => substage switch
    {
        "hierarchy" => 0,
        "structural_candidates" => 1,
        "verification" => 2,
        "persistence" => 3,
        _ => -1,
    };

    private async Task CancelAsync()
    {
        if (Run is not { Status: "running" } run)
        {
            return;
        }
        _onCancelling?.Invoke(run.Id);
        _cancelRequestPending = true;
        Run = run with { Status = "cancelling" };
        ErrorMessage = null;
        AnnounceLifecycle();
        try
        {
            ApplyLifecycle(await _workerClient.CancelRunAsync(run.Id));
        }
        catch (Exception exception)
        {
            if (Run?.Id == run.Id && IsActive)
            {
                _cancelRequestPending = false;
                Run = run;
                ErrorMessage = exception.Message;
                AnnounceLifecycle();
            }
        }
        finally
        {
            RaiseRunProperties();
        }
    }

    private Task OpenWarningsAsync(CancellationToken cancellationToken) =>
        Run is { WarningCount: > 0 } run && _openWarnings is not null
            ? _openWarnings(run, cancellationToken)
            : Task.CompletedTask;

    private void UpdateTimer()
    {
        if (_timerActive == IsActive) return;
        _timerActive = IsActive;
        _elapsedTimer.Change(
            IsActive ? TimeSpan.FromSeconds(1) : Timeout.InfiniteTimeSpan,
            IsActive ? TimeSpan.FromSeconds(1) : Timeout.InfiniteTimeSpan);
    }

    private void UpdateProgressAnnouncement(WorkerRunProgressEventArgs progress)
    {
        var currentNanos = progress.Progress.MonotonicNanos;
        var intervalElapsed = _lastAnnouncementMonotonicNanos is not { } previousNanos
            || (currentNanos >= previousNanos
                && currentNanos - previousNanos >= ProgressAnnouncementIntervalNanos);
        var statusChanged = !string.Equals(
            _lastAnnouncementStatus,
            progress.Status,
            StringComparison.Ordinal);
        var phaseChanged = !string.Equals(
            _lastAnnouncementPhase,
            progress.Progress.Phase,
            StringComparison.Ordinal);
        if (!intervalElapsed && !statusChanged && !phaseChanged)
        {
            return;
        }

        _lastAnnouncementMonotonicNanos = currentNanos;
        _lastAnnouncementStatus = progress.Status;
        _lastAnnouncementPhase = progress.Progress.Phase;
        AdvanceAnnouncementVersion();
    }

    private void AdvanceAnnouncementVersion()
    {
        _progressAnnouncementVersion = _progressAnnouncementVersion == long.MaxValue
            ? 1
            : _progressAnnouncementVersion + 1;
        OnPropertyChanged(nameof(ProgressAnnouncementVersion));
    }

    private void RaiseRunProperties()
    {
        OnPropertyChanged(nameof(HasRun));
        OnPropertyChanged(nameof(IsActive));
        OnPropertyChanged(nameof(IsCancelling));
        OnPropertyChanged(nameof(CanCancel));
        OnPropertyChanged(nameof(CancelButtonText));
        OnPropertyChanged(nameof(CancelAutomationName));
        OnPropertyChanged(nameof(IsIndeterminate));
        OnPropertyChanged(nameof(ActivityHeading));
        OnPropertyChanged(nameof(ActivityPathAutomationName));
        OnPropertyChanged(nameof(MetricsContext));
        OnPropertyChanged(nameof(ElapsedLabel));
        OnPropertyChanged(nameof(UpdateFreshness));
        OnPropertyChanged(nameof(HasError));
        OnPropertyChanged(nameof(DisplayErrorMessage));
        OnPropertyChanged(nameof(Status));
        OnPropertyChanged(nameof(Phase));
        OnPropertyChanged(nameof(FilesDiscovered));
        OnPropertyChanged(nameof(BytesDiscovered));
        OnPropertyChanged(nameof(FilesHashed));
        OnPropertyChanged(nameof(WarningCount));
        OnPropertyChanged(nameof(CanOpenWarnings));
        OnPropertyChanged(nameof(WarningAutomationName));
        OnPropertyChanged(nameof(ExcludedSubtreeCount));
        OnPropertyChanged(nameof(Elapsed));
        OnPropertyChanged(nameof(DetailedProgressUnavailableMessage));
        OnPropertyChanged(nameof(FolderAnalysisProgress));
        OnPropertyChanged(nameof(IsFolderAnalysis));
        OnPropertyChanged(nameof(ActiveDevices));
        OnPropertyChanged(nameof(RemainingWork));
        OnPropertyChanged(nameof(EstimatedTimeRemaining));
        OnPropertyChanged(nameof(ProgressAnnouncement));
        CancelCommand.NotifyCanExecuteChanged();
        OpenWarningsCommand.NotifyCanExecuteChanged();
    }

    private void RaiseProgressProperties()
    {
        OnPropertyChanged(nameof(Stages));
        OnPropertyChanged(nameof(HasDetailedProgress));
        OnPropertyChanged(nameof(DetailedProgressUnavailableMessage));
        OnPropertyChanged(nameof(ProgressPhaseElapsed));
        OnPropertyChanged(nameof(FolderAnalysisProgress));
        OnPropertyChanged(nameof(IsFolderAnalysis));
        OnPropertyChanged(nameof(PartialRecentRate));
        OnPropertyChanged(nameof(PartialCumulativeRate));
        OnPropertyChanged(nameof(FullRecentRate));
        OnPropertyChanged(nameof(FullCumulativeRate));
        OnPropertyChanged(nameof(CacheEffectiveness));
        OnPropertyChanged(nameof(ActiveDevices));
        OnPropertyChanged(nameof(RemainingWork));
        OnPropertyChanged(nameof(HashPipelineCandidateContext));
        OnPropertyChanged(nameof(EstimatedTimeRemaining));
        OnPropertyChanged(nameof(ProgressAnnouncement));
        OnPropertyChanged(nameof(ProgressAnnouncementVersion));
    }
}
