using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class ScanProgressViewModel : ObservableObject, IDisposable
{
    private readonly IWorkerClient _workerClient;
    private readonly IUiDispatcher _dispatcher;
    private readonly Action<long>? _onCancelling;
    private readonly Timer _elapsedTimer;
    private WorkerRun? _run;
    private ulong _lastSequence;
    private ulong _lastProgressRevision;
    private IReadOnlyList<ulong>? _lastCumulativeValues;
    private WorkerScanProgressSnapshot? _progressSnapshot;
    private string? _currentPath;
    private string? _message;
    private string? _errorMessage;
    private bool _cancelRequestPending;

    public ScanProgressViewModel(
        IWorkerClient workerClient,
        IUiDispatcher dispatcher,
        Action<long>? onCancelling = null)
    {
        _workerClient = workerClient;
        _dispatcher = dispatcher;
        _onCancelling = onCancelling;
        _elapsedTimer = new Timer(
            _ => _dispatcher.Post(() => OnPropertyChanged(nameof(Elapsed))),
            null,
            Timeout.InfiniteTimeSpan,
            Timeout.InfiniteTimeSpan);
        CancelCommand = new AsyncRelayCommand(CancelAsync, () => CanCancel);
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
            }
        }
    }

    public bool HasRun => Run is not null;

    public bool IsActive => Run?.Status is "pending" or "running" or "cancelling";

    public bool IsCancelling => Run?.Status == "cancelling" || _cancelRequestPending;

    public bool CanCancel => Run?.Status == "running" && !_cancelRequestPending;

    public bool IsIndeterminate => IsActive;

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

    public string ProgressPhaseElapsed => ScanProgressProjection.PhaseElapsed(ProgressSnapshot);

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

    public string ActiveDevices => ScanProgressProjection.Devices(ProgressSnapshot?.ActiveDevices);

    public string RemainingWork =>
        ScanProgressProjection.Remaining(ProgressSnapshot?.RemainingKnownWork);

    public string EstimatedTimeRemaining => ScanProgressProjection.Eta(ProgressSnapshot?.Eta);

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
            var end = Run.CompletedAt ?? DateTimeOffset.UtcNow;
            var elapsed = end > started ? end - started : TimeSpan.Zero;
            return elapsed.TotalHours >= 1
                ? elapsed.ToString(@"h\:mm\:ss")
                : elapsed.ToString(@"m\:ss");
        }
    }

    public IAsyncRelayCommand CancelCommand { get; }

    public void ShowRun(WorkerRun? run)
    {
        _lastSequence = 0;
        _lastProgressRevision = 0;
        _lastCumulativeValues = null;
        ProgressSnapshot = null;
        CurrentPath = null;
        Message = null;
        ErrorMessage = null;
        _cancelRequestPending = false;
        Run = run;
        UpdateTimer();
    }

    public bool ApplyProgress(WorkerRunProgressEventArgs progress)
    {
        if (Run?.Id != progress.RunId
            || Run.Status is not ("pending" or "running" or "cancelling")
            || progress.Sequence <= _lastSequence
            || (Run.Status == "cancelling" && progress.Status == "running")
            || !WorkerProgressContract.TryValidate(progress, out _)
            || progress.Progress.Revision <= _lastProgressRevision
            || !WorkerProgressContract.TryGetCumulativeValues(
                progress,
                out var cumulativeValues,
                out _)
            || HasRegression(_lastCumulativeValues, cumulativeValues))
        {
            return false;
        }
        _lastSequence = progress.Sequence;
        _lastProgressRevision = progress.Progress.Revision;
        _lastCumulativeValues = cumulativeValues.ToArray();
        Run = Run with
        {
            Status = progress.Status,
            Phase = progress.Phase,
            FilesDiscovered = progress.FilesDiscovered,
            BytesDiscovered = progress.BytesDiscovered,
            FilesHashed = progress.FilesHashed,
            WarningCount = progress.WarningCount,
        };
        CurrentPath = progress.CurrentPath;
        Message = progress.Message;
        ProgressSnapshot = progress.Progress;
        UpdateTimer();
        return true;
    }

    public void ApplyLifecycle(WorkerRun run)
    {
        if (Run?.Id != run.Id)
        {
            return;
        }
        _cancelRequestPending = false;
        Run = run;
        ErrorMessage = run.ErrorMessage;
        UpdateTimer();
    }

    public void Dispose() => _elapsedTimer.Dispose();

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
        try
        {
            ApplyLifecycle(await _workerClient.CancelRunAsync(run.Id));
        }
        catch (Exception exception)
        {
            _cancelRequestPending = false;
            Run = run;
            ErrorMessage = exception.Message;
        }
        finally
        {
            RaiseRunProperties();
        }
    }

    private void UpdateTimer() => _elapsedTimer.Change(
        IsActive ? TimeSpan.Zero : Timeout.InfiniteTimeSpan,
        IsActive ? TimeSpan.FromSeconds(1) : Timeout.InfiniteTimeSpan);

    private void RaiseRunProperties()
    {
        OnPropertyChanged(nameof(HasRun));
        OnPropertyChanged(nameof(IsActive));
        OnPropertyChanged(nameof(IsCancelling));
        OnPropertyChanged(nameof(CanCancel));
        OnPropertyChanged(nameof(IsIndeterminate));
        OnPropertyChanged(nameof(HasError));
        OnPropertyChanged(nameof(DisplayErrorMessage));
        OnPropertyChanged(nameof(Status));
        OnPropertyChanged(nameof(Phase));
        OnPropertyChanged(nameof(FilesDiscovered));
        OnPropertyChanged(nameof(BytesDiscovered));
        OnPropertyChanged(nameof(FilesHashed));
        OnPropertyChanged(nameof(WarningCount));
        OnPropertyChanged(nameof(ExcludedSubtreeCount));
        OnPropertyChanged(nameof(Elapsed));
        CancelCommand.NotifyCanExecuteChanged();
    }

    private void RaiseProgressProperties()
    {
        OnPropertyChanged(nameof(Stages));
        OnPropertyChanged(nameof(ProgressPhaseElapsed));
        OnPropertyChanged(nameof(PartialRecentRate));
        OnPropertyChanged(nameof(PartialCumulativeRate));
        OnPropertyChanged(nameof(FullRecentRate));
        OnPropertyChanged(nameof(FullCumulativeRate));
        OnPropertyChanged(nameof(CacheEffectiveness));
        OnPropertyChanged(nameof(ActiveDevices));
        OnPropertyChanged(nameof(RemainingWork));
        OnPropertyChanged(nameof(EstimatedTimeRemaining));
    }
}
