using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public enum ReviewResultKind
{
    File,
    Folder,
}

public sealed record ReviewResultTarget(
    ReviewResultKind Kind,
    long GroupId,
    long? MemberId = null);

public sealed class ReviewFileGroupListItemViewModel(WorkerReviewGroupSummary group)
{
    public WorkerReviewGroupSummary Group { get; } = group;

    public long GroupId => Group.GroupId;

    public string Title => $"File set {GroupId:N0}";

    public string DecisionSummary =>
        $"{Group.RemoveCount:N0} marked · {Group.KeepCount:N0} kept · {Group.UndecidedCount:N0} undecided";

    public string SurvivorSummary => Group.RemainingPhysicalCopyCount == 1
        ? "1 independently accessible physical copy remains"
        : $"{Group.RemainingPhysicalCopyCount:N0} independently accessible physical copies remain";

    public ReviewResultTarget Target => new(ReviewResultKind.File, GroupId);

    public string AutomationName => $"{Title}; {DecisionSummary}; {SurvivorSummary}";
}

public sealed class ReviewFolderGroupListItemViewModel(WorkerReviewFolderGroupSummary group)
{
    public WorkerReviewFolderGroupSummary Group { get; } = group;

    public long GroupId => Group.FolderGroupId;

    public string Title => $"Folder set {GroupId:N0}";

    public string DecisionSummary =>
        $"{Group.RemoveCount:N0} marked · {Group.KeepCount:N0} kept · {Group.UndecidedCount:N0} undecided";

    public string SurvivorSummary => Group.IntactCopyCount == 1
        ? "1 intact folder copy remains"
        : $"{Group.IntactCopyCount:N0} intact folder copies remain";

    public ReviewResultTarget Target => new(ReviewResultKind.Folder, GroupId);

    public string AutomationName => $"{Title}; {DecisionSummary}; {SurvivorSummary}";
}
