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
    private readonly BoundedCursorCache<WorkerPreferenceApplicationPage> _applicationCache = new(CacheCapacity);
    private CancellationTokenSource? _cancellation;
    private WorkerRun? _run;
    private IReadOnlyList<WorkerPreferenceRuleSummary> _rules = [];
    private WorkerPreferenceRuleSummary? _selectedRule;
    private long? _ruleId;
    private long _ruleRevision;
    private long _knownReviewRevision;
    private string _ruleName = string.Empty;
    private string _newRoot = string.Empty;
    private string? _selectedRoot;
    private PreferencePreviewScopeOption _selectedScope;
    private IReadOnlyList<PreferencePreviewGroupListItemViewModel> _previewGroups = [];
    private WorkerPreferencePreviewPage? _currentPage;
    private PreferencePreviewScope? _previewScope;
    private WorkerPreferenceApplication? _latestApplication;
    private WorkerPreferencePreviewSummary _summary = EmptySummary();
    private bool _isBusy;
    private string? _errorMessage;
    private string _statusMessage = "Save or select a named rule to preview. Nothing will be applied or deleted.";
    private long _generation;
    private long _announcementVersion;
    private bool _isRuleDirty = true;
    private bool _isApplicationConfirmationVisible;
    private bool _isReversalConfirmationVisible;
    private string? _pendingApplyOperationId;
    private string? _pendingReverseOperationId;
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
        ApplyCommand = new RelayCommand(BeginApplicationConfirmation, CanApply);
        ConfirmApplicationCommand = new AsyncRelayCommand(ConfirmApplicationAsync, CanConfirmApplication);
        CancelApplicationCommand = new RelayCommand(CancelApplicationConfirmation, () => IsApplicationConfirmationVisible && !IsBusy);
        ReverseCommand = new RelayCommand(BeginReversalConfirmation, CanReverse);
        ConfirmReversalCommand = new AsyncRelayCommand(ConfirmReversalAsync, CanConfirmReversal);
        CancelReversalCommand = new RelayCommand(CancelReversalConfirmation, () => IsReversalConfirmationVisible && !IsBusy);
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

    public event Action<long, long>? ReviewRevisionChanged;

    public bool IsApplicationConfirmationVisible
    {
        get => _isApplicationConfirmationVisible;
        private set
        {
            if (SetProperty(ref _isApplicationConfirmationVisible, value))
            {
                OnPropertyChanged(nameof(ApplicationConfirmationText));
                RaiseCommandState();
            }
        }
    }

    public bool IsReversalConfirmationVisible
    {
        get => _isReversalConfirmationVisible;
        private set
        {
            if (SetProperty(ref _isReversalConfirmationVisible, value))
            {
                OnPropertyChanged(nameof(ReversalConfirmationText));
                RaiseCommandState();
            }
        }
    }

    public WorkerPreferenceApplication? LatestApplication
    {
        get => _latestApplication;
        private set
        {
            if (SetProperty(ref _latestApplication, value))
            {
                OnPropertyChanged(nameof(HasReversibleApplication));
                OnPropertyChanged(nameof(ApplicationHistoryText));
                OnPropertyChanged(nameof(ReversalConfirmationText));
                RaiseCommandState();
            }
        }
    }

    public bool HasReversibleApplication => LatestApplication?.State == "active";

    public string ApplicationConfirmationText =>
        _currentPage is null
            ? "Run Preview again before applying this rule."
            : $"Apply {RuleName} revision {_ruleRevision:N0} to {SelectedScope.DisplayName.ToLowerInvariant()} at review revision {_currentPage.ReviewRevision:N0}: "
              + $"{_summary.AffectedGroupCount:N0} applicable sets, {_summary.BlockedGroupCount:N0} blocked, "
              + $"{_summary.ProposedKeepPathCount:N0} rule Keeps and {_summary.ProposedRemovePathCount:N0} rule Removes "
              + $"({_summary.ProposedRemovePhysicalItemCount:N0} physical items, {DisplayFormatting.Bytes(_summary.ProposedRemoveBytes)}). "
              + "This changes review decisions only; it does not delete or validate files.";

    public string ReversalConfirmationText => LatestApplication is null
        ? "No active rule application is available to reverse."
        : $"Reverse application {LatestApplication.Id:N0} from {LatestApplication.RuleName}: clear "
          + $"{LatestApplication.Summary.RuleKeepPathCount:N0} rule Keeps and "
          + $"{LatestApplication.Summary.RuleRemovePathCount:N0} rule Removes. Manual file and folder choices will be preserved.";

    public string ApplicationHistoryText => LatestApplication is null
        ? "No rule application has been recorded for this rule and run."
        : $"Application {LatestApplication.Id:N0}: {LatestApplication.State}; review revision {LatestApplication.AppliedRevision:N0}; "
          + $"{LatestApplication.Summary.RuleKeepPathCount:N0} rule Keeps and {LatestApplication.Summary.RuleRemovePathCount:N0} rule Removes.";

    public string SummaryText =>
        $"{_summary.AffectedGroupCount:N0} applicable sets; {_summary.BlockedGroupCount:N0} blocked; "
        + $"{_summary.ProposedRemovePathCount:N0} logical removals; {_summary.ProposedRemovePhysicalItemCount:N0} physical items; "
        + $"{DisplayFormatting.Bytes(_summary.ProposedRemoveBytes)}. {_summary.TiedGroupCount:N0} ties; "
        + $"{_summary.ManualKeepPathCount:N0} manual Keeps; {_summary.MissingRuleRootCount:N0} configured roots absent from this scope.";

    public bool CanMoveNext => !IsBusy && _currentPage?.NextCursor is not null;

    public int CachedPageCount => _previewCache.Count;

    public IAsyncRelayCommand SaveCommand { get; }

    public IAsyncRelayCommand PreviewCommand { get; }

    public IRelayCommand ApplyCommand { get; }

    public IAsyncRelayCommand ConfirmApplicationCommand { get; }

    public IRelayCommand CancelApplicationCommand { get; }

    public IRelayCommand ReverseCommand { get; }

    public IAsyncRelayCommand ConfirmReversalCommand { get; }

    public IRelayCommand CancelReversalCommand { get; }

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
        _knownReviewRevision = _reviewRevision();
        RuleName = string.Empty;
        OrderedRoots.Clear();
        PreviewGroups = [];
        _currentPage = null;
        _previewScope = null;
        LatestApplication = null;
        _applicationCache.Clear();
        IsApplicationConfirmationVisible = false;
        IsReversalConfirmationVisible = false;
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
            _knownReviewRevision = Math.Max(_knownReviewRevision, revision);
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
            await LoadApplicationsAsync(rule.Id, generation, _cancellation.Token);
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
            _knownReviewRevision = page.ReviewRevision;
            _previewScope = scope;
            _summary = page.Summary;
            PreviewGroups = page.Groups.Select(group => new PreferencePreviewGroupListItemViewModel(group)).ToArray();
            OnPropertyChanged(nameof(SummaryText));
            OnPropertyChanged(nameof(CanMoveNext));
            NextPageCommand.NotifyCanExecuteChanged();
            ApplyCommand.NotifyCanExecuteChanged();
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

    private async Task LoadApplicationsAsync(long ruleId, long generation, CancellationToken cancellationToken)
    {
        if (_run is null)
        {
            return;
        }
        var page = await _workerClient.GetPreferenceApplicationsAsync(
            _run.Id,
            ruleId,
            "all",
            100,
            null,
            cancellationToken);
        if (generation != _generation || _run is null || _ruleId != ruleId)
        {
            return;
        }
        _applicationCache.Set(null, page);
        LatestApplication = page.Applications.FirstOrDefault(application => application.State == "active")
            ?? page.Applications.FirstOrDefault();
    }

    private void BeginApplicationConfirmation()
    {
        if (!CanApply())
        {
            return;
        }
        IsReversalConfirmationVisible = false;
        IsApplicationConfirmationVisible = true;
        StatusMessage = "Confirm the exact previewed rule application. No files will be deleted or validated.";
        AnnouncementVersion++;
    }

    private void CancelApplicationConfirmation()
    {
        IsApplicationConfirmationVisible = false;
        StatusMessage = "Rule application cancelled. The read-only preview remains available.";
        AnnouncementVersion++;
    }

    private async Task ConfirmApplicationAsync()
    {
        if (!CanConfirmApplication() || _run is null || _ruleId is null || _currentPage is null || _previewScope is null)
        {
            return;
        }
        _pendingApplyOperationId ??= Guid.NewGuid().ToString("N");
        var operationId = _pendingApplyOperationId;
        var runId = _run.Id;
        var generation = _generation;
        IsBusy = true;
        ErrorMessage = null;
        try
        {
            var result = await _workerClient.ApplyPreferenceRuleAsync(
                operationId,
                runId,
                _ruleId.Value,
                _ruleRevision,
                _currentPage.ReviewRevision,
                _currentPage.PreviewSignature,
                _previewScope,
                _cancellation?.Token ?? CancellationToken.None);
            if (generation != _generation || _run?.Id != runId || result.Application.RuleId != _ruleId)
            {
                return;
            }
            _pendingApplyOperationId = null;
            LatestApplication = result.Application;
            _knownReviewRevision = result.Application.AppliedRevision;
            IsApplicationConfirmationVisible = false;
            _previewCache.Clear();
            _currentPage = null;
            _previewScope = null;
            PreviewGroups = [];
            _summary = EmptySummary();
            OnPropertyChanged(nameof(SummaryText));
            StatusMessage = $"Applied {result.Application.RuleName}: {result.Application.Summary.RuleKeepPathCount:N0} rule Keeps and {result.Application.Summary.RuleRemovePathCount:N0} rule Removes at review revision {result.Application.AppliedRevision:N0}. Nothing was deleted.";
            AnnouncementVersion++;
            ReviewRevisionChanged?.Invoke(runId, result.Application.AppliedRevision);
        }
        catch (OperationCanceledException) when (_cancellation?.IsCancellationRequested == true)
        {
        }
        catch (Exception exception)
        {
            if (generation == _generation)
            {
                PublishError($"The rule application was not confirmed. {exception.Message} Run Preview again if the review changed.");
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

    private void BeginReversalConfirmation()
    {
        if (!CanReverse())
        {
            return;
        }
        IsApplicationConfirmationVisible = false;
        IsReversalConfirmationVisible = true;
        StatusMessage = "Confirm reversal of only this application's rule-produced decisions. Manual choices will remain.";
        AnnouncementVersion++;
    }

    private void CancelReversalConfirmation()
    {
        IsReversalConfirmationVisible = false;
        StatusMessage = "Rule-application reversal cancelled. Review decisions were not changed.";
        AnnouncementVersion++;
    }

    private async Task ConfirmReversalAsync()
    {
        if (!CanConfirmReversal() || _run is null || LatestApplication is null)
        {
            return;
        }
        _pendingReverseOperationId ??= Guid.NewGuid().ToString("N");
        var operationId = _pendingReverseOperationId;
        var application = LatestApplication;
        var runId = _run.Id;
        var expectedRevision = Math.Max(_knownReviewRevision, _reviewRevision());
        var generation = _generation;
        IsBusy = true;
        ErrorMessage = null;
        try
        {
            var result = await _workerClient.ReversePreferenceApplicationAsync(
                operationId,
                runId,
                application.Id,
                expectedRevision,
                _cancellation?.Token ?? CancellationToken.None);
            if (generation != _generation || _run?.Id != runId || result.ApplicationId != application.Id)
            {
                return;
            }
            _pendingReverseOperationId = null;
            LatestApplication = application with
            {
                State = "reversed",
                ReversedAt = DateTimeOffset.UtcNow.ToString("O"),
            };
            _knownReviewRevision = result.AppliedRevision;
            IsReversalConfirmationVisible = false;
            StatusMessage = $"Reversed application {application.Id:N0}: cleared {result.RemovedRuleKeepCount:N0} rule Keeps and {result.RemovedRuleRemoveCount:N0} rule Removes. Manual choices were preserved; nothing was deleted.";
            AnnouncementVersion++;
            ReviewRevisionChanged?.Invoke(runId, result.AppliedRevision);
        }
        catch (OperationCanceledException) when (_cancellation?.IsCancellationRequested == true)
        {
        }
        catch (Exception exception)
        {
            if (generation == _generation)
            {
                PublishError($"The rule application was not reversed. {exception.Message} Reload application history and try again.");
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

    private bool CanApply() =>
        !IsBusy
        && !_isRuleDirty
        && _currentPage is not null
        && _previewScope is not null
        && !string.IsNullOrWhiteSpace(_currentPage.PreviewSignature)
        && _currentPage.ReviewRevision == Math.Max(_knownReviewRevision, _reviewRevision())
        && _summary.AffectedGroupCount > 0;

    private bool CanConfirmApplication() => CanApply() && IsApplicationConfirmationVisible;

    private bool CanReverse() =>
        !IsBusy
        && LatestApplication?.State == "active"
        && _run?.Id == LatestApplication.RunId;

    private bool CanConfirmReversal() => CanReverse() && IsReversalConfirmationVisible;

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
        LatestApplication = null;
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
        _previewScope = null;
        PreviewGroups = [];
        IsApplicationConfirmationVisible = false;
        IsReversalConfirmationVisible = false;
        _pendingApplyOperationId = null;
        _pendingReverseOperationId = null;
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
        ApplyCommand.NotifyCanExecuteChanged();
        ConfirmApplicationCommand.NotifyCanExecuteChanged();
        CancelApplicationCommand.NotifyCanExecuteChanged();
        ReverseCommand.NotifyCanExecuteChanged();
        ConfirmReversalCommand.NotifyCanExecuteChanged();
        CancelReversalCommand.NotifyCanExecuteChanged();
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
