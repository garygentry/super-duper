using CommunityToolkit.Mvvm.ComponentModel;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class RunListItemViewModel : ObservableObject
{
    private WorkerRun _run;

    public RunListItemViewModel(WorkerRun run) => _run = run;

    public WorkerRun Run => _run;

    public long Id => _run.Id;

    public string Status => DisplayFormatting.Status(_run.Status);

    public string Phase => DisplayFormatting.Phase(_run.Phase);

    public string Started => (_run.StartedAt ?? _run.CreatedAt).ToLocalTime().ToString("g");

    public string Completed => _run.CompletedAt?.ToLocalTime().ToString("g") ?? "—";

    public string Duration => _run.StartedAt is { } started && _run.CompletedAt is { } completed
        ? DisplayFormatting.Duration(completed - started)
        : _run.StartedAt is not null ? "In progress" : "Not started";

    public string FilesDiscovered => _run.FilesDiscovered.ToString("N0");

    public string BytesDiscovered => DisplayFormatting.Bytes(_run.BytesDiscovered);

    public string DuplicateGroups => _run.DuplicateFileGroups.ToString("N0");

    public string WastedBytes => DisplayFormatting.Bytes(_run.WastedBytes);

    public string WarningCount => _run.WarningCount.ToString("N0");

    public string ExcludedSubtreeCount => _run.ExcludedSubtreeCount.ToString("N0");

    public string OutcomeSummary => _run.Status == "completed"
        ? $"{DuplicateGroups} exact file sets · {WastedBytes} potential savings · {WarningCount} warnings"
        : _run.Status is "pending" or "running" or "cancelling"
            ? $"{Phase} · {FilesDiscovered} files found · {WarningCount} warnings"
            : $"Completed results unavailable · {WarningCount} warnings";

    public bool HasError => !string.IsNullOrWhiteSpace(_run.ErrorMessage);

    public string? ErrorMessage => _run.ErrorMessage;

    public void Update(WorkerRun run)
    {
        _run = run;
        OnPropertyChanged(string.Empty);
    }
}
