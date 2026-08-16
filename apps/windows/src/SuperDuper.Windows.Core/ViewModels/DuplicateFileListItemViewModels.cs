using System.Globalization;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class DuplicateFileGroupListItemViewModel(WorkerDuplicateFileGroup group)
{
    public WorkerDuplicateFileGroup Group { get; } = group;

    public long Id => Group.Id;

    public string RepresentativeName => Group.RepresentativeName;

    public string RepresentativeType => Group.RepresentativeType;

    public string GroupSize => DisplayFormatting.Bytes(Group.GroupSize);

    public string CopyCount => Group.CopyCount.ToString("N0");

    public string RecoverableBytes => DisplayFormatting.Bytes(Group.RecoverableBytes);
}

public sealed class DuplicateFileMemberListItemViewModel(WorkerDuplicateFileMember member)
{
    public WorkerDuplicateFileMember Member { get; } = member;

    public long Id => Member.Id;

    public string Path => Member.Path;

    public string Size => DisplayFormatting.Bytes(Member.Size);

    public string Modified
    {
        get
        {
            if (!long.TryParse(
                    Member.ModifiedTimeUnixNanos,
                    NumberStyles.Integer,
                    CultureInfo.InvariantCulture,
                    out var nanoseconds))
            {
                return Member.ModifiedTimeUnixNanos;
            }
            try
            {
                return DateTimeOffset
                    .FromUnixTimeSeconds(nanoseconds / 1_000_000_000)
                    .ToLocalTime()
                    .ToString("g");
            }
            catch (ArgumentOutOfRangeException)
            {
                return "Unknown";
            }
        }
    }
}
