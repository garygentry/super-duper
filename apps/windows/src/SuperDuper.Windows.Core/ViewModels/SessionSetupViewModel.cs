using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.Validation;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class SessionSetupViewModel : ObservableObject
{
    private readonly IWorkerClient _workerClient;
    private readonly IFolderPickerService _folderPicker;
    private readonly IUserConfirmationService _confirmation;
    private readonly Func<long?, IReadOnlyList<string>> _otherSessionNames;
    private long? _sessionId;
    private string _name = "";
    private string _ignorePatternsText = "";
    private bool _isBusy;
    private bool _canMutate = true;
    private bool _isDirty;
    private bool _suppressChanges;
    private string? _operationError;
    private SessionValidationResult _validation = new([], [], [], ["Enter a session name."], false);

    public SessionSetupViewModel(
        IWorkerClient workerClient,
        IFolderPickerService folderPicker,
        IUserConfirmationService confirmation,
        Func<long?, IReadOnlyList<string>> otherSessionNames)
    {
        _workerClient = workerClient;
        _folderPicker = folderPicker;
        _confirmation = confirmation;
        _otherSessionNames = otherSessionNames;
        Roots.CollectionChanged += OnRootsChanged;

        AddRootCommand = new RelayCommand(AddRoot, () => CanEdit);
        BrowseRootCommand = new AsyncRelayCommand(BrowseRootAsync, () => CanEdit);
        RemoveRootCommand = new RelayCommand<SessionRootViewModel>(RemoveRoot, _ => CanEdit);
        NormalizeRootsCommand = new RelayCommand(NormalizeRoots, () => CanEdit && Roots.Count > 0);
        SaveCommand = new AsyncRelayCommand(SaveCommandAsync, () => CanSave);
        DeleteCommand = new AsyncRelayCommand(DeleteAsync, () => CanDelete);
    }

    public ObservableCollection<SessionRootViewModel> Roots { get; } = [];

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

    public bool CanEdit => CanMutate && !IsBusy;

    public bool CanSave => CanEdit && IsDirty && _validation.IsValid;

    public bool CanDelete => CanEdit && SessionId is not null;

    public bool CanStart => CanEdit && _validation.IsValid && _validation.HasReachableRoot;

    public bool HasOperationError => !string.IsNullOrWhiteSpace(OperationError);

    public string ValidationMessage => string.Join(Environment.NewLine, _validation.Errors);

    public bool HasValidationErrors => _validation.Errors.Count > 0;

    public string WarningMessage => string.Join(Environment.NewLine, _validation.Warnings);

    public bool HasWarnings => _validation.Warnings.Count > 0;

    public IRelayCommand AddRootCommand { get; }

    public IAsyncRelayCommand BrowseRootCommand { get; }

    public IRelayCommand<SessionRootViewModel> RemoveRootCommand { get; }

    public IRelayCommand NormalizeRootsCommand { get; }

    public IAsyncRelayCommand SaveCommand { get; }

    public IAsyncRelayCommand DeleteCommand { get; }

    public event EventHandler<WorkerSessionDefinition>? SessionSaved;

    public event EventHandler<long>? SessionDeleted;

    public void BeginNew()
    {
        _suppressChanges = true;
        try
        {
            SessionId = null;
            Name = "New session";
            ReplaceRoots([""]);
            IgnorePatternsText = string.Join(Environment.NewLine, SessionDefinitionValidator.SafeWindowsIgnorePatterns);
            OperationError = null;
            IsDirty = true;
        }
        finally
        {
            _suppressChanges = false;
        }
        Validate();
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
            OperationError = null;
            IsDirty = false;
        }
        finally
        {
            _suppressChanges = false;
        }
        Validate();
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
                    cancellationToken)
                : await _workerClient.CreateSessionAsync(
                    Name.Trim(),
                    _validation.Roots,
                    _validation.IgnorePatterns,
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
        OnPropertyChanged(nameof(ValidationMessage));
        OnPropertyChanged(nameof(HasValidationErrors));
        OnPropertyChanged(nameof(WarningMessage));
        OnPropertyChanged(nameof(HasWarnings));
        OnPropertyChanged(nameof(CanSave));
        OnPropertyChanged(nameof(CanStart));
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
    }

    private static IEnumerable<string> SplitIgnorePatterns(string value) =>
        value.Split(["\r\n", "\n", "\r"], StringSplitOptions.None);
}
