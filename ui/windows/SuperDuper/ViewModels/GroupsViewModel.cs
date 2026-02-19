using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml.Media;
using SuperDuper.Converters;
using SuperDuper.Models;
using SuperDuper.NativeMethods;
using SuperDuper.Services;
using SuperDuper.Services.Platform;
using System.Collections.ObjectModel;

namespace SuperDuper.ViewModels;

public partial class GroupsViewModel : ObservableObject
{
    private readonly IDatabaseService _db;
    private readonly IUndoService _undo;
    private readonly IShellIntegrationService _shell;
    private readonly EngineWrapper _engine;
    private int _offset;
    private const int PageSize = 50;
    private string _sortColumn = "wasted_bytes";

    public ObservableCollection<GroupViewModel> Groups { get; } = new();
    public ObservableCollection<FilterChip> ActiveFilters { get; } = new();

    [ObservableProperty]
    private bool _hasMore;

    [ObservableProperty]
    private string? _autoSelectMessage;

    public GroupsViewModel(IDatabaseService db, IUndoService undo, IShellIntegrationService shell, EngineWrapper engine)
    {
        _db = db;
        _undo = undo;
        _shell = shell;
        _engine = engine;
    }

    public async Task LoadInitialAsync(long sessionId)
    {
        _offset = 0;
        Groups.Clear();
        await LoadPageAsync(sessionId);
    }

    [RelayCommand]
    private async Task LoadMoreAsync()
    {
        // Re-uses the last sessionId — simplified for now
        await LoadPageAsync(GetCurrentSessionId());
    }

    private long GetCurrentSessionId()
    {
        // Get session from EngineWrapper active session
        try { return 0; /* placeholder — caller should pass session */ }
        catch { return 0; }
    }

    private async Task LoadPageAsync(long sessionId)
    {
        var filter = BuildFilter();
        var result = await _db.QueryGroupsFilteredAsync(sessionId, filter, _sortColumn, false, _offset, PageSize);

        foreach (var group in result.Items)
        {
            var vm = new GroupViewModel(group, _db, _undo, _shell, _engine);
            Groups.Add(vm);
        }

        _offset += result.Items.Count;
        HasMore = _offset < result.TotalCount;
    }

    private GroupFilterOptions BuildFilter()
    {
        var text = ActiveFilters.FirstOrDefault(f => f.FilterType == FilterType.TextSearch)?.Value as string;
        return new GroupFilterOptions(text, null, null, null, null, null);
    }

    public void SetSort(string column)
    {
        _sortColumn = column;
        Groups.Clear();
        _offset = 0;
    }

    public async Task ApplyAutoSelectAsync(string strategy, long sessionId)
    {
        // Collect all files in loaded groups and apply strategy
        var changes = new List<(long FileId, long GroupId, ReviewAction NewAction, ReviewAction? OldAction)>();

        foreach (var group in Groups)
        {
            if (group.Files.Count < 2) continue;

            GroupFileViewModel? keepTarget = strategy switch
            {
                "newest" => group.Files.OrderByDescending(f => f.LastModified).FirstOrDefault(),
                "shortest" => group.Files.OrderBy(f => f.CanonicalPath.Length).FirstOrDefault(),
                _ => null
            };

            if (keepTarget == null) continue;

            foreach (var file in group.Files)
            {
                var newAction = file.FileId == keepTarget.FileId ? ReviewAction.Keep : ReviewAction.Delete;
                var old = await _db.GetDecisionAsync(file.FileId);
                changes.Add((file.FileId, group.GroupId, newAction, old));
                await _db.UpsertDecisionAsync(file.FileId, group.GroupId, newAction, sessionId);
            }
        }

        if (changes.Count > 0)
        {
            _undo.Push(new BulkDecisionAction(_db, changes, sessionId, strategy));
            AutoSelectMessage = $"Applied {strategy} strategy to {changes.Count / 2} groups. Undo with Ctrl+Z.";
        }
    }
}

public partial class GroupViewModel : ObservableObject
{
    private readonly IDatabaseService _db;
    private readonly IUndoService _undo;
    private readonly IShellIntegrationService _shell;
    private readonly EngineWrapper _engine;

    public long GroupId { get; }
    public string SampleFileName { get; }
    public int FileCount { get; }
    public string FormattedFileSize { get; }
    public string FormattedWastedBytes { get; }

    public ObservableCollection<GroupFileViewModel> Files { get; } = new();

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ReviewStatusLabel), nameof(ReviewStatusBackground))]
    private ReviewStatus _reviewStatus = ReviewStatus.Unreviewed;

    [ObservableProperty]
    private bool _isExpanded;

    public string ReviewStatusLabel => ReviewStatus switch
    {
        ReviewStatus.Unreviewed => "Unreviewed",
        ReviewStatus.Partial => "Partial",
        ReviewStatus.Decided => "Decided",
        _ => ""
    };

    public string ReviewStatusBackground => ReviewStatus switch
    {
        ReviewStatus.Unreviewed => "SkipBgBrush",
        ReviewStatus.Partial => "WarningBrush",
        ReviewStatus.Decided => "KeepBgBrush",
        _ => "SkipBgBrush"
    };

    public GroupViewModel(DbGroupInfo group, IDatabaseService db, IUndoService undo, IShellIntegrationService shell, EngineWrapper engine)
    {
        _db = db;
        _undo = undo;
        _shell = shell;
        _engine = engine;
        GroupId = group.GroupId;
        SampleFileName = group.SampleFileName;
        FileCount = group.FileCount;
        FormattedFileSize = FileSizeConverter.FormatBytes(group.FileSize);
        FormattedWastedBytes = FileSizeConverter.FormatBytes(group.WastedBytes);

        PropertyChanged += async (_, args) =>
        {
            if (args.PropertyName == nameof(IsExpanded) && IsExpanded && Files.Count == 0)
                await LoadFilesAsync();
        };
    }

    private async Task LoadFilesAsync()
    {
        var fileInfos = await Task.Run(() => _engine.QueryFilesInGroup(GroupId));
        foreach (var f in fileInfos)
        {
            var decision = await _db.GetDecisionAsync(f.Id);
            Files.Add(new GroupFileViewModel(f, GroupId, decision, _db, _undo, _shell));
        }
        ReviewStatus = await _db.GetGroupReviewStatusAsync(GroupId);
    }
}

public partial class GroupFileViewModel : ObservableObject
{
    private readonly IDatabaseService _db;
    private readonly IUndoService _undo;
    private readonly IShellIntegrationService _shell;

    public long FileId { get; }
    public long GroupId { get; }
    public string CanonicalPath { get; }
    public string LastModified { get; }
    public string KeepAccessibleName => $"Keep copy at {Path.GetFileName(CanonicalPath)}";
    public string DeleteAccessibleName => $"Delete copy at {Path.GetFileName(CanonicalPath)}";
    public string SkipAccessibleName => $"Skip {Path.GetFileName(CanonicalPath)}";

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(DecisionGlyph), nameof(DecisionBrush))]
    private ReviewAction? _currentDecision;

    public string DecisionGlyph => CurrentDecision switch
    {
        ReviewAction.Keep => "\uE73E",
        ReviewAction.Delete => "\uE711",
        ReviewAction.Skip => "\uE73A",
        _ => ""
    };

    public string DecisionBrush => CurrentDecision switch
    {
        ReviewAction.Keep => "KeepBrush",
        ReviewAction.Delete => "DeleteBrush",
        ReviewAction.Skip => "SkipBrush",
        _ => "SkipBrush"
    };

    // Drive color stripe — simplified: use a neutral brush
    public SolidColorBrush DriveColorBrush { get; } = new(Microsoft.UI.Colors.Gray);

    public GroupFileViewModel(NativeMethods.FileInfo file, long groupId, ReviewAction? decision,
        IDatabaseService db, IUndoService undo, IShellIntegrationService shell)
    {
        _db = db;
        _undo = undo;
        _shell = shell;
        FileId = file.Id;
        GroupId = groupId;
        CanonicalPath = file.CanonicalPath;
        LastModified = "";  // FileInfo doesn't have LastModified — use DisplayPath
        CurrentDecision = decision;
    }

    [RelayCommand]
    private async Task SetKeepAsync() => await SetDecisionAsync(ReviewAction.Keep);

    [RelayCommand]
    private async Task SetDeleteAsync() => await SetDecisionAsync(ReviewAction.Delete);

    [RelayCommand]
    private async Task SetSkipAsync() => await SetDecisionAsync(ReviewAction.Skip);

    [RelayCommand]
    private void Reveal() => _shell.RevealInExplorer(CanonicalPath);

    private async Task SetDecisionAsync(ReviewAction action)
    {
        var old = CurrentDecision;
        CurrentDecision = action;
        var undoAction = new SetDecisionAction(
            _db, FileId, GroupId, action, old, null,
            Path.GetFileName(CanonicalPath));
        _undo.Push(undoAction);
        await _db.UpsertDecisionAsync(FileId, GroupId, action);
    }
}
