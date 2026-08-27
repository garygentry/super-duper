using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Validation;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed record RepeatCachePolicyOption(string Value, string DisplayName, string Description);

public sealed class SessionSetupViewModel : ObservableObject
{
    private readonly IWorkerClient _workerClient;
    private readonly IFolderPickerService _folderPicker;
    private readonly IUserConfirmationService _confirmation;
    private readonly ICloudLocationService _cloudLocations;
    private readonly Func<long?, IReadOnlyList<string>> _otherSessionNames;
    private long? _sessionId;
    private string _name = "";
    private string _ignorePatternsText = "";
    private string _manualLocationExclusionsText = "";
    private string _cloudDetectionStatus = CloudDetectionStatusNames.Unavailable;
    private string? _cloudDetectionMessage;
    private bool _isDetectingCloudLocations;
    private IReadOnlyList<WorkerRegisteredCloudLocation> _registeredCloudLocations = [];
    private string? _manualExclusionValidationError;
    private bool _isBusy;
    private bool _canMutate = true;
    private bool _isDirty;
    private bool _suppressChanges;
    private string? _operationError;
    private string _repeatCachePolicy = RepeatCachePolicyNames.ReuseVerified;
    private SessionValidationResult _validation = new([], [], [], ["Enter a session name."], false);

    public SessionSetupViewModel(
        IWorkerClient workerClient,
        IFolderPickerService folderPicker,
        IUserConfirmationService confirmation,
        Func<long?, IReadOnlyList<string>> otherSessionNames,
        ICloudLocationService? cloudLocations = null)
    {
        _workerClient = workerClient;
        _folderPicker = folderPicker;
        _confirmation = confirmation;
        _cloudLocations = cloudLocations ?? new UnavailableCloudLocationService();
        _otherSessionNames = otherSessionNames;
        Roots.CollectionChanged += OnRootsChanged;

        AddRootCommand = new RelayCommand(AddRoot, () => CanEdit);
        BrowseRootCommand = new AsyncRelayCommand(BrowseRootAsync, () => CanEdit);
        RemoveRootCommand = new RelayCommand<SessionRootViewModel>(RemoveRoot, _ => CanEdit);
        NormalizeRootsCommand = new RelayCommand(NormalizeRoots, () => CanEdit && Roots.Count > 0);
        SaveCommand = new AsyncRelayCommand(SaveCommandAsync, () => CanSave);
        DeleteCommand = new AsyncRelayCommand(DeleteAsync, () => CanDelete);
        RefreshCloudLocationsCommand = new AsyncRelayCommand(
            () => RefreshCloudLocationsAsync(),
            () => CanEdit);
    }

    public ObservableCollection<SessionRootViewModel> Roots { get; } = [];

    public ObservableCollection<CloudLocationListItemViewModel> DetectedCloudLocations { get; } = [];

    public IReadOnlyList<RepeatCachePolicyOption> RepeatCachePolicies { get; } =
    [
        new(
            RepeatCachePolicyNames.ReuseVerified,
            "Reuse verified hashes (recommended)",
            "Reuses hashes only when stable file identity and qualified change metadata still match. Any uncertainty falls back to reading the file."),
        new(
            RepeatCachePolicyNames.RevalidateContent,
            "Always read file content",
            "Bypasses cache hits and reads file content again while still refreshing verified cache entries."),
    ];

    public long? SessionId
    {
        get => _sessionId;
        private set
        {
            if (SetProperty(ref _sessionId, value))
            {
                OnPropertyChanged(nameof(IsNew));
                RefreshCommands();
            }
        }
    }

    public string Name
    {
        get => _name;
        set
        {
            if (SetProperty(ref _name, value))
            {
                MarkChanged();
            }
        }
    }

    public string RepeatCachePolicy
    {
        get => _repeatCachePolicy;
        set
        {
            if (!RepeatCachePolicyNames.IsSupported(value))
            {
                throw new ArgumentOutOfRangeException(nameof(value), value, "Unsupported repeat-cache policy.");
            }
            if (SetProperty(ref _repeatCachePolicy, value))
            {
                OnPropertyChanged(nameof(RepeatCachePolicyDescription));
            }
        }
    }

    public string RepeatCachePolicyDescription => RepeatCachePolicies
        .Single(option => option.Value == RepeatCachePolicy)
        .Description;

    public string IgnorePatternsText
    {
        get => _ignorePatternsText;
        set
        {
            if (SetProperty(ref _ignorePatternsText, value))
            {
                MarkChanged();
            }
        }
    }

    public string ManualLocationExclusionsText
    {
        get => _manualLocationExclusionsText;
        set
        {
            if (SetProperty(ref _manualLocationExclusionsText, value))
            {
                MarkChanged();
            }
        }
    }

    public string CloudPolicy => CloudPolicyNames.ExcludeRegisteredRoots;

    public string CloudPolicyDisplayName => "Exclude registered cloud sync roots";

    public string CloudDetectionStatus
    {
        get => _cloudDetectionStatus;
        private set
        {
            if (SetProperty(ref _cloudDetectionStatus, value))
            {
                OnPropertyChanged(nameof(IsCloudDetectionReady));
                OnPropertyChanged(nameof(CloudDetectionSummary));
                OnPropertyChanged(nameof(CanStart));
            }
        }
    }

    public string? CloudDetectionMessage
    {
        get => _cloudDetectionMessage;
        private set
        {
            if (SetProperty(ref _cloudDetectionMessage, value))
            {
                OnPropertyChanged(nameof(HasCloudDetectionMessage));
                OnPropertyChanged(nameof(CloudDetectionSummary));
            }
        }
    }

    public bool IsDetectingCloudLocations
    {
        get => _isDetectingCloudLocations;
        private set
        {
            if (SetProperty(ref _isDetectingCloudLocations, value))
            {
                OnPropertyChanged(nameof(CanEdit));
                OnPropertyChanged(nameof(CanStart));
                OnPropertyChanged(nameof(CloudDetectionSummary));
                RefreshCommands();
            }
        }
    }

    public bool IsBusy
    {
        get => _isBusy;
        private set
        {
            if (SetProperty(ref _isBusy, value))
            {
                OnPropertyChanged(nameof(CanEdit));
                RefreshCommands();
            }
        }
    }

    public bool CanMutate
    {
        get => _canMutate;
        set
        {
            if (SetProperty(ref _canMutate, value))
            {
                OnPropertyChanged(nameof(CanEdit));
                OnPropertyChanged(nameof(CanStart));
                RefreshCommands();
            }
        }
    }

    public bool IsDirty
    {
        get => _isDirty;
        private set
        {
            if (SetProperty(ref _isDirty, value))
            {
                OnPropertyChanged(nameof(CanSave));
                RefreshCommands();
            }
        }
    }

    public string? OperationError
    {
        get => _operationError;
        private set
        {
            if (SetProperty(ref _operationError, value))
            {
                OnPropertyChanged(nameof(HasOperationError));
            }
        }
    }

    public bool IsNew => SessionId is null;

    public bool CanEdit => CanMutate && !IsBusy && !IsDetectingCloudLocations;

    public bool CanSave => CanEdit && IsDirty && _validation.IsValid && _manualExclusionValidationError is null;

    public bool CanDelete => CanEdit && SessionId is not null;

    public bool CanStart => CanEdit && _validation.IsValid && _validation.HasReachableRoot
        && _manualExclusionValidationError is null && IsCloudDetectionReady;

    public bool HasOperationError => !string.IsNullOrWhiteSpace(OperationError);

    public string ValidationMessage => string.Join(
        Environment.NewLine,
        _validation.Errors.Concat(
            _manualExclusionValidationError is null ? [] : [_manualExclusionValidationError]));

    public bool HasValidationErrors => _validation.Errors.Count > 0 || _manualExclusionValidationError is not null;

    public string WarningMessage => string.Join(Environment.NewLine, _validation.Warnings);

    public bool HasWarnings => _validation.Warnings.Count > 0;

    public bool IsCloudDetectionReady => CloudDetectionStatus == CloudDetectionStatusNames.Complete;

    public bool HasCloudDetectionMessage => !string.IsNullOrWhiteSpace(CloudDetectionMessage);

    public bool HasDetectedCloudLocations => DetectedCloudLocations.Count > 0;

    public string CloudDetectionSummary => IsDetectingCloudLocations
        ? "Checking registered cloud locations…"
        : CloudDetectionStatus switch
        {
            CloudDetectionStatusNames.Complete when DetectedCloudLocations.Count == 0 =>
                "No registered cloud locations intersect the selected scan roots.",
            CloudDetectionStatusNames.Complete =>
                $"{DetectedCloudLocations.Count:N0} registered cloud location(s) will be excluded.",
            CloudDetectionStatusNames.Unsupported =>
                "Windows Cloud Files registration detection is not supported. Scans fail closed under this policy.",
            _ => "Registered cloud location detection is unavailable. Refresh before starting a scan.",
        };

    public IRelayCommand AddRootCommand { get; }

    public IAsyncRelayCommand BrowseRootCommand { get; }

    public IRelayCommand<SessionRootViewModel> RemoveRootCommand { get; }

    public IRelayCommand NormalizeRootsCommand { get; }

    public IAsyncRelayCommand SaveCommand { get; }

    public IAsyncRelayCommand DeleteCommand { get; }

    public IAsyncRelayCommand RefreshCloudLocationsCommand { get; }

    public event EventHandler<WorkerSessionDefinition>? SessionSaved;

    public event EventHandler<long>? SessionDeleted;

    public void BeginNew()
    {
        _suppressChanges = true;
        try
        {
            SessionId = null;
            RepeatCachePolicy = RepeatCachePolicyNames.ReuseVerified;
            Name = "New session";
            ReplaceRoots([""]);
            IgnorePatternsText = string.Join(Environment.NewLine, SessionDefinitionValidator.SafeWindowsIgnorePatterns);
            ManualLocationExclusionsText = "";
            _registeredCloudLocations = [];
            CloudDetectionStatus = CloudDetectionStatusNames.Unavailable;
            CloudDetectionMessage = null;
            RefreshDetectedCloudLocations();
            OperationError = null;
            IsDirty = true;
        }
        finally
        {
            _suppressChanges = false;
        }
        Validate();
        _ = RefreshCloudLocationsAsync();
    }

    public void Load(WorkerSessionDefinition session)
    {
        _suppressChanges = true;
        try
        {
            SessionId = session.Id;
            Name = session.Name;
            ReplaceRoots(session.Roots);
            IgnorePatternsText = string.Join(Environment.NewLine, session.IgnorePatterns);
            ManualLocationExclusionsText = string.Join(Environment.NewLine, session.ManualLocationExclusions);
            _registeredCloudLocations = session.RegisteredCloudLocations;
            CloudDetectionStatus = session.CloudDetectionStatus;
            CloudDetectionMessage = null;
            RefreshDetectedCloudLocations();
            OperationError = null;
            IsDirty = false;
        }
        finally
        {
            _suppressChanges = false;
        }
        Validate();
        _ = RefreshCloudLocationsAsync();
    }

    public async Task<WorkerSessionDefinition?> EnsureSavedAsync(
        bool requireReachableRoot,
        CancellationToken cancellationToken = default)
    {
        Validate();
        if (!_validation.IsValid)
        {
            OperationError = ValidationMessage;
            return null;
        }
        if (requireReachableRoot && !_validation.HasReachableRoot)
        {
            OperationError = "At least one scan root must be available before starting a scan.";
            return null;
        }
        IsBusy = true;
        OperationError = null;
        try
        {
            await DetectCloudLocationsCoreAsync(cancellationToken);
            if (requireReachableRoot && !IsCloudDetectionReady)
            {
                OperationError = CloudDetectionSummary;
                return null;
            }
            if (requireReachableRoot)
            {
                var hasReachableRoot = await Task.Run(
                    () => _validation.Roots.Any(Directory.Exists),
                    cancellationToken);
                if (!hasReachableRoot)
                {
                    OperationError = "At least one scan root must be available before starting a scan.";
                    return null;
                }
            }

            if (!IsDirty && SessionId is long existingId)
            {
                return await _workerClient.GetSessionAsync(existingId, cancellationToken);
            }

            var session = SessionId is long sessionId
                ? await _workerClient.UpdateSessionAsync(
                    sessionId,
                    Name.Trim(),
                    _validation.Roots,
                    _validation.IgnorePatterns,
                    CloudPolicy,
                    NormalizeManualLocationExclusions(),
                    _registeredCloudLocations,
                    CloudDetectionStatus,
                    cancellationToken)
                : await _workerClient.CreateSessionAsync(
                    Name.Trim(),
                    _validation.Roots,
                    _validation.IgnorePatterns,
                    CloudPolicy,
                    NormalizeManualLocationExclusions(),
                    _registeredCloudLocations,
                    CloudDetectionStatus,
                    cancellationToken);
            Load(session);
            SessionSaved?.Invoke(this, session);
            return session;
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception)
        {
            OperationError = exception.Message;
            return null;
        }
        finally
        {
            IsBusy = false;
        }
    }

    private async Task SaveCommandAsync() =>
        _ = await EnsureSavedAsync(requireReachableRoot: false);

    private async Task DeleteAsync()
    {
        if (SessionId is not long sessionId)
        {
            return;
        }
        var confirmed = await _confirmation.ConfirmAsync(
            "Delete session?",
            $"Delete '{Name}' and all of its run history and results? This cannot be undone.");
        if (!confirmed)
        {
            return;
        }

        IsBusy = true;
        OperationError = null;
        try
        {
            await _workerClient.DeleteSessionAsync(sessionId);
            SessionDeleted?.Invoke(this, sessionId);
        }
        catch (Exception exception)
        {
            OperationError = exception.Message;
        }
        finally
        {
            IsBusy = false;
        }
    }

    private void AddRoot()
    {
        if (Roots.Any(root => string.IsNullOrWhiteSpace(root.Path)))
        {
            return;
        }
        Roots.Add(new SessionRootViewModel());
    }

    private async Task BrowseRootAsync()
    {
        var folder = await _folderPicker.PickFolderAsync();
        if (!string.IsNullOrWhiteSpace(folder))
        {
            var blank = Roots.FirstOrDefault(root => string.IsNullOrWhiteSpace(root.Path));
            if (blank is null)
            {
                Roots.Add(new SessionRootViewModel(folder));
            }
            else
            {
                blank.Path = folder;
            }
            await RefreshCloudLocationsAsync();
        }
    }

    private void RemoveRoot(SessionRootViewModel? root)
    {
        if (root is not null)
        {
            Roots.Remove(root);
        }
    }

    private void NormalizeRoots()
    {
        Validate();
        ReplaceRoots(_validation.Roots);
        IsDirty = true;
        Validate();
    }

    private void ReplaceRoots(IEnumerable<string> roots)
    {
        foreach (var root in Roots)
        {
            root.PropertyChanged -= OnRootPropertyChanged;
        }
        Roots.Clear();
        foreach (var path in roots)
        {
            var root = new SessionRootViewModel(path);
            root.PropertyChanged += OnRootPropertyChanged;
            Roots.Add(root);
        }
    }

    private void OnRootsChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        if (e.OldItems is not null)
        {
            foreach (SessionRootViewModel root in e.OldItems)
            {
                root.PropertyChanged -= OnRootPropertyChanged;
            }
        }
        if (e.NewItems is not null)
        {
            foreach (SessionRootViewModel root in e.NewItems)
            {
                root.PropertyChanged -= OnRootPropertyChanged;
                root.PropertyChanged += OnRootPropertyChanged;
            }
        }
        MarkChanged();
        OnPropertyChanged(nameof(CanSave));
        RefreshCommands();
    }

    private void OnRootPropertyChanged(object? sender, PropertyChangedEventArgs e) => MarkChanged();

    private void MarkChanged()
    {
        if (_suppressChanges)
        {
            return;
        }
        IsDirty = true;
        OperationError = null;
        Validate();
    }

    private void Validate()
    {
        _validation = SessionDefinitionValidator.Validate(
            Name,
            Roots.Select(root => root.Path),
            SplitIgnorePatterns(IgnorePatternsText),
            _otherSessionNames(SessionId));
        try
        {
            _ = NormalizeManualLocationExclusions();
            _manualExclusionValidationError = null;
        }
        catch (InvalidOperationException exception)
        {
            _manualExclusionValidationError = exception.Message;
        }
        OnPropertyChanged(nameof(ValidationMessage));
        OnPropertyChanged(nameof(HasValidationErrors));
        OnPropertyChanged(nameof(WarningMessage));
        OnPropertyChanged(nameof(HasWarnings));
        OnPropertyChanged(nameof(CanSave));
        OnPropertyChanged(nameof(CanStart));
        OnPropertyChanged(nameof(CanSave));
        OnPropertyChanged(nameof(CloudDetectionSummary));
        RefreshDetectedCloudLocations();
        RefreshCommands();
    }

    private void RefreshCommands()
    {
        AddRootCommand.NotifyCanExecuteChanged();
        BrowseRootCommand.NotifyCanExecuteChanged();
        RemoveRootCommand.NotifyCanExecuteChanged();
        NormalizeRootsCommand.NotifyCanExecuteChanged();
        SaveCommand.NotifyCanExecuteChanged();
        DeleteCommand.NotifyCanExecuteChanged();
        RefreshCloudLocationsCommand.NotifyCanExecuteChanged();
    }

    private static IEnumerable<string> SplitIgnorePatterns(string value) =>
        value.Split(["\r\n", "\n", "\r"], StringSplitOptions.None);

    private async Task RefreshCloudLocationsAsync(CancellationToken cancellationToken = default)
    {
        IsDetectingCloudLocations = true;
        OperationError = null;
        try
        {
            await DetectCloudLocationsCoreAsync(cancellationToken);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception)
        {
            CloudDetectionStatus = CloudDetectionStatusNames.Unavailable;
            CloudDetectionMessage = $"Registered cloud location detection failed: {exception.Message}";
            _registeredCloudLocations = [];
            RefreshDetectedCloudLocations();
        }
        finally
        {
            IsDetectingCloudLocations = false;
        }
    }

    private async Task DetectCloudLocationsCoreAsync(CancellationToken cancellationToken)
    {
        var result = await _cloudLocations.DetectAsync(cancellationToken);
        var changed = CloudDetectionStatus != result.Status
            || !_registeredCloudLocations.SequenceEqual(result.Locations);
        CloudDetectionStatus = result.Status;
        CloudDetectionMessage = result.ErrorMessage;
        _registeredCloudLocations = result.Locations;
        RefreshDetectedCloudLocations();
        if (changed && !_suppressChanges)
        {
            IsDirty = true;
        }
    }

    private IReadOnlyList<string> NormalizeManualLocationExclusions()
    {
        var normalized = new List<string>();
        foreach (var raw in SplitIgnorePatterns(ManualLocationExclusionsText))
        {
            var path = raw.Trim();
            if (path.Length == 0)
            {
                continue;
            }
            if (!Path.IsPathFullyQualified(path))
            {
                throw new InvalidOperationException("Manual cloud location exclusions must be absolute paths.");
            }
            path = Path.TrimEndingDirectorySeparator(path);
            if (normalized.Any(existing => IsPathWithin(path, existing)))
            {
                continue;
            }
            normalized.RemoveAll(existing => IsPathWithin(existing, path));
            normalized.Add(path);
        }
        return normalized;
    }

    private void RefreshDetectedCloudLocations()
    {
        DetectedCloudLocations.Clear();
        var roots = Roots
            .Select(root => root.Path.Trim())
            .Where(path => Path.IsPathFullyQualified(path))
            .ToArray();
        foreach (var location in _registeredCloudLocations)
        {
            var selectedInside = roots.Any(root => IsPathWithin(root, location.Path));
            var locationInside = roots.Any(root => IsPathWithin(location.Path, root));
            if (!selectedInside && !locationInside)
            {
                continue;
            }
            DetectedCloudLocations.Add(new CloudLocationListItemViewModel(
                location.DisplayName,
                location.Path,
                selectedInside
                    ? "A selected scan root is inside this location and will be fully excluded."
                    : "This registered subtree will be excluded from the broader scan root."));
        }
        OnPropertyChanged(nameof(HasDetectedCloudLocations));
        OnPropertyChanged(nameof(CloudDetectionSummary));
    }

    private static bool IsPathWithin(string path, string ancestor)
    {
        var candidate = Path.TrimEndingDirectorySeparator(Path.GetFullPath(path));
        var parent = Path.TrimEndingDirectorySeparator(Path.GetFullPath(ancestor));
        return candidate.Equals(parent, StringComparison.OrdinalIgnoreCase)
            || candidate.StartsWith(parent + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)
            || candidate.StartsWith(parent + Path.AltDirectorySeparatorChar, StringComparison.OrdinalIgnoreCase);
    }
}
