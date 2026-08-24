namespace SuperDuper.Windows.Core.Services;

public interface IExplorerService
{
    const int MaximumSelectionItems = 200;

    Task RevealAsync(string path, CancellationToken cancellationToken = default);

    Task<ExplorerSelectionResult> SelectByParentAsync(
        IReadOnlyList<string> paths,
        CancellationToken cancellationToken = default);
}

public sealed record ExplorerParentSelectionFailure(
    string ParentPath,
    int ItemCount,
    string ErrorMessage);

public sealed record ExplorerSelectionResult(
    int RequestedItemCount,
    int ParentCount,
    int SelectedItemCount,
    IReadOnlyList<ExplorerParentSelectionFailure> Failures)
{
    public int FailedItemCount => RequestedItemCount - SelectedItemCount;

    public int SuccessfulParentCount => ParentCount - Failures.Count;
}
