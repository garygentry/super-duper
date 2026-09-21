using System.Globalization;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class DuplicateFoldersViewModel : ObservableObject, IDisposable
{
    public const int PageSize = 200;
    public const int CacheCapacity = 5;

    private readonly IWorkerClient _workerClient;
    private readonly IClipboardService _clipboard;
    private readonly IExplorerService _explorer;
    private readonly BoundedCursorCache<WorkerDuplicateFolderGroupPage> _groupCache = new(CacheCapacity);
    private readonly BoundedCursorCache<WorkerDuplicateFolderMemberPage> _memberCache = new(CacheCapacity);
    private CancellationTokenSource? _groupCancellation;
    private CancellationTokenSource? _memberCancellation;
    private CancellationTokenSource? _reviewCancellation;
    private CancellationTokenSource? _explorerCancellation;
    private WorkerRun? _run;
    private IReadOnlyList<DuplicateFolderGroupListItemViewModel> _groups = [];
    private IReadOnlyList<DuplicateFolderMemberListItemViewModel> _members = [];
    private DuplicateFolderGroupListItemViewModel? _selectedGroup;
    private DuplicateFolderMemberListItemViewModel? _selectedMember;
    private WorkerDuplicateFolderGroupPage? _currentGroupPage;
    private WorkerDuplicateFolderMemberPage? _currentMemberPage;
    private WorkerReviewPlanView _reviewPlan = new(
        new WorkerReviewPlan(null, 0, "notCreated", 0, null, null),
        new WorkerReviewPlanSummary(0, 0, 0, 0, "0", 0));
    private WorkerReviewFolderGroupSummary _selectedReviewSummary = new(0, 0, 0, 0, 0);
    private long _groupGeneration;
    private long _memberGeneration;
    private long _reviewGeneration;
    private long _explorerGeneration;
    private long _totalGroups;
    private long _totalMembers;
    private string _searchText = string.Empty;
    private string _minimumSizeText = string.Empty;
    private string _minimumSizeUnit = "B";
    private string _stateMessage = "Select a completed run to browse duplicate folders.";
    private string? _errorMessage;
    private string? _detailErrorMessage;
    private string? _explorerStatusMessage;
    private string? _explorerErrorMessage;
    private string _groupStatusAnnouncement = "Duplicate folder results have not loaded.";
    private string _groupErrorAnnouncement = string.Empty;
    private string _memberStatusAnnouncement = "No exact duplicate folder group details have loaded.";
    private long _groupStatusAnnouncementVersion;
    private long _groupErrorAnnouncementVersion;
    private long _memberStatusAnnouncementVersion;
    private long _memberErrorAnnouncementVersion;
    private long _explorerStatusAnnouncementVersion;
    private long _explorerErrorAnnouncementVersion;
    private bool _isLoading;
    private bool _isDetailLoading;
    private bool _isReviewUpdating;
    private bool _isReviewLoaded;
    private bool _isExplorerCommandRunning;
    private DuplicateFolderGroupSortField _sortField = DuplicateFolderGroupSortField.TotalBytes;
    private WorkerSortDirection _sortDirection = WorkerSortDirection.Descending;
    private DuplicateFolderGroupSortField _appliedSortField = DuplicateFolderGroupSortField.TotalBytes;
    private WorkerSortDirection _appliedSortDirection = WorkerSortDirection.Descending;
    private DuplicateFolderGroupFilter _appliedFilter = new(string.Empty, "0");
    private bool _disposed;

    public event Action<long, long>? ReviewRevisionChanged;

    public DuplicateFoldersViewModel(IWorkerClient workerClient, IClipboardService clipboard, IExplorerService explorer)
    {
        _workerClient = workerClient;
        _clipboard = clipboard;
        _explorer = explorer;
        ApplyFiltersCommand = new AsyncRelayCommand(ApplyFiltersAsync);
        ClearFiltersCommand = new AsyncRelayCommand(ClearFiltersAsync);
        NextPageCommand = new AsyncRelayCommand(NextPageAsync, () => CanMoveNext);
        PreviousPageCommand = new AsyncRelayCommand(PreviousPageAsync, () => CanMovePrevious);
        NextMemberPageCommand = new AsyncRelayCommand(NextMemberPageAsync, () => CanMoveMembersNext);
        PreviousMemberPageCommand = new AsyncRelayCommand(PreviousMemberPageAsync, () => CanMoveMembersPrevious);
        CopyPathCommand = new RelayCommand<DuplicateFolderMemberListItemViewModel>(CopyPath);
        RevealInExplorerCommand = new AsyncRelayCommand<DuplicateFolderMemberListItemViewModel>(
            RevealAsync,
            CanRevealInExplorer);
        SelectPageInExplorerCommand = new AsyncRelayCommand(
            SelectPageInExplorerAsync,
            () => CanSelectPageInExplorer);
        KeepFolderCommand = new AsyncRelayCommand<DuplicateFolderMemberListItemViewModel>(
            member => SetReviewDecisionAsync(member, "keep"),
            CanSetReviewDecision);
        RemoveFolderCommand = new AsyncRelayCommand<DuplicateFolderMemberListItemViewModel>(
            member => SetReviewDecisionAsync(member, "remove"),
            CanSetReviewDecision);
        UndecideFolderCommand = new AsyncRelayCommand<DuplicateFolderMemberListItemViewModel>(
            member => SetReviewDecisionAsync(member, "undecided"),
            CanSetReviewDecision);
    }

    public WorkerRun? Run { get => _run; private set => SetProperty(ref _run, value); }
    public IReadOnlyList<DuplicateFolderGroupListItemViewModel> Groups
    {
        get => _groups;
        private set
        {
            if (SetProperty(ref _groups, value))
            {
                OnPropertyChanged(nameof(IsLoadingOverlayVisible));
                OnPropertyChanged(nameof(IsInitialError));
                OnPropertyChanged(nameof(HasRetainedGroupError));
            }
        }
    }
    public IReadOnlyList<DuplicateFolderMemberListItemViewModel> Members
    {
        get => _members;
        private set
        {
            var selectedId = SelectedMember?.Id;
            if (SetProperty(ref _members, value))
            {
                SelectedMember = selectedId is null
                    ? null
                    : value.FirstOrDefault(member => member.Id == selectedId);
                OnPropertyChanged(nameof(MemberPageStatusText));
                OnPropertyChanged(nameof(CanSelectPageInExplorer));
                RevealInExplorerCommand.NotifyCanExecuteChanged();
                SelectPageInExplorerCommand.NotifyCanExecuteChanged();
            }
        }
    }
    public DuplicateFolderGroupListItemViewModel? SelectedGroup
    {
        get => _selectedGroup;
        set => SetSelectedGroup(value, loadMembers: true);
    }
    public DuplicateFolderMemberListItemViewModel? SelectedMember
    {
        get => _selectedMember;
        set
        {
            if (SetProperty(ref _selectedMember, value))
            {
                CancelExplorerCommand(clearFeedback: true);
                OnPropertyChanged(nameof(HasSelectedMember));
                RevealInExplorerCommand.NotifyCanExecuteChanged();
            }
        }
    }
    public string SearchText { get => _searchText; set => SetProperty(ref _searchText, value); }
    public string MinimumSizeText { get => _minimumSizeText; set => SetProperty(ref _minimumSizeText, value); }
    public IReadOnlyList<string> SizeUnits => BinarySizeInput.Units;
    public string MinimumSizeUnit { get => _minimumSizeUnit; set => SetProperty(ref _minimumSizeUnit, value); }
    public string StateMessage { get => _stateMessage; private set => SetProperty(ref _stateMessage, value); }
    public string? ErrorMessage
    {
        get => _errorMessage;
        private set
        {
            if (SetProperty(ref _errorMessage, value))
            {
                OnPropertyChanged(nameof(HasError));
                OnPropertyChanged(nameof(IsInitialError));
                OnPropertyChanged(nameof(HasRetainedGroupError));
            }
        }
    }
    public string? DetailErrorMessage
    {
        get => _detailErrorMessage;
        private set
        {
            if (SetProperty(ref _detailErrorMessage, value))
            {
                OnPropertyChanged(nameof(HasDetailError));
                RaiseDetailEmptyState();
            }
        }
    }
    public string? ExplorerStatusMessage
    {
        get => _explorerStatusMessage;
        private set
        {
            if (SetProperty(ref _explorerStatusMessage, value))
            {
                OnPropertyChanged(nameof(HasExplorerStatus));
            }
        }
    }
    public string? ExplorerErrorMessage
    {
        get => _explorerErrorMessage;
        private set
        {
            if (SetProperty(ref _explorerErrorMessage, value))
            {
                OnPropertyChanged(nameof(HasExplorerError));
            }
        }
    }
    public bool IsLoading
    {
        get => _isLoading;
        private set
        {
            if (SetProperty(ref _isLoading, value))
            {
                OnPropertyChanged(nameof(IsEmpty));
                OnPropertyChanged(nameof(IsLoadingOverlayVisible));
                OnPropertyChanged(nameof(IsInitialError));
                OnPropertyChanged(nameof(HasRetainedGroupError));
                RaiseGroupPaging();
                if (!value)
                {
                    PublishGroupQueryAnnouncement();
                }
            }
        }
    }
    public bool IsDetailLoading
    {
        get => _isDetailLoading;
        private set
        {
            if (SetProperty(ref _isDetailLoading, value))
            {
                RaiseDetailEmptyState();
                RaiseMemberPaging();
            }
        }
    }
    public bool IsReviewUpdating
    {
        get => _isReviewUpdating;
        private set
        {
            if (SetProperty(ref _isReviewUpdating, value))
            {
                KeepFolderCommand.NotifyCanExecuteChanged();
                RemoveFolderCommand.NotifyCanExecuteChanged();
                UndecideFolderCommand.NotifyCanExecuteChanged();
            }
        }
    }
    public bool IsExplorerCommandRunning
    {
        get => _isExplorerCommandRunning;
        private set
        {
            if (SetProperty(ref _isExplorerCommandRunning, value))
            {
                RevealInExplorerCommand.NotifyCanExecuteChanged();
                SelectPageInExplorerCommand.NotifyCanExecuteChanged();
                OnPropertyChanged(nameof(CanSelectPageInExplorer));
            }
        }
    }
    public WorkerReviewPlanView ReviewPlan
    {
        get => _reviewPlan;
        private set
        {
            if (SetProperty(ref _reviewPlan, value))
            {
                OnPropertyChanged(nameof(ReviewPlanSummaryText));
            }
        }
    }
    public WorkerReviewFolderGroupSummary SelectedReviewSummary
    {
        get => _selectedReviewSummary;
        private set
        {
            if (SetProperty(ref _selectedReviewSummary, value))
            {
                OnPropertyChanged(nameof(SelectedReviewSummaryText));
            }
        }
    }
    public long TotalGroups
    {
        get => _totalGroups;
        private set { if (SetProperty(ref _totalGroups, value)) RaiseState(); }
    }
    public long TotalMembers
    {
        get => _totalMembers;
        private set
        {
            if (SetProperty(ref _totalMembers, value))
            {
                OnPropertyChanged(nameof(MemberCountText));
                OnPropertyChanged(nameof(MemberPageStatusText));
                RaiseDetailEmptyState();
            }
        }
    }
    public DuplicateFolderGroupSortField SortField => _sortField;
    public WorkerSortDirection SortDirection => _sortDirection;
    public string GroupCountText => $"{TotalGroups:N0} groups";
    public string MemberCountText => $"{TotalMembers:N0} folders";
    public string MemberPageStatusText =>
        $"Showing {Members.Count:N0} of {TotalMembers:N0} folder copies";
    public string ReviewPlanSummaryText =>
        $"Combined review: {ReviewPlan.Summary.RemoveCount:N0} files and "
        + $"{ReviewPlan.Summary.FolderRemoveCount:N0} folders marked for removal · "
        + $"{ReviewPlan.Summary.EffectiveRemovalFileCount:N0} distinct file paths, "
        + $"{DisplayFormatting.Bytes(ReviewPlan.Summary.PlannedRemovalBytes)} physical data";
    public string SelectedReviewSummaryText => SelectedGroup is null
        ? "No exact-folder set selected for review."
        : $"Folder review: {SelectedReviewSummary.KeepCount:N0} keep, "
            + $"{SelectedReviewSummary.RemoveCount:N0} remove, "
            + $"{SelectedReviewSummary.UndecidedCount:N0} undecided · "
            + (SelectedReviewSummary.IntactCopyCount == 1
                ? "1 intact copy remains"
                : $"{SelectedReviewSummary.IntactCopyCount:N0} intact copies remain");
    public string SelectedRelationshipSummaryText => SelectedGroup?.RelationshipSummary
        ?? "Select an exact-folder set to compare its locations.";
    public bool HasSelectedGroup => SelectedGroup is not null;
    public bool HasSelectedMember => SelectedMember is not null;
    public bool HasAppliedFilters => _appliedFilter.Search.Length > 0 || _appliedFilter.MinimumSize != "0";
    public string AppliedFilterSummaryText
    {
        get
        {
            var parts = new List<string>();
            if (_appliedFilter.Search.Length > 0)
            {
                parts.Add($"path contains “{_appliedFilter.Search}”");
            }
            if (_appliedFilter.MinimumSize != "0")
            {
                parts.Add($"at least {_appliedFilter.MinimumSize} bytes per folder copy");
            }
            return parts.Count == 0
                ? "Applied folder filters: none. Showing the default server query."
                : $"Applied folder filters: {string.Join("; ", parts)}.";
        }
    }
    public string AppliedFilterBriefText
    {
        get
        {
            if (!HasAppliedFilters) return "All exact-folder sets";
            var parts = new List<string>();
            if (_appliedFilter.Search.Length > 0)
                parts.Add($"Path contains “{_appliedFilter.Search}”");
            if (_appliedFilter.MinimumSize != "0")
                parts.Add($"At least {_appliedFilter.MinimumSize} bytes per copy");
            return string.Join(" · ", parts);
        }
    }
    public string EmptyStateTitle => HasAppliedFilters
        ? "No folders match these filters"
        : "No exact duplicate folders";
    public string UnavailableTitle => Run?.Status switch
    {
        null => "No scan selected",
        "running" or "pending" or "cancelling" => "Folder results not ready",
        "cancelled" => "Scan cancelled",
        "failed" => "Scan failed",
        _ => "Folder results unavailable",
    };
    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);
    public bool HasDetailError => !string.IsNullOrWhiteSpace(DetailErrorMessage);
    public bool HasExplorerStatus => !string.IsNullOrWhiteSpace(ExplorerStatusMessage);
    public bool HasExplorerError => !string.IsNullOrWhiteSpace(ExplorerErrorMessage);
    public bool IsUnavailable => Run is null || Run.Status != "completed";
    public bool IsEmpty => Run?.Status == "completed" && !IsLoading && !HasError && TotalGroups == 0;
    public bool IsLoadingOverlayVisible => IsLoading && Groups.Count == 0;
    public bool IsInitialError => HasError && Groups.Count == 0 && !IsLoading;
    public bool HasRetainedGroupError => HasError && Groups.Count > 0;
    public bool IsDetailEmpty => SelectedGroup is not null && !IsDetailLoading && !HasDetailError && TotalMembers == 0;
    public bool CanMoveNext => !IsLoading && _currentGroupPage?.NextCursor is not null;
    public bool CanMovePrevious => !IsLoading && _currentGroupPage?.PreviousCursor is not null;
    public bool CanMoveMembersNext => !IsDetailLoading && _currentMemberPage?.NextCursor is not null;
    public bool CanMoveMembersPrevious => !IsDetailLoading && _currentMemberPage?.PreviousCursor is not null;
    public int CachedGroupPageCount => _groupCache.Count;
    public int CachedMemberPageCount => _memberCache.Count;
    public string GroupStatusAnnouncement
    {
        get => _groupStatusAnnouncement;
        private set => SetProperty(ref _groupStatusAnnouncement, value);
    }
    public string GroupErrorAnnouncement
    {
        get => _groupErrorAnnouncement;
        private set => SetProperty(ref _groupErrorAnnouncement, value);
    }
    public long GroupStatusAnnouncementVersion
    {
        get => _groupStatusAnnouncementVersion;
        private set => SetProperty(ref _groupStatusAnnouncementVersion, value);
    }
    public long GroupErrorAnnouncementVersion
    {
        get => _groupErrorAnnouncementVersion;
        private set => SetProperty(ref _groupErrorAnnouncementVersion, value);
    }
    public string MemberStatusAnnouncement
    {
        get => _memberStatusAnnouncement;
        private set => SetProperty(ref _memberStatusAnnouncement, value);
    }
    public long MemberStatusAnnouncementVersion
    {
        get => _memberStatusAnnouncementVersion;
        private set => SetProperty(ref _memberStatusAnnouncementVersion, value);
    }
    public long MemberErrorAnnouncementVersion
    {
        get => _memberErrorAnnouncementVersion;
        private set => SetProperty(ref _memberErrorAnnouncementVersion, value);
    }
    public long ExplorerStatusAnnouncementVersion
    {
        get => _explorerStatusAnnouncementVersion;
        private set => SetProperty(ref _explorerStatusAnnouncementVersion, value);
    }
    public long ExplorerErrorAnnouncementVersion
    {
        get => _explorerErrorAnnouncementVersion;
        private set => SetProperty(ref _explorerErrorAnnouncementVersion, value);
    }

    public IAsyncRelayCommand ApplyFiltersCommand { get; }
    public IAsyncRelayCommand ClearFiltersCommand { get; }
    public IAsyncRelayCommand NextPageCommand { get; }
    public IAsyncRelayCommand PreviousPageCommand { get; }
    public IAsyncRelayCommand NextMemberPageCommand { get; }
    public IAsyncRelayCommand PreviousMemberPageCommand { get; }
    public IRelayCommand<DuplicateFolderMemberListItemViewModel> CopyPathCommand { get; }
    public IAsyncRelayCommand<DuplicateFolderMemberListItemViewModel> RevealInExplorerCommand { get; }
    public IAsyncRelayCommand SelectPageInExplorerCommand { get; }
    public IAsyncRelayCommand<DuplicateFolderMemberListItemViewModel> KeepFolderCommand { get; }
    public IAsyncRelayCommand<DuplicateFolderMemberListItemViewModel> RemoveFolderCommand { get; }
    public IAsyncRelayCommand<DuplicateFolderMemberListItemViewModel> UndecideFolderCommand { get; }

    public async Task ShowRunAsync(WorkerRun? run, CancellationToken cancellationToken = default)
    {
        Run = run;
        ResetQueries();
        OnPropertyChanged(nameof(IsUnavailable));
        OnPropertyChanged(nameof(UnavailableTitle));
        RaiseState();
        if (run is null)
        {
            StateMessage = "Select a completed run to browse duplicate folders.";
            return;
        }
        if (run.Status != "completed")
        {
            StateMessage = run.Status is "running" or "pending" or "cancelling"
                ? "Duplicate folder results become available after this scan completes."
                : $"This run is {DisplayFormatting.Status(run.Status).ToLowerInvariant()}; partial results are not shown.";
            return;
        }
        if (!TryBuildFilter(out var initialFilter))
        {
            PublishGroupErrorAnnouncement("Duplicate folder filters could not be applied.");
            return;
        }
        StateMessage = "Loading exact duplicate folder results.";
        _reviewCancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var reviewGeneration = ++_reviewGeneration;
        await Task.WhenAll(
            ResetAndLoadGroupsAsync(cancellationToken, requestedFilter: initialFilter),
            LoadReviewPlanAsync(run.Id, reviewGeneration, _reviewCancellation.Token));
    }

    public async Task<bool> OpenReviewTargetAsync(
        long groupId,
        long? memberId = null,
        CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var group = Groups.FirstOrDefault(item => item.Id == groupId);
        if (group is null || Run?.Status != "completed")
        {
            return false;
        }
        SetSelectedGroup(group, loadMembers: false);
        await LoadSelectedGroupAsync(group, memberId);
        cancellationToken.ThrowIfCancellationRequested();
        return SelectedGroup?.Id == groupId && !HasDetailError;
    }

    public async Task RefreshReviewRevisionAsync(long runId, long revision)
    {
        if (Run?.Id != runId || revision <= ReviewPlan.Plan.Revision)
        {
            return;
        }

        CancelReviewQuery();
        _reviewCancellation = new CancellationTokenSource();
        var generation = _reviewGeneration;
        var cancellationToken = _reviewCancellation.Token;
        _memberCache.Clear();
        var planTask = LoadReviewPlanAsync(runId, generation, cancellationToken);
        if (SelectedGroup is { } selectedGroup)
        {
            await Task.WhenAll(planTask, LoadSelectedGroupAsync(selectedGroup, SelectedMember?.Id));
        }
        else
        {
            await planTask;
        }
    }

    public async Task ApplySortAsync(DuplicateFolderGroupSortField field, WorkerSortDirection direction, CancellationToken cancellationToken = default)
    {
        if (_sortField == field && _sortDirection == direction) return;
        _sortField = field;
        _sortDirection = direction;
        OnPropertyChanged(nameof(SortField));
        OnPropertyChanged(nameof(SortDirection));
        if (Run?.Status == "completed")
        {
            await ResetAndLoadGroupsAsync(cancellationToken, preserveDisplayedResults: true);
        }
    }

    public void ApplyLifecycle(WorkerRun run)
    {
        if (Run?.Id == run.Id) _ = ShowRunAsync(run);
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        CancelGroupQuery();
        CancelMemberQuery();
        CancelReviewQuery();
    }

    private async Task ApplyFiltersAsync()
    {
        if (Run?.Status != "completed")
        {
            return;
        }
        if (!TryBuildFilter(out var filter))
        {
            PublishGroupErrorAnnouncement("Duplicate folder filters could not be applied.");
            return;
        }
        await ResetAndLoadGroupsAsync(preserveDisplayedResults: true, requestedFilter: filter);
    }

    private async Task ClearFiltersAsync()
    {
        SearchText = string.Empty;
        MinimumSizeText = string.Empty;
        MinimumSizeUnit = "B";
        if (Run?.Status == "completed")
        {
            _sortField = DuplicateFolderGroupSortField.TotalBytes;
            _sortDirection = WorkerSortDirection.Descending;
            OnPropertyChanged(nameof(SortField));
            OnPropertyChanged(nameof(SortDirection));
            await ResetAndLoadGroupsAsync(
                preserveDisplayedResults: true,
                requestedFilter: new DuplicateFolderGroupFilter(string.Empty, "0"));
        }
    }

    private async Task ResetAndLoadGroupsAsync(
        CancellationToken cancellationToken = default,
        bool preserveDisplayedResults = false,
        DuplicateFolderGroupFilter? requestedFilter = null)
    {
        var filter = requestedFilter ?? _appliedFilter;
        CancelExplorerCommand(clearFeedback: true);
        CancelGroupQuery();
        CancelMemberQuery();
        _groupCache.Clear();
        _memberCache.Clear();
        _currentGroupPage = null;
        _currentMemberPage = null;
        IsLoading = true;
        if (!preserveDisplayedResults)
        {
            Groups = [];
            Members = [];
            SelectedGroup = null;
            TotalGroups = 0;
            TotalMembers = 0;
        }
        ErrorMessage = null;
        DetailErrorMessage = null;
        _groupCancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var generation = ++_groupGeneration;
        await LoadGroupPageAsync(null, filter, generation, _groupCancellation.Token, true);
    }

    private async Task LoadGroupPageAsync(string? cursor, DuplicateFolderGroupFilter filter, long generation, CancellationToken token, bool display)
    {
        if (_groupCache.TryGet(cursor, out var cached))
        {
            if (display && generation == _groupGeneration)
            {
                AcceptFilter(filter);
                DisplayGroupPage(cached);
                PublishGroupQueryAnnouncement();
                _ = PrefetchGroupsAsync(cached, filter, generation, token);
            }
            return;
        }
        if (Run is not { Status: "completed" } run) return;
        if (display) { IsLoading = true; ErrorMessage = null; }
        try
        {
            var page = await _workerClient.GetDuplicateFolderGroupsAsync(
                new DuplicateFolderGroupQuery(run.Id, PageSize, _sortField, _sortDirection, filter, cursor), token);
            if (generation != _groupGeneration || token.IsCancellationRequested) return;
            _groupCache.Set(cursor, page);
            if (display)
            {
                AcceptFilter(filter);
                DisplayGroupPage(page);
                _ = PrefetchGroupsAsync(page, filter, generation, token);
            }
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested) { }
        catch (Exception exception)
        {
            if (display && generation == _groupGeneration)
            {
                _sortField = _appliedSortField;
                _sortDirection = _appliedSortDirection;
                OnPropertyChanged(nameof(SortField));
                OnPropertyChanged(nameof(SortDirection));
                ErrorMessage = exception.Message;
            }
        }
        finally { if (display && generation == _groupGeneration) { IsLoading = false; RaiseState(); } }
    }

    private void DisplayGroupPage(WorkerDuplicateFolderGroupPage page)
    {
        var selectedId = SelectedGroup?.Id;
        _currentGroupPage = page;
        TotalGroups = page.Total;
        Groups = page.Groups.Take(PageSize).Select(group => new DuplicateFolderGroupListItemViewModel(group)).ToArray();
        SelectedGroup = Groups.FirstOrDefault(group => group.Id == selectedId) ?? Groups.FirstOrDefault();
        StateMessage = HasAppliedFilters
            ? "No exact duplicate folders match the applied folder filters. Clear filters to return to the default query."
            : "This completed scan contains no exact duplicate folder sets.";
        RaiseGroupPaging();
    }

    private async Task PrefetchGroupsAsync(WorkerDuplicateFolderGroupPage page, DuplicateFolderGroupFilter filter, long generation, CancellationToken token)
    {
        await PrefetchGroupDirectionAsync(page.PreviousCursor, false, 2, filter, generation, token);
        await PrefetchGroupDirectionAsync(page.NextCursor, true, 2, filter, generation, token);
    }

    private async Task PrefetchGroupDirectionAsync(string? cursor, bool forward, int remaining, DuplicateFolderGroupFilter filter, long generation, CancellationToken token)
    {
        if (cursor is null || remaining == 0 || generation != _groupGeneration || token.IsCancellationRequested) return;
        if (!_groupCache.TryGet(cursor, out var page))
        {
            await LoadGroupPageAsync(cursor, filter, generation, token, false);
            if (!_groupCache.TryGet(cursor, out page)) return;
        }
        await PrefetchGroupDirectionAsync(forward ? page.NextCursor : page.PreviousCursor, forward, remaining - 1, filter, generation, token);
    }

    private Task NextPageAsync() =>
        _currentGroupPage?.NextCursor is { } cursor && _groupCancellation is not null
            ? LoadGroupPageAsync(cursor, _appliedFilter, _groupGeneration, _groupCancellation.Token, true)
            : Task.CompletedTask;

    private Task PreviousPageAsync() =>
        _currentGroupPage?.PreviousCursor is { } cursor && _groupCancellation is not null
            ? LoadGroupPageAsync(cursor, _appliedFilter, _groupGeneration, _groupCancellation.Token, true)
            : Task.CompletedTask;

    private async Task LoadSelectedGroupAsync(
        DuplicateFolderGroupListItemViewModel? group,
        long? preferredMemberId = null)
    {
        CancelMemberQuery();
        _memberCache.Clear();
        _currentMemberPage = null;
        Members = [];
        SelectedMember = null;
        TotalMembers = 0;
        SelectedReviewSummary = new WorkerReviewFolderGroupSummary(group?.Id ?? 0, 0, 0, 0, 0);
        DetailErrorMessage = null;
        if (Run is not { Status: "completed" } run || group is null) return;
        _memberCancellation = new CancellationTokenSource();
        var generation = ++_memberGeneration;
        await LoadMemberPageAsync(null, run.Id, group.Id, generation, _memberCancellation.Token, true);
        if (preferredMemberId is { } memberId && generation == _memberGeneration)
        {
            SelectedMember = Members.FirstOrDefault(member => member.Id == memberId) ?? SelectedMember;
        }
    }

    private void SetSelectedGroup(DuplicateFolderGroupListItemViewModel? value, bool loadMembers)
    {
        if (!SetProperty(ref _selectedGroup, value, nameof(SelectedGroup)))
        {
            return;
        }
        OnPropertyChanged(nameof(SelectedReviewSummaryText));
        OnPropertyChanged(nameof(SelectedRelationshipSummaryText));
        OnPropertyChanged(nameof(HasSelectedGroup));
        RaiseDetailEmptyState();
        KeepFolderCommand.NotifyCanExecuteChanged();
        RemoveFolderCommand.NotifyCanExecuteChanged();
        UndecideFolderCommand.NotifyCanExecuteChanged();
        if (loadMembers)
        {
            _ = LoadSelectedGroupAsync(value);
        }
    }

    private async Task LoadMemberPageAsync(string? cursor, long runId, long groupId, long generation, CancellationToken token, bool display)
    {
        if (_memberCache.TryGet(cursor, out var cached))
        {
            if (display && generation == _memberGeneration)
            {
                DetailErrorMessage = null;
                DisplayMemberPage(cached);
                PublishMemberQueryAnnouncement();
                _ = PrefetchMembersAsync(cached, runId, groupId, generation, token);
            }
            return;
        }
        if (display) { IsDetailLoading = true; DetailErrorMessage = null; }
        try
        {
            var page = await _workerClient.GetDuplicateFolderGroupMembersAsync(
                new DuplicateFolderMemberQuery(runId, groupId, PageSize, DuplicateFolderMemberSortField.Path,
                    WorkerSortDirection.Ascending, new DuplicateFolderMemberFilter(string.Empty), cursor), token);
            if (generation != _memberGeneration
                || token.IsCancellationRequested
                || page.ReviewRevision < ReviewPlan.Plan.Revision)
            {
                return;
            }
            _memberCache.Set(cursor, page);
            if (display)
            {
                DisplayMemberPage(page);
                _ = PrefetchMembersAsync(page, runId, groupId, generation, token);
            }
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested) { }
        catch (Exception exception) { if (display && generation == _memberGeneration) DetailErrorMessage = exception.Message; }
        finally
        {
            if (display && generation == _memberGeneration)
            {
                IsDetailLoading = false;
                PublishMemberQueryAnnouncement();
            }
        }
    }

    private void DisplayMemberPage(WorkerDuplicateFolderMemberPage page)
    {
        _currentMemberPage = page;
        TotalMembers = page.Total;
        Members = DuplicateFolderMemberListItemViewModel.CreatePage(page.Members, PageSize);
        OnPropertyChanged(nameof(MemberPageStatusText));
        SelectedReviewSummary = page.ReviewSummary;
        if (page.ReviewRevision >= ReviewPlan.Plan.Revision)
        {
            ReviewPlan = ReviewPlan with
            {
                Plan = ReviewPlan.Plan with
                {
                    Id = page.ReviewPlanId,
                    Revision = page.ReviewRevision,
                    State = page.ReviewPlanId is null ? "notCreated" : "active",
                },
            };
        }
        RaiseMemberPaging();
    }

    private async Task PrefetchMembersAsync(WorkerDuplicateFolderMemberPage page, long runId, long groupId, long generation, CancellationToken token)
    {
        await PrefetchMemberDirectionAsync(page.PreviousCursor, false, 2, runId, groupId, generation, token);
        await PrefetchMemberDirectionAsync(page.NextCursor, true, 2, runId, groupId, generation, token);
    }

    private async Task PrefetchMemberDirectionAsync(string? cursor, bool forward, int remaining, long runId, long groupId, long generation, CancellationToken token)
    {
        if (cursor is null || remaining == 0 || generation != _memberGeneration || token.IsCancellationRequested) return;
        if (!_memberCache.TryGet(cursor, out var page))
        {
            await LoadMemberPageAsync(cursor, runId, groupId, generation, token, false);
            if (!_memberCache.TryGet(cursor, out page)) return;
        }
        await PrefetchMemberDirectionAsync(forward ? page.NextCursor : page.PreviousCursor, forward, remaining - 1, runId, groupId, generation, token);
    }

    private Task NextMemberPageAsync()
    {
        if (Run is not { } run
            || SelectedGroup is not { } group
            || _currentMemberPage?.NextCursor is not { } cursor
            || _memberCancellation is null)
        {
            return Task.CompletedTask;
        }

        CancelExplorerCommand(clearFeedback: true);
        return LoadMemberPageAsync(cursor, run.Id, group.Id, _memberGeneration, _memberCancellation.Token, true);
    }

    private Task PreviousMemberPageAsync()
    {
        if (Run is not { } run
            || SelectedGroup is not { } group
            || _currentMemberPage?.PreviousCursor is not { } cursor
            || _memberCancellation is null)
        {
            return Task.CompletedTask;
        }

        CancelExplorerCommand(clearFeedback: true);
        return LoadMemberPageAsync(cursor, run.Id, group.Id, _memberGeneration, _memberCancellation.Token, true);
    }

    private bool CanSetReviewDecision(DuplicateFolderMemberListItemViewModel? member) =>
        member is not null
        && _isReviewLoaded
        && !IsReviewUpdating
        && Run?.Status == "completed"
        && SelectedGroup is not null;

    private async Task LoadReviewPlanAsync(
        long runId,
        long generation,
        CancellationToken cancellationToken)
    {
        try
        {
            var reviewPlan = await _workerClient.GetReviewPlanAsync(runId, cancellationToken);
            if (generation != _reviewGeneration
                || cancellationToken.IsCancellationRequested
                || Run?.Id != runId)
            {
                return;
            }
            ReviewPlan = reviewPlan;
            _isReviewLoaded = true;
            KeepFolderCommand.NotifyCanExecuteChanged();
            RemoveFolderCommand.NotifyCanExecuteChanged();
            UndecideFolderCommand.NotifyCanExecuteChanged();
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (generation == _reviewGeneration && Run?.Id == runId)
            {
                DetailErrorMessage = $"Combined review status could not be loaded. {exception.Message}";
                MemberErrorAnnouncementVersion++;
            }
        }
    }

    private async Task SetReviewDecisionAsync(
        DuplicateFolderMemberListItemViewModel? member,
        string decision)
    {
        if (!CanSetReviewDecision(member)
            || member is null
            || Run is not { Status: "completed" } run
            || SelectedGroup is not { } group
            || _reviewCancellation is null)
        {
            return;
        }

        var generation = _reviewGeneration;
        var cancellationToken = _reviewCancellation.Token;
        IsReviewUpdating = true;
        DetailErrorMessage = null;
        try
        {
            var mutation = await _workerClient.SetReviewFolderDecisionAsync(
                Guid.NewGuid().ToString("N"),
                run.Id,
                group.Id,
                member.Id,
                decision,
                ReviewPlan.Plan.Revision,
                cancellationToken);
            if (generation != _reviewGeneration
                || cancellationToken.IsCancellationRequested
                || Run?.Id != run.Id
                || SelectedGroup?.Id != group.Id)
            {
                return;
            }
            ReviewPlan = ReviewPlan with
            {
                Plan = ReviewPlan.Plan with
                {
                    Id = mutation.PlanId,
                    Revision = mutation.AppliedRevision,
                    State = "active",
                },
            };
            ReviewRevisionChanged?.Invoke(run.Id, mutation.AppliedRevision);
            _memberCache.Clear();
            await LoadReviewPlanAsync(run.Id, generation, cancellationToken);
            var reviewRefreshError = DetailErrorMessage;
            if (generation != _reviewGeneration
                || cancellationToken.IsCancellationRequested
                || SelectedGroup?.Id != group.Id)
            {
                return;
            }
            await LoadSelectedGroupAsync(SelectedGroup, member.Id);
            if (reviewRefreshError is not null)
            {
                DetailErrorMessage = reviewRefreshError;
            }
            if (generation == _reviewGeneration
                && SelectedGroup?.Id == group.Id
                && !HasDetailError)
            {
                var decisionText = decision switch
                {
                    "keep" => "Keep",
                    "remove" => "Mark for removal",
                    _ => "Reset decision to Undecided",
                };
                MemberStatusAnnouncement =
                    $"Folder review decision saved: {decisionText} for {member.DisplayPath}. {SelectedReviewSummaryText}.";
                MemberStatusAnnouncementVersion++;
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (generation == _reviewGeneration && Run?.Id == run.Id)
            {
                DetailErrorMessage = ReviewDecisionError(exception);
                MemberErrorAnnouncementVersion++;
            }
        }
        finally
        {
            if (generation == _reviewGeneration)
            {
                IsReviewUpdating = false;
            }
        }
    }

    private static string ReviewDecisionError(Exception exception) =>
        (exception as IWorkerRequestFailure)?.Code switch
        {
            "review_overlap_conflict" =>
                "This folder choice overlaps an existing Keep or Remove choice. Clear the contained file or folder decision first, then retry.",
            "unsafe_folder_review_decision" =>
                "This choice would leave an exact-folder set without an intact copy. Keep or undecide another folder copy first.",
            "unsafe_review_decision" =>
                "This choice would leave a duplicate-file set without an accessible physical copy. Keep or undecide another copy first.",
            "review_generation_conflict" =>
                "Review choices changed before this update was saved. Reload the selected run and try again.",
            _ => $"The folder review decision was not saved. {exception.Message}",
        };

    private bool TryBuildFilter(out DuplicateFolderGroupFilter filter)
    {
        var search = SearchText.Trim();
        if (search.Length > 512)
        {
            ErrorMessage = "Path search may contain at most 512 characters.";
            filter = new DuplicateFolderGroupFilter(string.Empty, "0");
            return false;
        }
        var minimum = MinimumSizeText.Trim();
        if (minimum.Length == 0) minimum = "0";
        if (!BinarySizeInput.TryConvertToBytes(minimum, MinimumSizeUnit, out var value))
        {
            ErrorMessage = "Minimum size must convert exactly to a non-negative whole number of bytes, at most 9,223,372,036,854,775,807 bytes. Use a decimal point for fractions.";
            filter = new DuplicateFolderGroupFilter(string.Empty, "0");
            return false;
        }
        ErrorMessage = null;
        filter = new DuplicateFolderGroupFilter(search, value.ToString(CultureInfo.InvariantCulture));
        return true;
    }

    private void AcceptFilter(DuplicateFolderGroupFilter filter)
    {
        _appliedFilter = filter;
        _appliedSortField = _sortField;
        _appliedSortDirection = _sortDirection;
        OnPropertyChanged(nameof(HasAppliedFilters));
        OnPropertyChanged(nameof(AppliedFilterSummaryText));
        OnPropertyChanged(nameof(AppliedFilterBriefText));
        OnPropertyChanged(nameof(EmptyStateTitle));
    }

    private void CopyPath(DuplicateFolderMemberListItemViewModel? member)
    {
        if (member is null) return;
        try { _clipboard.CopyText(member.DisplayPath); DetailErrorMessage = null; }
        catch (Exception exception) { DetailErrorMessage = exception.Message; }
    }

    private bool CanRevealInExplorer(DuplicateFolderMemberListItemViewModel? member) =>
        member is not null
        && !IsExplorerCommandRunning
        && Run?.Status == "completed"
        && SelectedGroup is not null
        && Members.Any(current => current.Id == member.Id && current.Path == member.Path);

    private async Task RevealAsync(DuplicateFolderMemberListItemViewModel? member)
    {
        if (!CanRevealInExplorer(member)
            || member is null
            || Run is not { } run
            || SelectedGroup is not { } group)
        {
            return;
        }

        CancelExplorerCommand(clearFeedback: true);
        _explorerCancellation = new CancellationTokenSource();
        var cancellationToken = _explorerCancellation.Token;
        var generation = _explorerGeneration;
        var memberGeneration = _memberGeneration;
        var runId = run.Id;
        var groupId = group.Id;
        IsExplorerCommandRunning = true;
        ExplorerStatusMessage = $"Opening {member.LocationLabel} in File Explorer…";
        try
        {
            await _explorer.RevealAsync(member.Path, cancellationToken);
            if (!IsCurrentExplorerContext(
                    member,
                    runId,
                    groupId,
                    memberGeneration,
                    generation,
                    cancellationToken))
            {
                return;
            }

            ExplorerErrorMessage = null;
            ExplorerStatusMessage = $"File Explorer opened and selected {member.LocationLabel}.";
            ExplorerStatusAnnouncementVersion++;
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (!IsCurrentExplorerContext(
                    member,
                    runId,
                    groupId,
                    memberGeneration,
                    generation,
                    cancellationToken))
            {
                return;
            }

            ExplorerStatusMessage = null;
            ExplorerErrorMessage = $"Could not show {member.LocationLabel} in File Explorer. "
                + $"Verify that the location is available, then try again. {exception.Message}";
            ExplorerErrorAnnouncementVersion++;
        }
        finally
        {
            if (generation == _explorerGeneration)
            {
                IsExplorerCommandRunning = false;
            }
        }
    }

    public bool CanSelectPageInExplorer =>
        !IsExplorerCommandRunning
        && Run?.Status == "completed"
        && SelectedGroup is not null
        && Members.Count is > 1 and <= IExplorerService.MaximumSelectionItems;

    private async Task SelectPageInExplorerAsync()
    {
        if (!CanSelectPageInExplorer
            || Run is not { } run
            || SelectedGroup is not { } group)
        {
            return;
        }

        var page = Members
            .Take(IExplorerService.MaximumSelectionItems)
            .Select(member => new ExplorerPageMember(member.Id, member.Path))
            .ToArray();
        var selectedMember = SelectedMember is { } selected
            ? new ExplorerPageMember(selected.Id, selected.Path)
            : null;
        CancelExplorerCommand(clearFeedback: true);
        _explorerCancellation = new CancellationTokenSource();
        var cancellationToken = _explorerCancellation.Token;
        var generation = _explorerGeneration;
        var memberGeneration = _memberGeneration;
        var runId = run.Id;
        var groupId = group.Id;
        IsExplorerCommandRunning = true;
        ExplorerStatusMessage =
            $"Selecting {FormatCount(page.Length, "folder copy", "folder copies")} from this page in File Explorer…";
        try
        {
            var result = await _explorer.SelectByParentAsync(
                page.Select(member => member.Path).ToArray(),
                cancellationToken);
            if (!IsCurrentExplorerPageContext(
                    page,
                    selectedMember,
                    runId,
                    groupId,
                    memberGeneration,
                    generation,
                    cancellationToken))
            {
                return;
            }

            PublishExplorerSelectionResult(result);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (!IsCurrentExplorerPageContext(
                    page,
                    selectedMember,
                    runId,
                    groupId,
                    memberGeneration,
                    generation,
                    cancellationToken))
            {
                return;
            }

            ExplorerStatusMessage = null;
            ExplorerErrorMessage = "Could not select this folder-copy page in File Explorer. "
                + $"Verify that its parent locations are available, then try again. {exception.Message}";
            ExplorerErrorAnnouncementVersion++;
        }
        finally
        {
            if (generation == _explorerGeneration)
            {
                IsExplorerCommandRunning = false;
            }
        }
    }

    private void PublishExplorerSelectionResult(ExplorerSelectionResult result)
    {
        if (result.Failures.Count == 0)
        {
            ExplorerErrorMessage = null;
            ExplorerStatusMessage =
                $"File Explorer selected {FormatCount(result.SelectedItemCount, "folder copy", "folder copies")} "
                + $"in {FormatCount(result.ParentCount, "parent location", "parent locations")} from this page.";
            ExplorerStatusAnnouncementVersion++;
            return;
        }

        ExplorerStatusMessage = result.SelectedItemCount == 0
            ? null
            : $"File Explorer selected {result.SelectedItemCount:N0} of {result.RequestedItemCount:N0} folder copies "
                + $"in {result.SuccessfulParentCount:N0} of {result.ParentCount:N0} parent locations from this page.";
        if (ExplorerStatusMessage is not null)
        {
            ExplorerStatusAnnouncementVersion++;
        }

        var failureDetails = string.Join(
            "; ",
            result.Failures.Take(3).Select(failure =>
                $"{CompactExplorerDetail(DisplayPaths.Plain(failure.ParentPath))} ({CompactExplorerDetail(failure.ErrorMessage)})"));
        if (result.Failures.Count > 3)
        {
            failureDetails += $"; plus {result.Failures.Count - 3:N0} more parent locations";
        }
        ExplorerErrorMessage = result.SelectedItemCount == 0
            ? $"File Explorer could not select any of the {FormatCount(result.RequestedItemCount, "folder copy", "folder copies")} "
                + $"in {result.ParentCount:N0} parent locations. Failed locations: {failureDetails}. "
                + "Verify that the locations are available, then try this current page again."
            : $"File Explorer could not select {FormatCount(result.FailedItemCount, "folder copy", "folder copies")} "
                + $"in {result.Failures.Count:N0} of {result.ParentCount:N0} parent locations. "
                + $"Failed locations: {failureDetails}. Verify that the locations are available, "
                + "then try this current page again.";
        ExplorerErrorAnnouncementVersion++;
    }

    private bool IsCurrentExplorerPageContext(
        IReadOnlyList<ExplorerPageMember> page,
        ExplorerPageMember? selectedMember,
        long runId,
        long groupId,
        long memberGeneration,
        long explorerGeneration,
        CancellationToken cancellationToken) =>
        explorerGeneration == _explorerGeneration
        && memberGeneration == _memberGeneration
        && !cancellationToken.IsCancellationRequested
        && Run?.Id == runId
        && SelectedGroup?.Id == groupId
        && SelectedMember?.Id == selectedMember?.Id
        && SelectedMember?.Path == selectedMember?.Path
        && Members.Count == page.Count
        && Members.Select(member => new ExplorerPageMember(member.Id, member.Path)).SequenceEqual(page);

    private static string CompactExplorerDetail(string value)
    {
        const int maximumLength = 160;
        return value.Length <= maximumLength
            ? value
            : $"…{value[^(maximumLength - 1)..]}";
    }

    private bool IsCurrentExplorerContext(
        DuplicateFolderMemberListItemViewModel member,
        long runId,
        long groupId,
        long memberGeneration,
        long explorerGeneration,
        CancellationToken cancellationToken) =>
        explorerGeneration == _explorerGeneration
        && memberGeneration == _memberGeneration
        && !cancellationToken.IsCancellationRequested
        && Run?.Id == runId
        && SelectedGroup?.Id == groupId
        && SelectedMember?.Id == member.Id
        && SelectedMember.Path == member.Path
        && Members.Any(current => current.Id == member.Id && current.Path == member.Path);

    private void ResetQueries()
    {
        CancelGroupQuery();
        CancelMemberQuery();
        CancelReviewQuery();
        _groupCache.Clear();
        _memberCache.Clear();
        _currentGroupPage = null;
        _currentMemberPage = null;
        Groups = [];
        Members = [];
        SelectedGroup = null;
        SelectedMember = null;
        TotalGroups = 0;
        TotalMembers = 0;
        _isReviewLoaded = false;
        ReviewPlan = new WorkerReviewPlanView(
            new WorkerReviewPlan(null, Run?.Id ?? 0, "notCreated", 0, null, null),
            new WorkerReviewPlanSummary(0, 0, 0, 0, "0", 0));
        SelectedReviewSummary = new WorkerReviewFolderGroupSummary(0, 0, 0, 0, 0);
        ErrorMessage = null;
        DetailErrorMessage = null;
    }

    private void CancelGroupQuery()
    {
        _groupCancellation?.Cancel();
        _groupCancellation?.Dispose();
        _groupCancellation = null;
        _groupGeneration++;
    }

    private void CancelMemberQuery()
    {
        CancelExplorerCommand(clearFeedback: true);
        _memberCancellation?.Cancel();
        _memberCancellation?.Dispose();
        _memberCancellation = null;
        _memberGeneration++;
    }

    private void CancelExplorerCommand(bool clearFeedback)
    {
        _explorerCancellation?.Cancel();
        _explorerCancellation?.Dispose();
        _explorerCancellation = null;
        _explorerGeneration++;
        IsExplorerCommandRunning = false;
        if (clearFeedback)
        {
            ExplorerStatusMessage = null;
            ExplorerErrorMessage = null;
        }
    }

    private void CancelReviewQuery()
    {
        _reviewCancellation?.Cancel();
        _reviewCancellation?.Dispose();
        _reviewCancellation = null;
        _reviewGeneration++;
    }

    private void PublishGroupQueryAnnouncement()
    {
        if (HasError)
        {
            PublishGroupErrorAnnouncement("Duplicate folder results could not be loaded.");
            return;
        }

        if (Run?.Status != "completed")
        {
            return;
        }

        GroupStatusAnnouncement = TotalGroups == 0
            ? "Duplicate folder query complete. No matching exact duplicate folder groups."
            : $"Duplicate folder query complete. {FormatCount(TotalGroups, "matching exact duplicate folder group", "matching exact duplicate folder groups")}.";
        GroupStatusAnnouncementVersion++;
    }

    private void PublishGroupErrorAnnouncement(string prefix)
    {
        if (!HasError)
        {
            return;
        }

        GroupErrorAnnouncement = $"{prefix} {ErrorMessage}";
        GroupErrorAnnouncementVersion++;
    }

    private void PublishMemberQueryAnnouncement()
    {
        if (SelectedGroup is null || Run?.Status != "completed")
        {
            return;
        }

        if (HasDetailError)
        {
            MemberErrorAnnouncementVersion++;
            return;
        }

        MemberStatusAnnouncement = TotalMembers == 0
            ? "Selected exact duplicate folder group loaded. No folder copies to display."
            : $"Selected exact duplicate folder group loaded. {MemberPageStatusText}. "
                + "Use the folder-copy comparison list; shared context and differing path segments describe this page.";
        MemberStatusAnnouncementVersion++;
    }

    private static string FormatCount(long value, string singular, string plural) =>
        $"{value:N0} {(value == 1 ? singular : plural)}";

    private sealed record ExplorerPageMember(long Id, string Path);

    private void RaiseState()
    {
        OnPropertyChanged(nameof(GroupCountText));
        OnPropertyChanged(nameof(IsEmpty));
        OnPropertyChanged(nameof(EmptyStateTitle));
        OnPropertyChanged(nameof(UnavailableTitle));
        OnPropertyChanged(nameof(IsInitialError));
        OnPropertyChanged(nameof(HasRetainedGroupError));
    }

    private void RaiseGroupPaging()
    {
        OnPropertyChanged(nameof(CanMoveNext));
        OnPropertyChanged(nameof(CanMovePrevious));
        NextPageCommand.NotifyCanExecuteChanged();
        PreviousPageCommand.NotifyCanExecuteChanged();
    }

    private void RaiseMemberPaging()
    {
        OnPropertyChanged(nameof(CanMoveMembersNext));
        OnPropertyChanged(nameof(CanMoveMembersPrevious));
        NextMemberPageCommand.NotifyCanExecuteChanged();
        PreviousMemberPageCommand.NotifyCanExecuteChanged();
    }

    // IsDetailEmpty is computed from SelectedGroup, IsDetailLoading, HasDetailError and
    // TotalMembers; every setter that can change one of those four raises it here so the
    // binding can never go stale behind an unraised dependency.
    private void RaiseDetailEmptyState() => OnPropertyChanged(nameof(IsDetailEmpty));
}
