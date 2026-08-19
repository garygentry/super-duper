using System.Globalization;
using System.Text;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class DuplicateFilesViewModel : ObservableObject, IDisposable
{
    public const int PageSize = 200;
    public const int RootFacetPageSize = 25;
    public const int DriveFacetPageSize = 25;
    public const int CacheCapacity = 5;
    public const long OneGigabyteBytes = 1_073_741_824;
    public const int MaximumSubstringSearchCharacters = 512;
    public const int MaximumExactPathCharacters = 32_767;
    public const int MaximumExtensionCharacters = 255;

    private readonly IWorkerClient _workerClient;
    private readonly IClipboardService _clipboard;
    private readonly IExplorerService _explorer;
    private readonly BoundedCursorCache<WorkerDuplicateFileGroupPage> _groupCache = new(CacheCapacity);
    private readonly BoundedCursorCache<WorkerDuplicateFileMemberPage> _memberCache = new(CacheCapacity);
    private readonly BoundedCursorCache<WorkerDuplicateFileSelectedRootFacetPage> _rootFacetCache = new(CacheCapacity);
    private readonly BoundedCursorCache<WorkerDuplicateFileDriveFacetPage> _driveFacetCache = new(CacheCapacity);
    private CancellationTokenSource? _groupCancellation;
    private CancellationTokenSource? _memberCancellation;
    private CancellationTokenSource? _rootFacetCancellation;
    private CancellationTokenSource? _driveFacetCancellation;
    private WorkerRun? _run;
    private IReadOnlyList<DuplicateFileGroupListItemViewModel> _groups = [];
    private IReadOnlyList<DuplicateFileMemberListItemViewModel> _members = [];
    private IReadOnlyList<DuplicateFileSelectedRootFacetListItemViewModel> _selectedRootFacetOptions =
        [new()];
    private IReadOnlyList<DuplicateFileDriveFacetListItemViewModel> _driveFacetOptions = [new()];
    private DuplicateFileGroupListItemViewModel? _selectedGroup;
    private DuplicateFileSelectedRootFacetListItemViewModel? _selectedRootFacet;
    private DuplicateFileDriveFacetListItemViewModel? _selectedDriveFacet;
    private WorkerDuplicateFileGroupPage? _currentGroupPage;
    private WorkerDuplicateFileMemberPage? _currentMemberPage;
    private WorkerDuplicateFileSelectedRootFacetPage? _currentRootFacetPage;
    private WorkerDuplicateFileDriveFacetPage? _currentDriveFacetPage;
    private WorkerDuplicateFileReviewSummary _summary = new(0, 0, "0", "0");
    private long _groupGeneration;
    private long _memberGeneration;
    private long _rootFacetGeneration;
    private long _driveFacetGeneration;
    private string _searchText = string.Empty;
    private bool _exactPathMatch;
    private string _extensionText = string.Empty;
    private bool _withoutExtension;
    private bool _allMembersMustMatchExtension;
    private string _minimumSizeText = string.Empty;
    private bool _oneGigabyteOrLarger;
    private bool _threeOrMoreCopies;
    private bool _acrossDrives;
    private string? _errorMessage;
    private string? _detailErrorMessage;
    private string _stateMessage = "Select a completed run to browse duplicate files.";
    private bool _isLoading;
    private bool _isDetailLoading;
    private bool _isRootFacetLoading;
    private bool _isDriveFacetLoading;
    private DuplicateFileGroupSortField _sortField = DuplicateFileGroupSortField.RecoverableBytes;
    private WorkerSortDirection _sortDirection = WorkerSortDirection.Descending;
    private DuplicateFileSelectedRootFacetSortField _rootFacetSortField =
        DuplicateFileSelectedRootFacetSortField.MatchingGroupCount;
    private WorkerSortDirection _rootFacetSortDirection = WorkerSortDirection.Descending;
    private DuplicateFileDriveFacetSortField _driveFacetSortField =
        DuplicateFileDriveFacetSortField.MatchingGroupCount;
    private WorkerSortDirection _driveFacetSortDirection = WorkerSortDirection.Descending;
    private long _totalGroups;
    private long _totalMembers;
    private long _totalRootFacets;
    private long _totalDriveFacets;
    private string? _rootFacetErrorMessage;
    private string? _driveFacetErrorMessage;
    private string _groupStatusAnnouncement = "Duplicate file results have not loaded.";
    private string _groupErrorAnnouncement = string.Empty;
    private long _groupStatusAnnouncementVersion;
    private long _groupErrorAnnouncementVersion;
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
        NextSetCommand = new AsyncRelayCommand(NextSetAsync, () => CanMoveToNextSet);
        PreviousSetCommand = new AsyncRelayCommand(PreviousSetAsync, () => CanMoveToPreviousSet);
        NextRootFacetPageCommand = new AsyncRelayCommand(NextRootFacetPageAsync, () => CanMoveRootFacetsNext);
        PreviousRootFacetPageCommand = new AsyncRelayCommand(PreviousRootFacetPageAsync, () => CanMoveRootFacetsPrevious);
        SortRootFacetsByCountCommand = new AsyncRelayCommand(
            () => ApplyRootFacetSortAsync(
                DuplicateFileSelectedRootFacetSortField.MatchingGroupCount,
                WorkerSortDirection.Descending));
        SortRootFacetsByNameCommand = new AsyncRelayCommand(
            () => ApplyRootFacetSortAsync(
                DuplicateFileSelectedRootFacetSortField.Value,
                WorkerSortDirection.Ascending));
        NextDriveFacetPageCommand = new AsyncRelayCommand(NextDriveFacetPageAsync, () => CanMoveDriveFacetsNext);
        PreviousDriveFacetPageCommand = new AsyncRelayCommand(PreviousDriveFacetPageAsync, () => CanMoveDriveFacetsPrevious);
        SortDriveFacetsByCountCommand = new AsyncRelayCommand(
            () => ApplyDriveFacetSortAsync(
                DuplicateFileDriveFacetSortField.MatchingGroupCount,
                WorkerSortDirection.Descending));
        SortDriveFacetsByNameCommand = new AsyncRelayCommand(
            () => ApplyDriveFacetSortAsync(
                DuplicateFileDriveFacetSortField.Value,
                WorkerSortDirection.Ascending));
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

    public IReadOnlyList<DuplicateFileSelectedRootFacetListItemViewModel> SelectedRootFacetOptions
    {
        get => _selectedRootFacetOptions;
        private set => SetProperty(ref _selectedRootFacetOptions, value);
    }

    public DuplicateFileSelectedRootFacetListItemViewModel? SelectedRootFacet
    {
        get => _selectedRootFacet;
        set
        {
            if (SetProperty(ref _selectedRootFacet, value))
            {
                OnPropertyChanged(nameof(SelectedRootFilterText));
            }
        }
    }

    public IReadOnlyList<DuplicateFileDriveFacetListItemViewModel> DriveFacetOptions
    {
        get => _driveFacetOptions;
        private set => SetProperty(ref _driveFacetOptions, value);
    }

    public DuplicateFileDriveFacetListItemViewModel? SelectedDriveFacet
    {
        get => _selectedDriveFacet;
        set
        {
            if (SetProperty(ref _selectedDriveFacet, value))
            {
                OnPropertyChanged(nameof(SelectedDriveFilterText));
            }
        }
    }

    public DuplicateFileGroupListItemViewModel? SelectedGroup
    {
        get => _selectedGroup;
        set
        {
            if (SetProperty(ref _selectedGroup, value))
            {
                OnPropertyChanged(nameof(HasSelectedGroup));
                RaiseSetNavigationProperties();
                _ = LoadSelectedGroupAsync(value);
            }
        }
    }

    public string SearchText
    {
        get => _searchText;
        set => SetProperty(ref _searchText, value);
    }

    public bool ExactPathMatch
    {
        get => _exactPathMatch;
        set => SetProperty(ref _exactPathMatch, value);
    }

    public string ExtensionText
    {
        get => _extensionText;
        set => SetProperty(ref _extensionText, value);
    }

    public bool WithoutExtension
    {
        get => _withoutExtension;
        set => SetProperty(ref _withoutExtension, value);
    }

    public bool AllMembersMustMatchExtension
    {
        get => _allMembersMustMatchExtension;
        set => SetProperty(ref _allMembersMustMatchExtension, value);
    }

    public string MinimumSizeText
    {
        get => _minimumSizeText;
        set => SetProperty(ref _minimumSizeText, value);
    }

    public bool OneGigabyteOrLarger
    {
        get => _oneGigabyteOrLarger;
        set => SetProperty(ref _oneGigabyteOrLarger, value);
    }

    public bool ThreeOrMoreCopies
    {
        get => _threeOrMoreCopies;
        set => SetProperty(ref _threeOrMoreCopies, value);
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
                RaiseMemberPagingProperties();
            }
        }
    }

    public bool IsRootFacetLoading
    {
        get => _isRootFacetLoading;
        private set
        {
            if (SetProperty(ref _isRootFacetLoading, value))
            {
                RaiseRootFacetPagingProperties();
            }
        }
    }

    public bool IsDriveFacetLoading
    {
        get => _isDriveFacetLoading;
        private set
        {
            if (SetProperty(ref _isDriveFacetLoading, value))
            {
                RaiseDriveFacetPagingProperties();
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

    public long TotalRootFacets
    {
        get => _totalRootFacets;
        private set
        {
            if (SetProperty(ref _totalRootFacets, value))
            {
                OnPropertyChanged(nameof(RootFacetCountText));
            }
        }
    }

    public long TotalDriveFacets
    {
        get => _totalDriveFacets;
        private set
        {
            if (SetProperty(ref _totalDriveFacets, value))
            {
                OnPropertyChanged(nameof(DriveFacetCountText));
            }
        }
    }

    public string? RootFacetErrorMessage
    {
        get => _rootFacetErrorMessage;
        private set
        {
            if (SetProperty(ref _rootFacetErrorMessage, value))
            {
                OnPropertyChanged(nameof(HasRootFacetError));
            }
        }
    }

    public string? DriveFacetErrorMessage
    {
        get => _driveFacetErrorMessage;
        private set
        {
            if (SetProperty(ref _driveFacetErrorMessage, value))
            {
                OnPropertyChanged(nameof(HasDriveFacetError));
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
                OnPropertyChanged(nameof(LocationCoverageText));
            }
        }
    }

    public DuplicateFileGroupSortField SortField => _sortField;

    public WorkerSortDirection SortDirection => _sortDirection;

    public string GroupCountText => $"{TotalGroups:N0} groups";

    public string MemberCountText => $"{TotalMembers:N0} copies";

    public string RootFacetCountText => TotalRootFacets == 1
        ? "1 selected root"
        : $"{TotalRootFacets:N0} selected roots";

    public string DriveFacetCountText => TotalDriveFacets == 1
        ? "1 drive"
        : $"{TotalDriveFacets:N0} drives";

    public string SelectedRootFilterText => SelectedRootFacet?.Value is { } value
        ? $"Filtering sets represented under {value}"
        : "All selected roots";

    public string SelectedDriveFilterText => SelectedDriveFacet?.Value is { } value
        ? $"Filtering sets represented on {value}"
        : "All drives";

    public string MatchingSetCountText => $"{Summary.MatchingGroupCount:N0}";

    public string MatchingCopyCountText => $"{Summary.MatchingCopyCount:N0}";

    public string PotentialRecoverableText => DisplayFormatting.Bytes(Summary.PotentialRecoverableBytes);

    public string LargestOpportunityText => DisplayFormatting.Bytes(Summary.LargestRecoverableBytes);

    public string LocationCoverageText
    {
        get
        {
            var roots = Summary.DistinctSelectedRootCount switch
            {
                0 => "Selected roots unavailable",
                1 => "1 selected root represented",
                _ => $"{Summary.DistinctSelectedRootCount:N0} selected roots represented",
            };
            var drives = Summary.DistinctDriveCount switch
            {
                0 => "no drive labels",
                1 => "1 drive represented",
                _ => $"{Summary.DistinctDriveCount:N0} drives represented",
            };
            var acrossDrives = Summary.AcrossDriveGroupCount switch
            {
                0 => "no sets span multiple drives",
                1 => "1 set spans multiple drives",
                _ => $"{Summary.AcrossDriveGroupCount:N0} sets span multiple drives",
            };
            return $"{roots} · {drives} · {acrossDrives}";
        }
    }

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

    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);

    public bool HasDetailError => !string.IsNullOrWhiteSpace(DetailErrorMessage);

    public bool HasRootFacetError => !string.IsNullOrWhiteSpace(RootFacetErrorMessage);

    public bool HasDriveFacetError => !string.IsNullOrWhiteSpace(DriveFacetErrorMessage);

    public bool IsUnavailable => Run is null || Run.Status != "completed";

    public bool IsEmpty => Run?.Status == "completed" && !IsLoading && !HasError && TotalGroups == 0;

    public bool HasGroups => Groups.Count > 0;

    public bool IsLoadingOverlayVisible => IsLoading && !HasGroups;

    public bool HasSelectedGroup => SelectedGroup is not null;

    public bool IsDetailEmpty => SelectedGroup is not null && !IsDetailLoading && !HasDetailError && TotalMembers == 0;

    public bool CanMoveNext => !IsLoading && _currentGroupPage?.NextCursor is not null;

    public bool CanMovePrevious => !IsLoading && _currentGroupPage?.PreviousCursor is not null;

    public bool CanMoveToNextSet =>
        !IsLoading
        && SelectedGroup is not null
        && (SelectedGroupIndex < Groups.Count - 1 || _currentGroupPage?.NextCursor is not null);

    public bool CanMoveToPreviousSet =>
        !IsLoading
        && SelectedGroup is not null
        && (SelectedGroupIndex > 0 || _currentGroupPage?.PreviousCursor is not null);

    public bool CanMoveMembersNext => !IsDetailLoading && _currentMemberPage?.NextCursor is not null;

    public bool CanMoveMembersPrevious => !IsDetailLoading && _currentMemberPage?.PreviousCursor is not null;

    public bool CanMoveRootFacetsNext => !IsRootFacetLoading && _currentRootFacetPage?.NextCursor is not null;

    public bool CanMoveRootFacetsPrevious => !IsRootFacetLoading && _currentRootFacetPage?.PreviousCursor is not null;

    public bool CanMoveDriveFacetsNext => !IsDriveFacetLoading && _currentDriveFacetPage?.NextCursor is not null;

    public bool CanMoveDriveFacetsPrevious => !IsDriveFacetLoading && _currentDriveFacetPage?.PreviousCursor is not null;

    public int CachedGroupPageCount => _groupCache.Count;

    public int CachedMemberPageCount => _memberCache.Count;

    public int CachedRootFacetPageCount => _rootFacetCache.Count;

    public int CachedDriveFacetPageCount => _driveFacetCache.Count;

    public IAsyncRelayCommand ApplyFiltersCommand { get; }

    public IAsyncRelayCommand ClearFiltersCommand { get; }

    public IAsyncRelayCommand NextPageCommand { get; }

    public IAsyncRelayCommand PreviousPageCommand { get; }

    public IAsyncRelayCommand NextSetCommand { get; }

    public IAsyncRelayCommand PreviousSetCommand { get; }

    public IAsyncRelayCommand NextRootFacetPageCommand { get; }

    public IAsyncRelayCommand PreviousRootFacetPageCommand { get; }

    public IAsyncRelayCommand SortRootFacetsByCountCommand { get; }

    public IAsyncRelayCommand SortRootFacetsByNameCommand { get; }

    public IAsyncRelayCommand NextDriveFacetPageCommand { get; }

    public IAsyncRelayCommand PreviousDriveFacetPageCommand { get; }

    public IAsyncRelayCommand SortDriveFacetsByCountCommand { get; }

    public IAsyncRelayCommand SortDriveFacetsByNameCommand { get; }

    public IAsyncRelayCommand NextMemberPageCommand { get; }

    public IAsyncRelayCommand PreviousMemberPageCommand { get; }

    public IRelayCommand<DuplicateFileMemberListItemViewModel> CopyPathCommand { get; }

    public IAsyncRelayCommand<DuplicateFileMemberListItemViewModel> RevealInExplorerCommand { get; }

    public async Task ShowRunAsync(WorkerRun? run, CancellationToken cancellationToken = default)
    {
        Run = run;
        CancelGroupQuery();
        CancelMemberQuery();
        CancelRootFacetQuery();
        CancelDriveFacetQuery();
        _groupCache.Clear();
        _memberCache.Clear();
        _rootFacetCache.Clear();
        _driveFacetCache.Clear();
        _currentGroupPage = null;
        _currentMemberPage = null;
        _currentRootFacetPage = null;
        _currentDriveFacetPage = null;
        Groups = [];
        Members = [];
        SelectedRootFacetOptions = [new()];
        SelectedRootFacet = SelectedRootFacetOptions[0];
        DriveFacetOptions = [new()];
        SelectedDriveFacet = DriveFacetOptions[0];
        SelectedGroup = null;
        TotalGroups = 0;
        TotalMembers = 0;
        TotalRootFacets = 0;
        TotalDriveFacets = 0;
        Summary = new WorkerDuplicateFileReviewSummary(0, 0, "0", "0");
        ErrorMessage = null;
        DetailErrorMessage = null;
        RootFacetErrorMessage = null;
        DriveFacetErrorMessage = null;
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
        CancelRootFacetQuery();
        CancelDriveFacetQuery();
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
        ExactPathMatch = false;
        ExtensionText = string.Empty;
        WithoutExtension = false;
        AllMembersMustMatchExtension = false;
        MinimumSizeText = string.Empty;
        OneGigabyteOrLarger = false;
        ThreeOrMoreCopies = false;
        AcrossDrives = false;
        SelectedRootFacet = SelectedRootFacetOptions.FirstOrDefault(option => option.Value is null)
            ?? new DuplicateFileSelectedRootFacetListItemViewModel();
        SelectedDriveFacet = DriveFacetOptions.FirstOrDefault(option => option.Value is null)
            ?? new DuplicateFileDriveFacetListItemViewModel();
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
            PublishGroupErrorAnnouncement("Duplicate file filters could not be applied.");
            return;
        }
        CancelGroupQuery();
        CancelMemberQuery();
        CancelRootFacetQuery();
        CancelDriveFacetQuery();
        _groupCancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var generation = ++_groupGeneration;
        IsLoading = true;
        _groupCache.Clear();
        _memberCache.Clear();
        _rootFacetCache.Clear();
        _driveFacetCache.Clear();
        _currentGroupPage = null;
        _currentMemberPage = null;
        _currentRootFacetPage = null;
        _currentDriveFacetPage = null;
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
        RootFacetErrorMessage = null;
        DriveFacetErrorMessage = null;
        await LoadGroupPageAsync(null, filter, generation, _groupCancellation.Token, display: true);
        if (generation == _groupGeneration && !cancellationToken.IsCancellationRequested)
        {
            await Task.WhenAll(
                ResetAndLoadRootFacetsAsync(filter, cancellationToken),
                ResetAndLoadDriveFacetsAsync(filter, cancellationToken));
        }
    }

    private async Task LoadGroupPageAsync(
        string? cursor,
        DuplicateFileGroupFilter filter,
        long generation,
        CancellationToken cancellationToken,
        bool display,
        bool selectLast = false)
    {
        if (_groupCache.TryGet(cursor, out var cached))
        {
            if (display && generation == _groupGeneration)
            {
                DisplayGroupPage(cached, selectLast);
                PublishGroupQueryAnnouncement();
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
                DisplayGroupPage(page, selectLast);
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

    private void DisplayGroupPage(WorkerDuplicateFileGroupPage page, bool selectLast)
    {
        _currentGroupPage = page;
        TotalGroups = page.Total;
        Summary = page.Summary;
        Groups = page.Groups.Select(group => new DuplicateFileGroupListItemViewModel(group)).ToArray();
        OnPropertyChanged(nameof(HasGroups));
        OnPropertyChanged(nameof(IsEmpty));
        SelectedGroup = selectLast ? Groups.LastOrDefault() : Groups.FirstOrDefault();
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

    private async Task NextSetAsync()
    {
        var selectedIndex = SelectedGroupIndex;
        if (selectedIndex < 0)
        {
            return;
        }
        if (selectedIndex < Groups.Count - 1)
        {
            SelectedGroup = Groups[selectedIndex + 1];
            return;
        }
        if (_currentGroupPage?.NextCursor is { } cursor
            && TryBuildFilter(out var filter)
            && _groupCancellation is not null)
        {
            await LoadGroupPageAsync(
                cursor,
                filter,
                _groupGeneration,
                _groupCancellation.Token,
                display: true);
        }
    }

    private async Task PreviousSetAsync()
    {
        var selectedIndex = SelectedGroupIndex;
        if (selectedIndex < 0)
        {
            return;
        }
        if (selectedIndex > 0)
        {
            SelectedGroup = Groups[selectedIndex - 1];
            return;
        }
        if (_currentGroupPage?.PreviousCursor is { } cursor
            && TryBuildFilter(out var filter)
            && _groupCancellation is not null)
        {
            await LoadGroupPageAsync(
                cursor,
                filter,
                _groupGeneration,
                _groupCancellation.Token,
                display: true,
                selectLast: true);
        }
    }

    private async Task ApplyRootFacetSortAsync(
        DuplicateFileSelectedRootFacetSortField field,
        WorkerSortDirection direction)
    {
        if ((_rootFacetSortField == field && _rootFacetSortDirection == direction)
            || !TryBuildFilter(out var filter)
            || Run?.Status != "completed")
        {
            return;
        }
        _rootFacetSortField = field;
        _rootFacetSortDirection = direction;
        await ResetAndLoadRootFacetsAsync(filter);
    }

    private async Task ResetAndLoadRootFacetsAsync(
        DuplicateFileGroupFilter groupFilter,
        CancellationToken cancellationToken = default)
    {
        CancelRootFacetQuery();
        _rootFacetCancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var generation = ++_rootFacetGeneration;
        _rootFacetCache.Clear();
        _currentRootFacetPage = null;
        RootFacetErrorMessage = null;
        IsRootFacetLoading = true;
        var filter = new DuplicateFileSelectedRootFacetFilter(
            groupFilter.Search,
            groupFilter.MinimumSize,
            groupFilter.AcrossDrives,
            groupFilter.SelectedDrive,
            groupFilter.MinimumCopyCount,
            groupFilter.PathMatch,
            groupFilter.Extension,
            groupFilter.ExtensionMatch);
        await LoadRootFacetPageAsync(
            null,
            filter,
            generation,
            _rootFacetCancellation.Token,
            display: true);
    }

    private async Task LoadRootFacetPageAsync(
        string? cursor,
        DuplicateFileSelectedRootFacetFilter filter,
        long generation,
        CancellationToken cancellationToken,
        bool display)
    {
        if (_rootFacetCache.TryGet(cursor, out var cached))
        {
            if (display && generation == _rootFacetGeneration)
            {
                DisplayRootFacetPage(cached);
                _ = PrefetchRootFacetNeighborsAsync(cached, filter, generation, cancellationToken);
            }
            return;
        }
        if (Run is not { Status: "completed" } run)
        {
            return;
        }
        if (display)
        {
            IsRootFacetLoading = true;
            RootFacetErrorMessage = null;
        }
        try
        {
            var page = await _workerClient.GetDuplicateFileSelectedRootFacetsAsync(
                new DuplicateFileSelectedRootFacetQuery(
                    run.Id,
                    RootFacetPageSize,
                    _rootFacetSortField,
                    _rootFacetSortDirection,
                    filter,
                    cursor),
                cancellationToken);
            if (generation != _rootFacetGeneration || cancellationToken.IsCancellationRequested)
            {
                return;
            }
            _rootFacetCache.Set(cursor, page);
            if (display)
            {
                DisplayRootFacetPage(page);
                _ = PrefetchRootFacetNeighborsAsync(page, filter, generation, cancellationToken);
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (display && generation == _rootFacetGeneration)
            {
                RootFacetErrorMessage = exception.Message;
            }
        }
        finally
        {
            if (display && generation == _rootFacetGeneration)
            {
                IsRootFacetLoading = false;
            }
        }
    }

    private void DisplayRootFacetPage(WorkerDuplicateFileSelectedRootFacetPage page)
    {
        _currentRootFacetPage = page;
        TotalRootFacets = page.Total;
        var selectedValue = SelectedRootFacet?.Value;
        var options = new List<DuplicateFileSelectedRootFacetListItemViewModel>
        {
            new(),
        };
        options.AddRange(page.Facets.Select(facet =>
            new DuplicateFileSelectedRootFacetListItemViewModel(facet)));
        if (selectedValue is not null
            && options.All(option => !string.Equals(
                option.Value,
                selectedValue,
                StringComparison.OrdinalIgnoreCase)))
        {
            options.Add(new DuplicateFileSelectedRootFacetListItemViewModel(selectedValue: selectedValue));
        }
        SelectedRootFacetOptions = options;
        SelectedRootFacet = options.First(option => string.Equals(
            option.Value,
            selectedValue,
            StringComparison.OrdinalIgnoreCase));
        RaiseRootFacetPagingProperties();
    }

    private async Task PrefetchRootFacetNeighborsAsync(
        WorkerDuplicateFileSelectedRootFacetPage page,
        DuplicateFileSelectedRootFacetFilter filter,
        long generation,
        CancellationToken cancellationToken)
    {
        await PrefetchRootFacetDirectionAsync(
            page.PreviousCursor,
            false,
            2,
            filter,
            generation,
            cancellationToken);
        await PrefetchRootFacetDirectionAsync(
            page.NextCursor,
            true,
            2,
            filter,
            generation,
            cancellationToken);
    }

    private async Task PrefetchRootFacetDirectionAsync(
        string? cursor,
        bool forward,
        int remaining,
        DuplicateFileSelectedRootFacetFilter filter,
        long generation,
        CancellationToken cancellationToken)
    {
        if (cursor is null
            || remaining == 0
            || generation != _rootFacetGeneration
            || cancellationToken.IsCancellationRequested)
        {
            return;
        }
        if (!_rootFacetCache.TryGet(cursor, out var page))
        {
            await LoadRootFacetPageAsync(cursor, filter, generation, cancellationToken, display: false);
            if (!_rootFacetCache.TryGet(cursor, out page))
            {
                return;
            }
        }
        await PrefetchRootFacetDirectionAsync(
            forward ? page.NextCursor : page.PreviousCursor,
            forward,
            remaining - 1,
            filter,
            generation,
            cancellationToken);
    }

    private async Task NextRootFacetPageAsync()
    {
        if (_currentRootFacetPage?.NextCursor is { } cursor
            && TryBuildFilter(out var groupFilter)
            && _rootFacetCancellation is not null)
        {
            var filter = new DuplicateFileSelectedRootFacetFilter(
                groupFilter.Search,
                groupFilter.MinimumSize,
                groupFilter.AcrossDrives,
                groupFilter.SelectedDrive,
                groupFilter.MinimumCopyCount,
                groupFilter.PathMatch,
                groupFilter.Extension,
                groupFilter.ExtensionMatch);
            await LoadRootFacetPageAsync(
                cursor,
                filter,
                _rootFacetGeneration,
                _rootFacetCancellation.Token,
                display: true);
        }
    }

    private async Task PreviousRootFacetPageAsync()
    {
        if (_currentRootFacetPage?.PreviousCursor is { } cursor
            && TryBuildFilter(out var groupFilter)
            && _rootFacetCancellation is not null)
        {
            var filter = new DuplicateFileSelectedRootFacetFilter(
                groupFilter.Search,
                groupFilter.MinimumSize,
                groupFilter.AcrossDrives,
                groupFilter.SelectedDrive,
                groupFilter.MinimumCopyCount,
                groupFilter.PathMatch,
                groupFilter.Extension,
                groupFilter.ExtensionMatch);
            await LoadRootFacetPageAsync(
                cursor,
                filter,
                _rootFacetGeneration,
                _rootFacetCancellation.Token,
                display: true);
        }
    }

    private async Task ApplyDriveFacetSortAsync(
        DuplicateFileDriveFacetSortField field,
        WorkerSortDirection direction)
    {
        if ((_driveFacetSortField == field && _driveFacetSortDirection == direction)
            || !TryBuildFilter(out var filter)
            || Run?.Status != "completed")
        {
            return;
        }
        _driveFacetSortField = field;
        _driveFacetSortDirection = direction;
        await ResetAndLoadDriveFacetsAsync(filter);
    }

    private async Task ResetAndLoadDriveFacetsAsync(
        DuplicateFileGroupFilter groupFilter,
        CancellationToken cancellationToken = default)
    {
        CancelDriveFacetQuery();
        _driveFacetCancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var generation = ++_driveFacetGeneration;
        _driveFacetCache.Clear();
        _currentDriveFacetPage = null;
        DriveFacetErrorMessage = null;
        IsDriveFacetLoading = true;
        var filter = new DuplicateFileDriveFacetFilter(
            groupFilter.Search,
            groupFilter.MinimumSize,
            groupFilter.AcrossDrives,
            groupFilter.SelectedRoot,
            groupFilter.MinimumCopyCount,
            groupFilter.PathMatch,
            groupFilter.Extension,
            groupFilter.ExtensionMatch);
        await LoadDriveFacetPageAsync(
            null,
            filter,
            generation,
            _driveFacetCancellation.Token,
            display: true);
    }

    private async Task LoadDriveFacetPageAsync(
        string? cursor,
        DuplicateFileDriveFacetFilter filter,
        long generation,
        CancellationToken cancellationToken,
        bool display)
    {
        if (_driveFacetCache.TryGet(cursor, out var cached))
        {
            if (display && generation == _driveFacetGeneration)
            {
                DisplayDriveFacetPage(cached);
                _ = PrefetchDriveFacetNeighborsAsync(cached, filter, generation, cancellationToken);
            }
            return;
        }
        if (Run is not { Status: "completed" } run)
        {
            return;
        }
        if (display)
        {
            IsDriveFacetLoading = true;
            DriveFacetErrorMessage = null;
        }
        try
        {
            var page = await _workerClient.GetDuplicateFileDriveFacetsAsync(
                new DuplicateFileDriveFacetQuery(
                    run.Id,
                    DriveFacetPageSize,
                    _driveFacetSortField,
                    _driveFacetSortDirection,
                    filter,
                    cursor),
                cancellationToken);
            if (generation != _driveFacetGeneration || cancellationToken.IsCancellationRequested)
            {
                return;
            }
            _driveFacetCache.Set(cursor, page);
            if (display)
            {
                DisplayDriveFacetPage(page);
                _ = PrefetchDriveFacetNeighborsAsync(page, filter, generation, cancellationToken);
            }
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (display && generation == _driveFacetGeneration)
            {
                DriveFacetErrorMessage = exception.Message;
            }
        }
        finally
        {
            if (display && generation == _driveFacetGeneration)
            {
                IsDriveFacetLoading = false;
            }
        }
    }

    private void DisplayDriveFacetPage(WorkerDuplicateFileDriveFacetPage page)
    {
        _currentDriveFacetPage = page;
        TotalDriveFacets = page.Total;
        var selectedValue = SelectedDriveFacet?.Value;
        var options = new List<DuplicateFileDriveFacetListItemViewModel>
        {
            new(),
        };
        options.AddRange(page.Facets.Select(facet => new DuplicateFileDriveFacetListItemViewModel(facet)));
        if (selectedValue is not null
            && options.All(option => !string.Equals(
                option.Value,
                selectedValue,
                StringComparison.OrdinalIgnoreCase)))
        {
            options.Add(new DuplicateFileDriveFacetListItemViewModel(selectedValue: selectedValue));
        }
        DriveFacetOptions = options;
        SelectedDriveFacet = options.First(option => string.Equals(
            option.Value,
            selectedValue,
            StringComparison.OrdinalIgnoreCase));
        RaiseDriveFacetPagingProperties();
    }

    private async Task PrefetchDriveFacetNeighborsAsync(
        WorkerDuplicateFileDriveFacetPage page,
        DuplicateFileDriveFacetFilter filter,
        long generation,
        CancellationToken cancellationToken)
    {
        await PrefetchDriveFacetDirectionAsync(
            page.PreviousCursor,
            false,
            2,
            filter,
            generation,
            cancellationToken);
        await PrefetchDriveFacetDirectionAsync(
            page.NextCursor,
            true,
            2,
            filter,
            generation,
            cancellationToken);
    }

    private async Task PrefetchDriveFacetDirectionAsync(
        string? cursor,
        bool forward,
        int remaining,
        DuplicateFileDriveFacetFilter filter,
        long generation,
        CancellationToken cancellationToken)
    {
        if (cursor is null
            || remaining == 0
            || generation != _driveFacetGeneration
            || cancellationToken.IsCancellationRequested)
        {
            return;
        }
        if (!_driveFacetCache.TryGet(cursor, out var page))
        {
            await LoadDriveFacetPageAsync(cursor, filter, generation, cancellationToken, display: false);
            if (!_driveFacetCache.TryGet(cursor, out page))
            {
                return;
            }
        }
        await PrefetchDriveFacetDirectionAsync(
            forward ? page.NextCursor : page.PreviousCursor,
            forward,
            remaining - 1,
            filter,
            generation,
            cancellationToken);
    }

    private async Task NextDriveFacetPageAsync()
    {
        if (_currentDriveFacetPage?.NextCursor is { } cursor
            && TryBuildFilter(out var groupFilter)
            && _driveFacetCancellation is not null)
        {
            var filter = new DuplicateFileDriveFacetFilter(
                groupFilter.Search,
                groupFilter.MinimumSize,
                groupFilter.AcrossDrives,
                groupFilter.SelectedRoot,
                groupFilter.MinimumCopyCount,
                groupFilter.PathMatch,
                groupFilter.Extension,
                groupFilter.ExtensionMatch);
            await LoadDriveFacetPageAsync(
                cursor,
                filter,
                _driveFacetGeneration,
                _driveFacetCancellation.Token,
                display: true);
        }
    }

    private async Task PreviousDriveFacetPageAsync()
    {
        if (_currentDriveFacetPage?.PreviousCursor is { } cursor
            && TryBuildFilter(out var groupFilter)
            && _driveFacetCancellation is not null)
        {
            var filter = new DuplicateFileDriveFacetFilter(
                groupFilter.Search,
                groupFilter.MinimumSize,
                groupFilter.AcrossDrives,
                groupFilter.SelectedRoot,
                groupFilter.MinimumCopyCount,
                groupFilter.PathMatch,
                groupFilter.Extension,
                groupFilter.ExtensionMatch);
            await LoadDriveFacetPageAsync(
                cursor,
                filter,
                _driveFacetGeneration,
                _driveFacetCancellation.Token,
                display: true);
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
        var search = ExactPathMatch ? SearchText : SearchText.Trim();
        if (ExactPathMatch && string.IsNullOrWhiteSpace(search))
        {
            search = string.Empty;
        }
        var maximumSearchCharacters = ExactPathMatch
            ? MaximumExactPathCharacters
            : MaximumSubstringSearchCharacters;
        if (search.EnumerateRunes().Count() > maximumSearchCharacters)
        {
            ErrorMessage = ExactPathMatch
                ? $"Exact member path may contain at most {MaximumExactPathCharacters:N0} characters."
                : $"Path search may contain at most {MaximumSubstringSearchCharacters:N0} characters.";
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
        if (OneGigabyteOrLarger)
        {
            value = Math.Max(value, OneGigabyteBytes);
        }
        string? extension = null;
        if (WithoutExtension)
        {
            extension = string.Empty;
        }
        else if (ExtensionText.Length > 0)
        {
            if (ExtensionText.EnumerateRunes().Count() > MaximumExtensionCharacters
                || ExtensionText.Contains('.', StringComparison.Ordinal)
                || ExtensionText.Contains('/', StringComparison.Ordinal)
                || ExtensionText.Contains('\\', StringComparison.Ordinal))
            {
                ErrorMessage = $"Extension must contain at most {MaximumExtensionCharacters:N0} characters without a dot or path separator.";
                filter = new DuplicateFileGroupFilter(string.Empty, "0", false);
                return false;
            }
            extension = ExtensionText;
        }
        ErrorMessage = null;
        filter = new DuplicateFileGroupFilter(
            search,
            value.ToString(CultureInfo.InvariantCulture),
            AcrossDrives,
            SelectedRootFacet?.Value,
            SelectedDriveFacet?.Value,
            ThreeOrMoreCopies ? 3 : 2,
            ExactPathMatch ? DuplicateFilePathMatchMode.Exact : DuplicateFilePathMatchMode.Substring,
            extension,
            extension is not null && AllMembersMustMatchExtension
                ? DuplicateFileExtensionMatchMode.AllMembers
                : DuplicateFileExtensionMatchMode.AnyMember);
        return true;
    }

    private void PublishGroupQueryAnnouncement()
    {
        if (HasError)
        {
            PublishGroupErrorAnnouncement("Duplicate file results could not be loaded.");
            return;
        }

        if (Run?.Status != "completed")
        {
            return;
        }

        GroupStatusAnnouncement = TotalGroups == 0
            ? "Duplicate file query complete. No matching duplicate sets."
            : $"Duplicate file query complete. {FormatCount(TotalGroups, "matching set", "matching sets")}, "
                + $"{FormatCount(Summary.MatchingCopyCount, "copy", "copies")}, "
                + $"{PotentialRecoverableText} potentially recoverable. {LocationCoverageText}.";
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

    private static string FormatCount(long value, string singular, string plural) =>
        value == 1 ? $"1 {singular}" : $"{value:N0} {plural}";

    private int SelectedGroupIndex
    {
        get
        {
            if (SelectedGroup is null)
            {
                return -1;
            }
            for (var index = 0; index < Groups.Count; index++)
            {
                if (Groups[index].Id == SelectedGroup.Id)
                {
                    return index;
                }
            }
            return -1;
        }
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

    private void CancelRootFacetQuery()
    {
        _rootFacetCancellation?.Cancel();
        _rootFacetCancellation?.Dispose();
        _rootFacetCancellation = null;
        _rootFacetGeneration++;
    }

    private void CancelDriveFacetQuery()
    {
        _driveFacetCancellation?.Cancel();
        _driveFacetCancellation?.Dispose();
        _driveFacetCancellation = null;
        _driveFacetGeneration++;
    }

    private void RaisePagingProperties()
    {
        OnPropertyChanged(nameof(CanMoveNext));
        OnPropertyChanged(nameof(CanMovePrevious));
        NextPageCommand.NotifyCanExecuteChanged();
        PreviousPageCommand.NotifyCanExecuteChanged();
        RaiseSetNavigationProperties();
    }

    private void RaiseSetNavigationProperties()
    {
        OnPropertyChanged(nameof(CanMoveToNextSet));
        OnPropertyChanged(nameof(CanMoveToPreviousSet));
        NextSetCommand.NotifyCanExecuteChanged();
        PreviousSetCommand.NotifyCanExecuteChanged();
    }

    private void RaiseMemberPagingProperties()
    {
        OnPropertyChanged(nameof(CanMoveMembersNext));
        OnPropertyChanged(nameof(CanMoveMembersPrevious));
        NextMemberPageCommand.NotifyCanExecuteChanged();
        PreviousMemberPageCommand.NotifyCanExecuteChanged();
    }

    private void RaiseRootFacetPagingProperties()
    {
        OnPropertyChanged(nameof(CanMoveRootFacetsNext));
        OnPropertyChanged(nameof(CanMoveRootFacetsPrevious));
        NextRootFacetPageCommand.NotifyCanExecuteChanged();
        PreviousRootFacetPageCommand.NotifyCanExecuteChanged();
    }

    private void RaiseDriveFacetPagingProperties()
    {
        OnPropertyChanged(nameof(CanMoveDriveFacetsNext));
        OnPropertyChanged(nameof(CanMoveDriveFacetsPrevious));
        NextDriveFacetPageCommand.NotifyCanExecuteChanged();
        PreviousDriveFacetPageCommand.NotifyCanExecuteChanged();
    }
}
