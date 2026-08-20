using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Infrastructure;

public sealed class DisabledRecycleOperationCapabilityExecutor : IRecycleOperationCapabilityExecutor
{
    public bool IsEnabled => false;

    public Task<IReadOnlyList<RecycleEligibilityObservation>> InspectAsync(
        IReadOnlyList<WorkerRecycleOperationItem> items,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(items);
        cancellationToken.ThrowIfCancellationRequested();
        IReadOnlyList<RecycleEligibilityObservation> observations = items
            .Select(item => new RecycleEligibilityObservation(
                item.Id,
                "non_recyclable",
                "executor_disabled"))
            .ToArray();
        return Task.FromResult(observations);
    }
}
