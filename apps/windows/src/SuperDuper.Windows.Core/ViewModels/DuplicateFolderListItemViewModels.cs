using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class DuplicateFolderGroupListItemViewModel(WorkerDuplicateFolderGroup group)
{
    public WorkerDuplicateFolderGroup Group { get; } = group;

    public long Id => Group.Id;

    public string RepresentativePath => Group.RepresentativePath;

    public string TotalBytes => DisplayFormatting.Bytes(Group.TotalBytes);

    public string DescendantFileCount => Group.DescendantFileCount.ToString("N0");

    public string CopyCount => Group.CopyCount.ToString("N0");
}

public sealed class DuplicateFolderMemberListItemViewModel(WorkerDuplicateFolderMember member)
{
    public WorkerDuplicateFolderMember Member { get; } = member;

    public long Id => Member.Id;

    public string Path => Member.Path;
}
