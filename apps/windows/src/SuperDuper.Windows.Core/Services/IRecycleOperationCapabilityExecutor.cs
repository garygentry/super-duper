using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Services;

public interface IRecycleOperationCapabilityExecutor
{
    bool IsEnabled { get; }

    Task<IReadOnlyList<RecycleEligibilityObservation>> InspectAsync(
        IReadOnlyList<WorkerRecycleOperationItem> items,
        CancellationToken cancellationToken = default);

    Task<RecycleBatchExecutionResult> ExecuteBatchAsync(
        WorkerRecycleOperationBatch batch,
        Func<CancellationToken, Task> acknowledgeShellStart,
        CancellationToken cancellationToken = default);
}
