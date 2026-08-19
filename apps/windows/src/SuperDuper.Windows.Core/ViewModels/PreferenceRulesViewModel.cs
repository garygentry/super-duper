using System.Collections.ObjectModel;
using System.Text;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed record PreferencePreviewScopeOption(
    PreferencePreviewScopeKind Kind,
    string DisplayName,
    string HelpText);

public sealed class PreferencePreviewGroupListItemViewModel
{
    public PreferencePreviewGroupListItemViewModel(WorkerPreferencePreviewGroup group)
    {
        Group = group;
    }

    public WorkerPreferencePreviewGroup Group { get; }

    public long GroupId => Group.GroupId;

    public string Status => Group.Status == "blocked" ? "Blocked" : "Applicable";

    public string PreferredRoot => Group.PreferredRoot ?? "No ranked root";

    public string ProposedKeepText => $"{Group.ProposedKeepPathCount:N0} keep";

    public string ProposedRemoveText =>
        $"{Group.ProposedRemovePathCount:N0} remove, {DisplayFormatting.Bytes(Group.ProposedRemoveBytes)} physical bytes";

    public string Explanation => Group.ExplanationCode switch
    {
        "highest_rank_tie" => $"All {Group.TiedPreferredPathCount:N0} paths on {PreferredRoot} share the highest-ranked root.",
        "manual_keep_precedence" => $"{PreferredRoot} is highest ranked; {Group.ManualKeepCount:N0} manual Keep decision remains protected.",
        "manual_folder_keep_conflict" => "Blocked because a manual folder Keep protects a contained path. Clear that folder decision before applying a future rule.",
        "file_survivor_conflict" => "Blocked because the virtual result would leave no independently accessible physical file survivor.",
        "folder_survivor_conflict" => "Blocked because the virtual result would leave no intact exact-folder copy.",
        _ => $"{PreferredRoot} has the highest configured rank; lower-ranked and unranked eligible paths would be removed.",
    };

    public string AutomationName =>
        $"Duplicate set {GroupId}. {Status}. {Explanation} {ProposedKeepText}; {ProposedRemoveText}. Nothing is applied or deleted.";
}

public sealed class PreferenceRulesViewModel : ObservableObject, IDisposable
{
    public const int PreviewPageSize = 100;
    public const int CacheCapacity = 5;
    private readonly IWorkerClient _workerClient;
    private readonly Func<DuplicateFileGroupFilter?> _currentFilter;
    private readonly Func<long?> _selectedGroupId;
    private readonly Func<long> _reviewRevision;
    private readonly BoundedCursorCache<WorkerPreferencePreviewPage> _previewCache = new(CacheCapacity);
    private CancellationTokenSource? _cancellation;
    private WorkerRun? _run;
    private IReadOnlyList<WorkerPreferenceRuleSummary> _rules = [];
    private WorkerPreferenceRuleSummary? _selectedRule;
    private long? _ruleId;
    private long _ruleRevision;
    private string _ruleName = string.Empty;
    private string _newRoot = string.Empty;
    private string? _selectedRoot;
    private PreferencePreviewScopeOption _selectedScope;
    private IReadOnlyList<PreferencePreviewGroupListItemViewModel> _previewGroups = [];
    private WorkerPreferencePreviewPage? _currentPage;
    private WorkerPreferencePreviewSummary _summary = EmptySummary();
    private bool _isBusy;
    private string? _errorMessage;
    private string _statusMessage = "Save or select a named rule to preview. Nothing will be applied or deleted.";
    private long _generation;
    private long _announcementVersion;
    private bool _isRuleDirty = true;
    private bool _disposed;

    public PreferenceRulesViewModel(
        IWorkerClient workerClient,
        Func<DuplicateFileGroupFilter?> currentFilter,
        Func<long?> selectedGroupId,
        Func<long> reviewRevision)
    {
        _workerClient = workerClient;
        _currentFilter = currentFilter;
        _selectedGroupId = selectedGroupId;
        _reviewRevision = reviewRevision;
        ScopeOptions =
        [
            new(PreferencePreviewScopeKind.CurrentFilter, "Current complete filter", "Every set matching the complete server-owned duplicate-file filter."),
            new(PreferencePreviewScopeKind.SelectedSets, "Selected set", "Only the currently selected duplicate set."),
            new(PreferencePreviewScopeKind.CompletedRun, "Completed run", "Every duplicate-file set in the immutable completed run."),
        ];
        _selectedScope = ScopeOptions[0];
        SaveCommand = new AsyncRelayCommand(SaveAsync, CanSave);
        PreviewCommand = new AsyncRelayCommand(() => LoadPreviewAsync(null), CanPreview);
        NextPageCommand = new AsyncRelayCommand(NextPageAsync, () => CanMoveNext);
        MoveRootUpCommand = new RelayCommand(MoveRootUp, CanMoveRootUp);
        MoveRootDownCommand = new RelayCommand(MoveRootDown, CanMoveRootDown);
        AddRootCommand = new RelayCommand(AddRoot, CanAddRoot);
        RemoveRootCommand = new RelayCommand(RemoveRoot, () => SelectedRoot is not null && !IsBusy);
        NewRuleCommand = new RelayCommand(NewRule, () => !IsBusy);
    }

    public IReadOnlyList<PreferencePreviewScopeOption> ScopeOptions { get; }

    public ObservableCollection<string> OrderedRoots { get; } = [];

    public IReadOnlyList<WorkerPreferenceRuleSummary> Rules
    {
        get => _rules;
        private set => SetProperty(ref _rules, value);
    }

    public WorkerPreferenceRuleSummary? SelectedRule
    {
        get => _selectedRule;
        set
        {
            if (SetProperty(ref _selectedRule, value) && value is not null)
            {
                _ = LoadRuleAsync(value.Id);
            }
        }
    }

    public string RuleName
    {
        get => _ruleName;
        set
        {
            if (SetProperty(ref _ruleName, value))
            {
                if (!IsBusy)
                {
                    _isRuleDirty = true;
                    InvalidatePreview("Rule name changed. Save it before running a new preview.");
                }
                RaiseCommandState();
            }
        }
    }

    public string NewRoot
    {
        get => _newRoot;
        set
        {
            if (SetProperty(ref _newRoot, value))
            {
                AddRootCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public string? SelectedRoot
    {
        get => _selectedRoot;
        set
        {
            if (SetProperty(ref _selectedRoot, value))
            {
                RaiseRootCommandState();
            }
        }
    }

    public PreferencePreviewScopeOption SelectedScope
    {
        get => _selectedScope;
        set
        {
            if (SetProperty(ref _selectedScope, value))
            {
                InvalidatePreview("Scope changed. Run Preview to recompute the virtual result.");
            }
        }
    }

    public IReadOnlyList<PreferencePreviewGroupListItemViewModel> PreviewGroups
    {
        get => _previewGroups;
        private set => SetProperty(ref _previewGroups, value);
    }

    public bool IsBusy
    {
        get => _isBusy;
        private set
        {
            if (SetProperty(ref _isBusy, value))
            {
                RaiseCommandState();
                OnPropertyChanged(nameof(IsNotBusy));
            }
        }
    }

    public bool IsNotBusy => !IsBusy;

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

    public bool HasError => ErrorMessage is not null;

    public string StatusMessage
    {
        get => _statusMessage;
        private set => SetProperty(ref _statusMessage, value);
    }

    public long AnnouncementVersion
    {
        get => _announcementVersion;
        private set => SetProperty(ref _announcementVersion, value);
    }

    public string SummaryText =>
        $"{_summary.AffectedGroupCount:N0} applicable sets; {_summary.BlockedGroupCount:N0} blocked; "
        + $"{_summary.ProposedRemovePathCount:N0} logical removals; {_summary.ProposedRemovePhysicalItemCount:N0} physical items; "
        + $"{DisplayFormatting.Bytes(_summary.ProposedRemoveBytes)}. {_summary.TiedGroupCount:N0} ties; "
        + $"{_summary.ManualKeepPathCount:N0} manual Keeps; {_summary.MissingRuleRootCount:N0} configured roots absent from this scope.";

    public bool CanMoveNext => !IsBusy && _currentPage?.NextCursor is not null;

    public int CachedPageCount => _previewCache.Count;

    public IAsyncRelayCommand SaveCommand { get; }

    public IAsyncRelayCommand PreviewCommand { get; }

    public IAsyncRelayCommand NextPageCommand { get; }

    public IRelayCommand MoveRootUpCommand { get; }

    public IRelayCommand MoveRootDownCommand { get; }

    public IRelayCommand AddRootCommand { get; }

    public IRelayCommand RemoveRootCommand { get; }

    public IRelayCommand NewRuleCommand { get; }

    public async Task ShowRunAsync(WorkerRun? run, CancellationToken cancellationToken = default)
    {
        _run = run;
        Cancel();
        Rules = [];
        SelectedRule = null;
        _ruleId = null;
        _ruleRevision = 0;
        RuleName = string.Empty;
        OrderedRoots.Clear();
        PreviewGroups = [];
        _currentPage = null;
        _summary = EmptySummary();
        OnPropertyChanged(nameof(SummaryText));
        if (run?.Status != "completed")
        {
            StatusMessage = "Preference preview is available for completed runs only.";
            return;
        }
        foreach (var root in run.Parameters.Roots.Take(64))
        {
            OrderedRoots.Add(root);
        }
        StatusMessage = "Loading saved preference rules. Nothing will be applied or deleted.";
        _cancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var generation = ++_generation;
        IsBusy = true;
        try
        {
            var page = await _workerClient.ListPreferenceRulesAsync(0, 200, _cancellation.Token);
            if (generation != _generation || _run?.Id != run.Id)
            {
                return;
            }
            Rules = page.Rules;
            if (Rules.Count > 0)
            {
                SelectedRule = Rules[0];
            }
            else
            {
                RuleName = "Preferred scan roots";
                StatusMessage = "Name and save this ordered root list before previewing. Nothing will be applied or deleted.";
            }
        }
        catch (OperationCanceledException) when (_cancellation?.IsCancellationRequested == true)
        {
        }
        catch (Exception exception)
        {
            if (generation == _generation)
            {
                PublishError($"Preference rules could not be loaded. {exception.Message}");
            }
        }
        finally
        {
            if (generation == _generation)
            {
                IsBusy = false;
            }
        }
    }

    public void InvalidateReviewRevision(long revision)
    {
        if (_run?.Status == "completed" && revision >= 0)
        {
            Cancel(preserveRuleSelection: true);
            InvalidatePreview("Manual review decisions changed. Run Preview again to use the current review revision.");
        }
    }

    public void InvalidateFilter()
    {
        Cancel(preserveRuleSelection: true);
        InvalidatePreview("The duplicate-file filter changed. Run Preview again for the complete current filter.");
    }

    private async Task LoadRuleAsync(long ruleId)
    {
        if (_run?.Status != "completed")
        {
            return;
        }
        Cancel(preserveRuleSelection: true);
        _cancellation = new CancellationTokenSource();
        var generation = ++_generation;
        IsBusy = true;
        try
        {
            var rule = await _workerClient.GetPreferenceRuleAsync(ruleId, _cancellation.Token);
            if (generation != _generation || SelectedRule?.Id != ruleId)
            {
                return;
            }
            _ruleId = rule.Id;
            _ruleRevision = rule.Revision;
            RuleName = rule.Name;
            OrderedRoots.Clear();
            foreach (var root in rule.Roots)
            {
                OrderedRoots.Add(root);
            }
            _isRuleDirty = false;
            InvalidatePreview("Rule loaded. Run Preview to compute a read-only virtual result.");
        }
        catch (OperationCanceledException) when (_cancellation?.IsCancellationRequested == true)
        {
        }
        catch (Exception exception)
        {
            if (generation == _generation)
            {
                PublishError($"The selected preference rule could not be loaded. {exception.Message}");
            }
        }
        finally
        {
            if (generation == _generation)
            {
                IsBusy = false;
            }
        }
    }

    private async Task SaveAsync()
    {
        if (!CanSave())
        {
            return;
        }
        Cancel(preserveRuleSelection: true);
        _cancellation = new CancellationTokenSource();
        var generation = ++_generation;
        IsBusy = true;
        ErrorMessage = null;
        try
        {
            var result = await _workerClient.SavePreferenceRuleAsync(
                Guid.NewGuid().ToString("N"),
                _ruleId,
                RuleName,
                OrderedRoots.ToArray(),
                _ruleRevision,
                _cancellation.Token);
            if (generation != _generation)
            {
                return;
            }
            _ruleId = result.Rule.Id;
            _ruleRevision = result.Rule.Revision;
            _isRuleDirty = false;
            var page = await _workerClient.ListPreferenceRulesAsync(0, 200, _cancellation.Token);
            if (generation != _generation)
            {
                return;
            }
            Rules = page.Rules;
            _selectedRule = Rules.FirstOrDefault(rule => rule.Id == _ruleId);
            OnPropertyChanged(nameof(SelectedRule));
            InvalidatePreview($"Saved {result.Rule.Name} at revision {result.Rule.Revision:N0}. Nothing was applied or deleted.");
            AnnouncementVersion++;
        }
        catch (OperationCanceledException) when (_cancellation?.IsCancellationRequested == true)
        {
        }
        catch (Exception exception)
        {
            if (generation == _generation)
            {
                PublishError($"The preference rule was not saved. {exception.Message}");
            }
        }
        finally
        {
            if (generation == _generation)
            {
                IsBusy = false;
            }
        }
    }

    private async Task LoadPreviewAsync(string? cursor)
    {
        if (!CanPreview() || _run is null || _ruleId is null)
        {
            return;
        }
        PreferencePreviewScope scope;
        switch (SelectedScope.Kind)
        {
            case PreferencePreviewScopeKind.SelectedSets:
                if (_selectedGroupId() is not { } groupId)
                {
                    PublishError("Select one duplicate set before previewing the Selected set scope.");
                    return;
                }
                scope = new PreferencePreviewScope(SelectedScope.Kind, [groupId]);
                break;
            case PreferencePreviewScopeKind.CurrentFilter:
                if (_currentFilter() is not { } filter)
                {
                    PublishError("Correct the duplicate-file filter before previewing its complete scope.");
                    return;
                }
                scope = new PreferencePreviewScope(SelectedScope.Kind, Filter: filter);
                break;
            default:
                scope = new PreferencePreviewScope(PreferencePreviewScopeKind.CompletedRun);
                break;
        }
        Cancel(preserveRuleSelection: true);
        _cancellation = new CancellationTokenSource();
        var generation = ++_generation;
        IsBusy = true;
        ErrorMessage = null;
        try
        {
            WorkerPreferencePreviewPage page;
            if (!_previewCache.TryGet(cursor, out page!))
            {
                page = await _workerClient.GetPreferencePreviewAsync(
                    new PreferencePreviewQuery(
                        _run.Id,
                        _ruleId.Value,
                        _ruleRevision,
                        _reviewRevision(),
                        PreviewPageSize,
                        scope,
                        cursor),
                    _cancellation.Token);
                if (generation != _generation || _ruleId != page.RuleId)
                {
                    return;
                }
                _previewCache.Set(cursor, page);
            }
            if (generation != _generation
                || page.RuleId != _ruleId
                || page.RuleRevision != _ruleRevision
                || page.ReviewRevision != _reviewRevision())
            {
                return;
            }
            _currentPage = page;
            _summary = page.Summary;
            PreviewGroups = page.Groups.Select(group => new PreferencePreviewGroupListItemViewModel(group)).ToArray();
            OnPropertyChanged(nameof(SummaryText));
            OnPropertyChanged(nameof(CanMoveNext));
            NextPageCommand.NotifyCanExecuteChanged();
            StatusMessage = $"Read-only preview complete for {SelectedScope.DisplayName.ToLowerInvariant()}: {page.Total:N0} applicable or blocked sets. Nothing was applied or deleted.";
            AnnouncementVersion++;
        }
        catch (OperationCanceledException) when (_cancellation?.IsCancellationRequested == true)
        {
        }
        catch (Exception exception)
        {
            if (generation == _generation)
            {
                PublishError($"The read-only preference preview failed. {exception.Message} Reload the rule and try again.");
            }
        }
        finally
        {
            if (generation == _generation)
            {
                IsBusy = false;
            }
        }
    }

    private Task NextPageAsync() =>
        _currentPage?.NextCursor is { } cursor ? LoadPreviewAsync(cursor) : Task.CompletedTask;

    private bool CanSave() =>
        !IsBusy
        && _run?.Status == "completed"
        && !string.IsNullOrWhiteSpace(RuleName)
        && RuleName == RuleName.Trim()
        && RuleName.EnumerateRunes().Count() <= 128
        && OrderedRoots.Count is >= 1 and <= 64;

    private bool CanPreview() =>
        !IsBusy && !_isRuleDirty && _run?.Status == "completed" && _ruleId is not null;

    private bool CanAddRoot() =>
        !IsBusy
        && OrderedRoots.Count < 64
        && !string.IsNullOrWhiteSpace(NewRoot)
        && NewRoot == NewRoot.Trim()
        && !OrderedRoots.Any(root => string.Equals(root, NewRoot, StringComparison.OrdinalIgnoreCase));

    private bool CanMoveRootUp() =>
        !IsBusy && SelectedRoot is not null && OrderedRoots.IndexOf(SelectedRoot) > 0;

    private bool CanMoveRootDown() =>
        !IsBusy
        && SelectedRoot is not null
        && OrderedRoots.IndexOf(SelectedRoot) is var index
        && index >= 0
        && index < OrderedRoots.Count - 1;

    private void AddRoot()
    {
        if (!CanAddRoot())
        {
            return;
        }
        OrderedRoots.Add(NewRoot);
        SelectedRoot = NewRoot;
        NewRoot = string.Empty;
        RuleEdited();
    }

    private void RemoveRoot()
    {
        if (SelectedRoot is not { } root)
        {
            return;
        }
        var index = OrderedRoots.IndexOf(root);
        OrderedRoots.Remove(root);
        SelectedRoot = OrderedRoots.Count == 0 ? null : OrderedRoots[Math.Min(index, OrderedRoots.Count - 1)];
        RuleEdited();
    }

    private void MoveRootUp()
    {
        if (SelectedRoot is not { } root)
        {
            return;
        }
        var index = OrderedRoots.IndexOf(root);
        if (index <= 0)
        {
            return;
        }
        OrderedRoots.Move(index, index - 1);
        RuleEdited();
    }

    private void MoveRootDown()
    {
        if (SelectedRoot is not { } root)
        {
            return;
        }
        var index = OrderedRoots.IndexOf(root);
        if (index < 0 || index >= OrderedRoots.Count - 1)
        {
            return;
        }
        OrderedRoots.Move(index, index + 1);
        RuleEdited();
    }

    private void NewRule()
    {
        _selectedRule = null;
        OnPropertyChanged(nameof(SelectedRule));
        _ruleId = null;
        _ruleRevision = 0;
        _isRuleDirty = true;
        RuleName = "Preferred scan roots";
        OrderedRoots.Clear();
        foreach (var root in _run?.Parameters.Roots.Take(64) ?? [])
        {
            OrderedRoots.Add(root);
        }
        SelectedRoot = OrderedRoots.FirstOrDefault();
        InvalidatePreview("New unsaved rule. Save it before previewing.");
    }

    private void RuleEdited()
    {
        _isRuleDirty = true;
        InvalidatePreview("Rule order changed. Save it before running a new preview.");
        RaiseCommandState();
    }

    private void InvalidatePreview(string message)
    {
        _previewCache.Clear();
        _currentPage = null;
        PreviewGroups = [];
        _summary = EmptySummary();
        OnPropertyChanged(nameof(SummaryText));
        OnPropertyChanged(nameof(CanMoveNext));
        NextPageCommand.NotifyCanExecuteChanged();
        StatusMessage = message;
        ErrorMessage = null;
    }

    private void PublishError(string message)
    {
        ErrorMessage = message;
        StatusMessage = message;
        AnnouncementVersion++;
    }

    private void RaiseCommandState()
    {
        SaveCommand.NotifyCanExecuteChanged();
        PreviewCommand.NotifyCanExecuteChanged();
        NextPageCommand.NotifyCanExecuteChanged();
        NewRuleCommand.NotifyCanExecuteChanged();
        AddRootCommand.NotifyCanExecuteChanged();
        RaiseRootCommandState();
    }

    private void RaiseRootCommandState()
    {
        MoveRootUpCommand.NotifyCanExecuteChanged();
        MoveRootDownCommand.NotifyCanExecuteChanged();
        RemoveRootCommand.NotifyCanExecuteChanged();
    }

    private void Cancel(bool preserveRuleSelection = false)
    {
        _cancellation?.Cancel();
        _cancellation?.Dispose();
        _cancellation = null;
        _generation++;
        IsBusy = false;
        if (!preserveRuleSelection)
        {
            _previewCache.Clear();
        }
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }
        _disposed = true;
        Cancel();
    }

    private static WorkerPreferencePreviewSummary EmptySummary() =>
        new(0, 0, 0, "0", 0, 0, 0, 0, 0, "0", 0, 0, 0, 0, 0, 0, 0, 0);
}
