using System.Globalization;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class DuplicateFilesViewModel : ObservableObject, IDisposable
{
    public const int PageSize = 200;
    public const int CacheCapacity = 5;

    private readonly IWorkerClient _workerClient;
    private readonly IClipboardService _clipboard;
    private readonly IExplorerService _explorer;
    private readonly BoundedCursorCache<WorkerDuplicateFileGroupPage> _groupCache = new(CacheCapacity);
    private readonly BoundedCursorCache<WorkerDuplicateFileMemberPage> _memberCache = new(CacheCapacity);
    private CancellationTokenSource? _groupCancellation;
    private CancellationTokenSource? _memberCancellation;
    private WorkerRun? _run;
    private IReadOnlyList<DuplicateFileGroupListItemViewModel> _groups = [];
    private IReadOnlyList<DuplicateFileMemberListItemViewModel> _members = [];
    private DuplicateFileGroupListItemViewModel? _selectedGroup;
    private WorkerDuplicateFileGroupPage? _currentGroupPage;
    private WorkerDuplicateFileMemberPage? _currentMemberPage;
    private WorkerDuplicateFileReviewSummary _summary = new(0, 0, "0", "0");
    private long _groupGeneration;
    private long _memberGeneration;
    private string _searchText = string.Empty;
    private string _minimumSizeText = string.Empty;
    private bool _acrossDrives;
    private string? _errorMessage;
    private string? _detailErrorMessage;
    private string _stateMessage = "Select a completed run to browse duplicate files.";
    private bool _isLoading;
    private bool _isDetailLoading;
    private DuplicateFileGroupSortField _sortField = DuplicateFileGroupSortField.RecoverableBytes;
    private WorkerSortDirection _sortDirection = WorkerSortDirection.Descending;
    private long _totalGroups;
    private long _totalMembers;
    private bool _disposed;

    public DuplicateFilesViewModel(
        IWorkerClient workerClient,
        IClipboardService clipboard,
        IExplorerService explorer)
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
        CopyPathCommand = new RelayCommand<DuplicateFileMemberListItemViewModel>(CopyPath);
        RevealInExplorerCommand = new AsyncRelayCommand<DuplicateFileMemberListItemViewModel>(RevealInExplorerAsync);
    }

    public WorkerRun? Run
    {
        get => _run;
        private set => SetProperty(ref _run, value);
    }

    public IReadOnlyList<DuplicateFileGroupListItemViewModel> Groups
    {
        get => _groups;
        private set
        {
            if (SetProperty(ref _groups, value))
            {
                OnPropertyChanged(nameof(HasGroups));
                OnPropertyChanged(nameof(IsLoadingOverlayVisible));
            }
        }
    }

    public IReadOnlyList<DuplicateFileMemberListItemViewModel> Members
    {
        get => _members;
        private set => SetProperty(ref _members, value);
    }

    public DuplicateFileGroupListItemViewModel? SelectedGroup
    {
        get => _selectedGroup;
        set
        {
            if (SetProperty(ref _selectedGroup, value))
            {
                OnPropertyChanged(nameof(HasSelectedGroup));
                _ = LoadSelectedGroupAsync(value);
            }
        }
    }

    public string SearchText
    {
        get => _searchText;
        set => SetProperty(ref _searchText, value);
    }

    public string MinimumSizeText
    {
        get => _minimumSizeText;
        set => SetProperty(ref _minimumSizeText, value);
    }

    public bool AcrossDrives
    {
        get => _acrossDrives;
        set => SetProperty(ref _acrossDrives, value);
    }

    public string? ErrorMessage
    {
        get => _errorMessage;
        private set
        {
            if (SetProperty(ref _errorMessage, value))
            {
                OnPropertyChanged(nameof(HasError));
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
            }
        }
    }

    public string StateMessage
    {
        get => _stateMessage;
        private set => SetProperty(ref _stateMessage, value);
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
                RaisePagingProperties();
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
                RaiseMemberPagingProperties();
            }
        }
    }

    public long TotalGroups
    {
        get => _totalGroups;
        private set
        {
            if (SetProperty(ref _totalGroups, value))
            {
                OnPropertyChanged(nameof(GroupCountText));
                OnPropertyChanged(nameof(IsEmpty));
            }
        }
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

    public WorkerDuplicateFileReviewSummary Summary
    {
        get => _summary;
        private set
        {
            if (SetProperty(ref _summary, value))
            {
                OnPropertyChanged(nameof(MatchingSetCountText));
                OnPropertyChanged(nameof(MatchingCopyCountText));
                OnPropertyChanged(nameof(PotentialRecoverableText));
                OnPropertyChanged(nameof(LargestOpportunityText));
            }
        }
    }

    public DuplicateFileGroupSortField SortField => _sortField;

    public WorkerSortDirection SortDirection => _sortDirection;

    public string GroupCountText => $"{TotalGroups:N0} groups";

    public string MemberCountText => $"{TotalMembers:N0} copies";

    public string MatchingSetCountText => $"{Summary.MatchingGroupCount:N0}";

    public string MatchingCopyCountText => $"{Summary.MatchingCopyCount:N0}";

    public string PotentialRecoverableText => DisplayFormatting.Bytes(Summary.PotentialRecoverableBytes);

    public string LargestOpportunityText => DisplayFormatting.Bytes(Summary.LargestRecoverableBytes);

    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);

    public bool HasDetailError => !string.IsNullOrWhiteSpace(DetailErrorMessage);

    public bool IsUnavailable => Run is null || Run.Status != "completed";

    public bool IsEmpty => Run?.Status == "completed" && !IsLoading && !HasError && TotalGroups == 0;

    public bool HasGroups => Groups.Count > 0;

    public bool IsLoadingOverlayVisible => IsLoading && !HasGroups;

    public bool HasSelectedGroup => SelectedGroup is not null;

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

    public IRelayCommand<DuplicateFileMemberListItemViewModel> CopyPathCommand { get; }

    public IAsyncRelayCommand<DuplicateFileMemberListItemViewModel> RevealInExplorerCommand { get; }

    public async Task ShowRunAsync(WorkerRun? run, CancellationToken cancellationToken = default)
    {
        Run = run;
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
        Summary = new WorkerDuplicateFileReviewSummary(0, 0, "0", "0");
        ErrorMessage = null;
        DetailErrorMessage = null;
        OnPropertyChanged(nameof(IsUnavailable));
        OnPropertyChanged(nameof(IsEmpty));
        OnPropertyChanged(nameof(HasGroups));

        if (run is null)
        {
            StateMessage = "Select a completed run to browse duplicate files.";
            return;
        }
        if (run.Status != "completed")
        {
            StateMessage = run.Status is "running" or "pending" or "cancelling"
                ? "Duplicate results become available after this scan completes."
                : $"This run is {DisplayFormatting.Status(run.Status).ToLowerInvariant()}; partial results are not shown.";
            return;
        }

        StateMessage = "No duplicate files matched this run and filter.";
        await ResetAndLoadGroupsAsync(cancellationToken);
    }

    public async Task ApplySortAsync(
        DuplicateFileGroupSortField field,
        WorkerSortDirection direction,
        CancellationToken cancellationToken = default)
    {
        if (_sortField == field && _sortDirection == direction)
        {
            return;
        }
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
        if (Run?.Id == run.Id)
        {
            _ = ShowRunAsync(run);
        }
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }
        _disposed = true;
        CancelGroupQuery();
        CancelMemberQuery();
    }

    private async Task ApplyFiltersAsync()
    {
        if (Run?.Status == "completed")
        {
            await ResetAndLoadGroupsAsync();
        }
    }

    private async Task ClearFiltersAsync()
    {
        SearchText = string.Empty;
        MinimumSizeText = string.Empty;
        AcrossDrives = false;
        if (Run?.Status == "completed")
        {
            await ResetAndLoadGroupsAsync();
        }
    }

    private async Task ResetAndLoadGroupsAsync(
        CancellationToken cancellationToken = default,
        bool preserveDisplayedResults = false)
    {
        if (!TryBuildFilter(out var filter))
        {
            return;
        }
        CancelGroupQuery();
        CancelMemberQuery();
        _groupCancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var generation = ++_groupGeneration;
        IsLoading = true;
        _groupCache.Clear();
        _memberCache.Clear();
        _currentGroupPage = null;
        _currentMemberPage = null;
        if (!preserveDisplayedResults)
        {
            Groups = [];
            Members = [];
            SelectedGroup = null;
            TotalGroups = 0;
            TotalMembers = 0;
            Summary = new WorkerDuplicateFileReviewSummary(0, 0, "0", "0");
        }
        ErrorMessage = null;
        DetailErrorMessage = null;
        await LoadGroupPageAsync(null, filter, generation, _groupCancellation.Token, display: true);
    }

    private async Task LoadGroupPageAsync(
        string? cursor,
        DuplicateFileGroupFilter filter,
        long generation,
        CancellationToken cancellationToken,
        bool display)
    {
        if (_groupCache.TryGet(cursor, out var cached))
        {
            if (display && generation == _groupGeneration)
            {
                DisplayGroupPage(cached);
                _ = PrefetchGroupNeighborsAsync(cached, filter, generation, cancellationToken);
            }
            return;
        }
        if (Run is not { Status: "completed" } run)
        {
            return;
        }
        if (display)
        {
            IsLoading = true;
            ErrorMessage = null;
        }
        try
        {
            var page = await _workerClient.GetDuplicateFileGroupsAsync(
                new DuplicateFileGroupQuery(
                    run.Id,
                    PageSize,
                    _sortField,
                    _sortDirection,
                    filter,
                    cursor),
                cancellationToken);
            if (generation != _groupGeneration || cancellationToken.IsCancellationRequested)
            {
                return;
            }
            _groupCache.Set(cursor, page);
            if (display)
            {
                DisplayGroupPage(page);
                _ = PrefetchGroupNeighborsAsync(page, filter, generation, cancellationToken);
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (display && generation == _groupGeneration)
            {
                ErrorMessage = exception.Message;
            }
        }
        finally
        {
            if (display && generation == _groupGeneration)
            {
                IsLoading = false;
                OnPropertyChanged(nameof(IsEmpty));
            }
        }
    }

    private void DisplayGroupPage(WorkerDuplicateFileGroupPage page)
    {
        _currentGroupPage = page;
        TotalGroups = page.Total;
        Summary = page.Summary;
        Groups = page.Groups.Select(group => new DuplicateFileGroupListItemViewModel(group)).ToArray();
        OnPropertyChanged(nameof(HasGroups));
        OnPropertyChanged(nameof(IsEmpty));
        SelectedGroup = Groups.FirstOrDefault();
        RaisePagingProperties();
    }

    private async Task PrefetchGroupNeighborsAsync(
        WorkerDuplicateFileGroupPage page,
        DuplicateFileGroupFilter filter,
        long generation,
        CancellationToken cancellationToken)
    {
        await PrefetchGroupDirectionAsync(page.PreviousCursor, false, 2, filter, generation, cancellationToken);
        await PrefetchGroupDirectionAsync(page.NextCursor, true, 2, filter, generation, cancellationToken);
    }

    private async Task PrefetchGroupDirectionAsync(
        string? cursor,
        bool forward,
        int remaining,
        DuplicateFileGroupFilter filter,
        long generation,
        CancellationToken cancellationToken)
    {
        if (cursor is null || remaining == 0 || generation != _groupGeneration || cancellationToken.IsCancellationRequested)
        {
            return;
        }
        if (!_groupCache.TryGet(cursor, out var page))
        {
            await LoadGroupPageAsync(cursor, filter, generation, cancellationToken, display: false);
            if (!_groupCache.TryGet(cursor, out page))
            {
                return;
            }
        }
        await PrefetchGroupDirectionAsync(
            forward ? page.NextCursor : page.PreviousCursor,
            forward,
            remaining - 1,
            filter,
            generation,
            cancellationToken);
    }

    private async Task NextPageAsync()
    {
        if (_currentGroupPage?.NextCursor is { } cursor && TryBuildFilter(out var filter) && _groupCancellation is not null)
        {
            await LoadGroupPageAsync(cursor, filter, _groupGeneration, _groupCancellation.Token, display: true);
        }
    }

    private async Task PreviousPageAsync()
    {
        if (_currentGroupPage?.PreviousCursor is { } cursor && TryBuildFilter(out var filter) && _groupCancellation is not null)
        {
            await LoadGroupPageAsync(cursor, filter, _groupGeneration, _groupCancellation.Token, display: true);
        }
    }

    private async Task LoadSelectedGroupAsync(DuplicateFileGroupListItemViewModel? group)
    {
        CancelMemberQuery();
        _memberCache.Clear();
        _currentMemberPage = null;
        Members = [];
        TotalMembers = 0;
        DetailErrorMessage = null;
        OnPropertyChanged(nameof(IsDetailEmpty));
        if (Run is not { Status: "completed" } run || group is null)
        {
            return;
        }
        _memberCancellation = new CancellationTokenSource();
        var generation = ++_memberGeneration;
        await LoadMemberPageAsync(null, run.Id, group.Id, generation, _memberCancellation.Token, display: true);
    }

    private async Task LoadMemberPageAsync(
        string? cursor,
        long runId,
        long groupId,
        long generation,
        CancellationToken cancellationToken,
        bool display)
    {
        if (_memberCache.TryGet(cursor, out var cached))
        {
            if (display && generation == _memberGeneration)
            {
                DisplayMemberPage(cached);
                _ = PrefetchMemberNeighborsAsync(cached, runId, groupId, generation, cancellationToken);
            }
            return;
        }
        if (display)
        {
            IsDetailLoading = true;
            DetailErrorMessage = null;
        }
        try
        {
            var page = await _workerClient.GetDuplicateFileGroupMembersAsync(
                new DuplicateFileMemberQuery(
                    runId,
                    groupId,
                    PageSize,
                    DuplicateFileMemberSortField.Path,
                    WorkerSortDirection.Ascending,
                    new DuplicateFileMemberFilter(string.Empty),
                    cursor),
                cancellationToken);
            if (generation != _memberGeneration || cancellationToken.IsCancellationRequested)
            {
                return;
            }
            _memberCache.Set(cursor, page);
            if (display)
            {
                DisplayMemberPage(page);
                _ = PrefetchMemberNeighborsAsync(page, runId, groupId, generation, cancellationToken);
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (display && generation == _memberGeneration)
            {
                DetailErrorMessage = exception.Message;
            }
        }
        finally
        {
            if (display && generation == _memberGeneration)
            {
                IsDetailLoading = false;
                OnPropertyChanged(nameof(IsDetailEmpty));
            }
        }
    }

    private void DisplayMemberPage(WorkerDuplicateFileMemberPage page)
    {
        _currentMemberPage = page;
        TotalMembers = page.Total;
        Members = page.Members.Select(member => new DuplicateFileMemberListItemViewModel(member)).ToArray();
        OnPropertyChanged(nameof(IsDetailEmpty));
        RaiseMemberPagingProperties();
    }

    private async Task PrefetchMemberNeighborsAsync(
        WorkerDuplicateFileMemberPage page,
        long runId,
        long groupId,
        long generation,
        CancellationToken cancellationToken)
    {
        await PrefetchMemberDirectionAsync(page.PreviousCursor, false, 2, runId, groupId, generation, cancellationToken);
        await PrefetchMemberDirectionAsync(page.NextCursor, true, 2, runId, groupId, generation, cancellationToken);
    }

    private async Task PrefetchMemberDirectionAsync(
        string? cursor,
        bool forward,
        int remaining,
        long runId,
        long groupId,
        long generation,
        CancellationToken cancellationToken)
    {
        if (cursor is null || remaining == 0 || generation != _memberGeneration || cancellationToken.IsCancellationRequested)
        {
            return;
        }
        if (!_memberCache.TryGet(cursor, out var page))
        {
            await LoadMemberPageAsync(cursor, runId, groupId, generation, cancellationToken, display: false);
            if (!_memberCache.TryGet(cursor, out page))
            {
                return;
            }
        }
        await PrefetchMemberDirectionAsync(
            forward ? page.NextCursor : page.PreviousCursor,
            forward,
            remaining - 1,
            runId,
            groupId,
            generation,
            cancellationToken);
    }

    private async Task NextMemberPageAsync()
    {
        if (Run is { } run
            && SelectedGroup is { } group
            && _currentMemberPage?.NextCursor is { } cursor
            && _memberCancellation is not null)
        {
            await LoadMemberPageAsync(cursor, run.Id, group.Id, _memberGeneration, _memberCancellation.Token, display: true);
        }
    }

    private async Task PreviousMemberPageAsync()
    {
        if (Run is { } run
            && SelectedGroup is { } group
            && _currentMemberPage?.PreviousCursor is { } cursor
            && _memberCancellation is not null)
        {
            await LoadMemberPageAsync(cursor, run.Id, group.Id, _memberGeneration, _memberCancellation.Token, display: true);
        }
    }

    private bool TryBuildFilter(out DuplicateFileGroupFilter filter)
    {
        var search = SearchText.Trim();
        if (search.Length > 512)
        {
            ErrorMessage = "Path search may contain at most 512 characters.";
            filter = new DuplicateFileGroupFilter(string.Empty, "0", false);
            return false;
        }
        var minimum = MinimumSizeText.Trim();
        if (minimum.Length == 0)
        {
            minimum = "0";
        }
        if (!long.TryParse(minimum, NumberStyles.None, CultureInfo.InvariantCulture, out var value) || value < 0)
        {
            ErrorMessage = "Minimum size must be a non-negative whole number of bytes.";
            filter = new DuplicateFileGroupFilter(string.Empty, "0", false);
            return false;
        }
        ErrorMessage = null;
        filter = new DuplicateFileGroupFilter(
            search,
            value.ToString(CultureInfo.InvariantCulture),
            AcrossDrives);
        return true;
    }

    private void CopyPath(DuplicateFileMemberListItemViewModel? member)
    {
        if (member is null)
        {
            return;
        }
        try
        {
            _clipboard.CopyText(member.Path);
            DetailErrorMessage = null;
        }
        catch (Exception exception)
        {
            DetailErrorMessage = exception.Message;
        }
    }

    private async Task RevealInExplorerAsync(DuplicateFileMemberListItemViewModel? member)
    {
        if (member is null)
        {
            return;
        }
        try
        {
            await _explorer.RevealAsync(member.Path);
            DetailErrorMessage = null;
        }
        catch (Exception exception)
        {
            DetailErrorMessage = exception.Message;
        }
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

    private void RaisePagingProperties()
    {
        OnPropertyChanged(nameof(CanMoveNext));
        OnPropertyChanged(nameof(CanMovePrevious));
        NextPageCommand.NotifyCanExecuteChanged();
        PreviousPageCommand.NotifyCanExecuteChanged();
    }

    private void RaiseMemberPagingProperties()
    {
        OnPropertyChanged(nameof(CanMoveMembersNext));
        OnPropertyChanged(nameof(CanMoveMembersPrevious));
        NextMemberPageCommand.NotifyCanExecuteChanged();
        PreviousMemberPageCommand.NotifyCanExecuteChanged();
    }
}
