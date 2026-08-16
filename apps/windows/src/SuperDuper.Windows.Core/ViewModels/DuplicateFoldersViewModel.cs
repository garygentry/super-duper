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
    private WorkerRun? _run;
    private IReadOnlyList<DuplicateFolderGroupListItemViewModel> _groups = [];
    private IReadOnlyList<DuplicateFolderMemberListItemViewModel> _members = [];
    private DuplicateFolderGroupListItemViewModel? _selectedGroup;
    private WorkerDuplicateFolderGroupPage? _currentGroupPage;
    private WorkerDuplicateFolderMemberPage? _currentMemberPage;
    private long _groupGeneration;
    private long _memberGeneration;
    private long _totalGroups;
    private long _totalMembers;
    private string _searchText = string.Empty;
    private string _minimumSizeText = string.Empty;
    private string _stateMessage = "Select a completed run to browse duplicate folders.";
    private string? _errorMessage;
    private string? _detailErrorMessage;
    private bool _isLoading;
    private bool _isDetailLoading;
    private DuplicateFolderGroupSortField _sortField = DuplicateFolderGroupSortField.TotalBytes;
    private WorkerSortDirection _sortDirection = WorkerSortDirection.Descending;
    private bool _disposed;

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
        RevealInExplorerCommand = new AsyncRelayCommand<DuplicateFolderMemberListItemViewModel>(RevealAsync);
    }

    public WorkerRun? Run { get => _run; private set => SetProperty(ref _run, value); }
    public IReadOnlyList<DuplicateFolderGroupListItemViewModel> Groups { get => _groups; private set => SetProperty(ref _groups, value); }
    public IReadOnlyList<DuplicateFolderMemberListItemViewModel> Members { get => _members; private set => SetProperty(ref _members, value); }
    public DuplicateFolderGroupListItemViewModel? SelectedGroup
    {
        get => _selectedGroup;
        set
        {
            if (SetProperty(ref _selectedGroup, value))
            {
                _ = LoadSelectedGroupAsync(value);
            }
        }
    }
    public string SearchText { get => _searchText; set => SetProperty(ref _searchText, value); }
    public string MinimumSizeText { get => _minimumSizeText; set => SetProperty(ref _minimumSizeText, value); }
    public string StateMessage { get => _stateMessage; private set => SetProperty(ref _stateMessage, value); }
    public string? ErrorMessage
    {
        get => _errorMessage;
        private set { if (SetProperty(ref _errorMessage, value)) OnPropertyChanged(nameof(HasError)); }
    }
    public string? DetailErrorMessage
    {
        get => _detailErrorMessage;
        private set { if (SetProperty(ref _detailErrorMessage, value)) OnPropertyChanged(nameof(HasDetailError)); }
    }
    public bool IsLoading
    {
        get => _isLoading;
        private set { if (SetProperty(ref _isLoading, value)) RaiseGroupPaging(); }
    }
    public bool IsDetailLoading
    {
        get => _isDetailLoading;
        private set { if (SetProperty(ref _isDetailLoading, value)) RaiseMemberPaging(); }
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
                OnPropertyChanged(nameof(IsDetailEmpty));
            }
        }
    }
    public DuplicateFolderGroupSortField SortField => _sortField;
    public WorkerSortDirection SortDirection => _sortDirection;
    public string GroupCountText => $"{TotalGroups:N0} groups";
    public string MemberCountText => $"{TotalMembers:N0} folders";
    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);
    public bool HasDetailError => !string.IsNullOrWhiteSpace(DetailErrorMessage);
    public bool IsUnavailable => Run is null || Run.Status != "completed";
    public bool IsEmpty => Run?.Status == "completed" && !IsLoading && !HasError && TotalGroups == 0;
    public bool IsDetailEmpty => SelectedGroup is not null && !IsDetailLoading && !HasDetailError && TotalMembers == 0;
    public bool CanMoveNext => !IsLoading && _currentGroupPage?.NextCursor is not null;
    public bool CanMovePrevious => !IsLoading && _currentGroupPage?.PreviousCursor is not null;
    public bool CanMoveMembersNext => !IsDetailLoading && _currentMemberPage?.NextCursor is not null;
    public bool CanMoveMembersPrevious => !IsDetailLoading && _currentMemberPage?.PreviousCursor is not null;
    public int CachedGroupPageCount => _groupCache.Count;
    public int CachedMemberPageCount => _memberCache.Count;

    public IAsyncRelayCommand ApplyFiltersCommand { get; }
    public IAsyncRelayCommand ClearFiltersCommand { get; }
    public IAsyncRelayCommand NextPageCommand { get; }
    public IAsyncRelayCommand PreviousPageCommand { get; }
    public IAsyncRelayCommand NextMemberPageCommand { get; }
    public IAsyncRelayCommand PreviousMemberPageCommand { get; }
    public IRelayCommand<DuplicateFolderMemberListItemViewModel> CopyPathCommand { get; }
    public IAsyncRelayCommand<DuplicateFolderMemberListItemViewModel> RevealInExplorerCommand { get; }

    public async Task ShowRunAsync(WorkerRun? run, CancellationToken cancellationToken = default)
    {
        Run = run;
        ResetQueries();
        OnPropertyChanged(nameof(IsUnavailable));
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
        StateMessage = "No exact duplicate folders matched this run and filter.";
        await ResetAndLoadGroupsAsync(cancellationToken);
    }

    public async Task ApplySortAsync(DuplicateFolderGroupSortField field, WorkerSortDirection direction, CancellationToken cancellationToken = default)
    {
        if (_sortField == field && _sortDirection == direction) return;
        _sortField = field;
        _sortDirection = direction;
        OnPropertyChanged(nameof(SortField));
        OnPropertyChanged(nameof(SortDirection));
        if (Run?.Status == "completed") await ResetAndLoadGroupsAsync(cancellationToken);
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
    }

    private Task ApplyFiltersAsync() => Run?.Status == "completed" ? ResetAndLoadGroupsAsync() : Task.CompletedTask;

    private async Task ClearFiltersAsync()
    {
        SearchText = string.Empty;
        MinimumSizeText = string.Empty;
        if (Run?.Status == "completed") await ResetAndLoadGroupsAsync();
    }

    private async Task ResetAndLoadGroupsAsync(CancellationToken cancellationToken = default)
    {
        if (!TryBuildFilter(out var filter)) return;
        ResetQueries();
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
                DisplayGroupPage(cached);
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
                DisplayGroupPage(page);
                _ = PrefetchGroupsAsync(page, filter, generation, token);
            }
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested) { }
        catch (Exception exception) { if (display && generation == _groupGeneration) ErrorMessage = exception.Message; }
        finally { if (display && generation == _groupGeneration) { IsLoading = false; RaiseState(); } }
    }

    private void DisplayGroupPage(WorkerDuplicateFolderGroupPage page)
    {
        _currentGroupPage = page;
        TotalGroups = page.Total;
        Groups = page.Groups.Select(group => new DuplicateFolderGroupListItemViewModel(group)).ToArray();
        SelectedGroup = Groups.FirstOrDefault();
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
        _currentGroupPage?.NextCursor is { } cursor && TryBuildFilter(out var filter) && _groupCancellation is not null
            ? LoadGroupPageAsync(cursor, filter, _groupGeneration, _groupCancellation.Token, true)
            : Task.CompletedTask;

    private Task PreviousPageAsync() =>
        _currentGroupPage?.PreviousCursor is { } cursor && TryBuildFilter(out var filter) && _groupCancellation is not null
            ? LoadGroupPageAsync(cursor, filter, _groupGeneration, _groupCancellation.Token, true)
            : Task.CompletedTask;

    private async Task LoadSelectedGroupAsync(DuplicateFolderGroupListItemViewModel? group)
    {
        CancelMemberQuery();
        _memberCache.Clear();
        _currentMemberPage = null;
        Members = [];
        TotalMembers = 0;
        DetailErrorMessage = null;
        if (Run is not { Status: "completed" } run || group is null) return;
        _memberCancellation = new CancellationTokenSource();
        var generation = ++_memberGeneration;
        await LoadMemberPageAsync(null, run.Id, group.Id, generation, _memberCancellation.Token, true);
    }

    private async Task LoadMemberPageAsync(string? cursor, long runId, long groupId, long generation, CancellationToken token, bool display)
    {
        if (_memberCache.TryGet(cursor, out var cached))
        {
            if (display && generation == _memberGeneration)
            {
                DisplayMemberPage(cached);
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
            if (generation != _memberGeneration || token.IsCancellationRequested) return;
            _memberCache.Set(cursor, page);
            if (display)
            {
                DisplayMemberPage(page);
                _ = PrefetchMembersAsync(page, runId, groupId, generation, token);
            }
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested) { }
        catch (Exception exception) { if (display && generation == _memberGeneration) DetailErrorMessage = exception.Message; }
        finally { if (display && generation == _memberGeneration) { IsDetailLoading = false; OnPropertyChanged(nameof(IsDetailEmpty)); } }
    }

    private void DisplayMemberPage(WorkerDuplicateFolderMemberPage page)
    {
        _currentMemberPage = page;
        TotalMembers = page.Total;
        Members = page.Members.Select(member => new DuplicateFolderMemberListItemViewModel(member)).ToArray();
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

    private Task NextMemberPageAsync() =>
        Run is { } run && SelectedGroup is { } group && _currentMemberPage?.NextCursor is { } cursor && _memberCancellation is not null
            ? LoadMemberPageAsync(cursor, run.Id, group.Id, _memberGeneration, _memberCancellation.Token, true)
            : Task.CompletedTask;

    private Task PreviousMemberPageAsync() =>
        Run is { } run && SelectedGroup is { } group && _currentMemberPage?.PreviousCursor is { } cursor && _memberCancellation is not null
            ? LoadMemberPageAsync(cursor, run.Id, group.Id, _memberGeneration, _memberCancellation.Token, true)
            : Task.CompletedTask;

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
        if (!long.TryParse(minimum, NumberStyles.None, CultureInfo.InvariantCulture, out var value) || value < 0)
        {
            ErrorMessage = "Minimum size must be a non-negative whole number of bytes.";
            filter = new DuplicateFolderGroupFilter(string.Empty, "0");
            return false;
        }
        ErrorMessage = null;
        filter = new DuplicateFolderGroupFilter(search, value.ToString(CultureInfo.InvariantCulture));
        return true;
    }

    private void CopyPath(DuplicateFolderMemberListItemViewModel? member)
    {
        if (member is null) return;
        try { _clipboard.CopyText(member.Path); DetailErrorMessage = null; }
        catch (Exception exception) { DetailErrorMessage = exception.Message; }
    }

    private async Task RevealAsync(DuplicateFolderMemberListItemViewModel? member)
    {
        if (member is null) return;
        try { await _explorer.RevealAsync(member.Path); DetailErrorMessage = null; }
        catch (Exception exception) { DetailErrorMessage = exception.Message; }
    }

    private void ResetQueries()
    {
        CancelGroupQuery();
        CancelMemberQuery();
        _groupCache.Clear();
        _memberCache.Clear();
        _currentGroupPage = null;
        _currentMemberPage = null;
        Groups = [];
        Members = [];
        SelectedGroup = null;
        TotalGroups = 0;
        TotalMembers = 0;
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
        _memberCancellation?.Cancel();
        _memberCancellation?.Dispose();
        _memberCancellation = null;
        _memberGeneration++;
    }

    private void RaiseState()
    {
        OnPropertyChanged(nameof(GroupCountText));
        OnPropertyChanged(nameof(IsEmpty));
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
}
