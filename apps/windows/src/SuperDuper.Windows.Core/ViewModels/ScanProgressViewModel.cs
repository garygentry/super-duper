using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class ScanProgressViewModel : ObservableObject, IDisposable
{
    private readonly IWorkerClient _workerClient;
    private readonly IUiDispatcher _dispatcher;
    private readonly Timer _elapsedTimer;
    private WorkerRun? _run;
    private ulong _lastSequence;
    private string? _currentPath;
    private string? _message;
    private string? _errorMessage;
    private bool _cancelRequestPending;

    public ScanProgressViewModel(IWorkerClient workerClient, IUiDispatcher dispatcher)
    {
        _workerClient = workerClient;
        _dispatcher = dispatcher;
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
        CurrentPath = null;
        Message = null;
        ErrorMessage = null;
        _cancelRequestPending = false;
        Run = run;
        UpdateTimer();
    }

    public void ApplyProgress(WorkerRunProgressEventArgs progress)
    {
        if (Run?.Id != progress.RunId || progress.Sequence <= _lastSequence)
        {
            return;
        }
        _lastSequence = progress.Sequence;
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
        UpdateTimer();
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

    private async Task CancelAsync()
    {
        if (Run is not { Status: "running" } run)
        {
            return;
        }
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
        OnPropertyChanged(nameof(Elapsed));
        CancelCommand.NotifyCanExecuteChanged();
    }
}
