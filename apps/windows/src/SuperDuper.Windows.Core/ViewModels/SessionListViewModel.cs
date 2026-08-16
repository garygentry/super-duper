using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class SessionListViewModel : ObservableObject
{
    private const int PageSize = 500;
    private readonly IWorkerClient _workerClient;
    private readonly Func<Task> _newSession;
    private SessionListItemViewModel? _selectedSession;
    private bool _isLoading;
    private bool _canMutate = true;
    private string? _errorMessage;

    public SessionListViewModel(IWorkerClient workerClient, Func<Task> newSession)
    {
        _workerClient = workerClient;
        _newSession = newSession;
        NewSessionCommand = new AsyncRelayCommand(CreateNewSessionAsync, () => CanMutate && !IsLoading);
        RefreshCommand = new AsyncRelayCommand(LoadAsync, () => !IsLoading);
    }

    public ObservableCollection<SessionListItemViewModel> Items { get; } = [];

    public SessionListItemViewModel? SelectedSession
    {
        get => _selectedSession;
        set
        {
            if (SetProperty(ref _selectedSession, value))
            {
                SelectionChanged?.Invoke(this, value);
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
                NewSessionCommand.NotifyCanExecuteChanged();
                RefreshCommand.NotifyCanExecuteChanged();
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
                NewSessionCommand.NotifyCanExecuteChanged();
            }
        }
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

    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);

    public bool IsEmpty => !IsLoading && Items.Count == 0;

    public IAsyncRelayCommand NewSessionCommand { get; }

    public IAsyncRelayCommand RefreshCommand { get; }

    public event EventHandler<SessionListItemViewModel?>? SelectionChanged;

    public async Task LoadAsync(CancellationToken cancellationToken = default)
    {
        IsLoading = true;
        ErrorMessage = null;
        try
        {
            var sessions = new List<WorkerSessionDefinition>();
            long offset = 0;
            while (true)
            {
                var page = await _workerClient.ListSessionsAsync(offset, PageSize, cancellationToken);
                sessions.AddRange(page.Sessions);
                offset += page.Sessions.Count;
                if (offset >= page.Total || page.Sessions.Count == 0)
                {
                    break;
                }
            }

            var selectedId = SelectedSession?.Id;
            Items.Clear();
            foreach (var session in sessions)
            {
                Items.Add(new SessionListItemViewModel(session));
            }
            SelectedSession = selectedId is long id
                ? Items.FirstOrDefault(item => item.Id == id)
                : Items.FirstOrDefault();
            OnPropertyChanged(nameof(IsEmpty));
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception)
        {
            ErrorMessage = exception.Message;
            OnPropertyChanged(nameof(IsEmpty));
        }
        finally
        {
            IsLoading = false;
        }
    }

    public void Upsert(WorkerSessionDefinition session, bool select)
    {
        var item = Items.FirstOrDefault(existing => existing.Id == session.Id);
        if (item is null)
        {
            item = new SessionListItemViewModel(session);
            Items.Add(item);
        }
        else
        {
            item.Update(session);
        }
        if (select)
        {
            SelectedSession = item;
        }
        OnPropertyChanged(nameof(IsEmpty));
    }

    public void Remove(long sessionId)
    {
        var item = Items.FirstOrDefault(existing => existing.Id == sessionId);
        if (item is null)
        {
            return;
        }
        var wasSelected = ReferenceEquals(SelectedSession, item);
        Items.Remove(item);
        if (wasSelected)
        {
            SelectedSession = Items.FirstOrDefault();
        }
        OnPropertyChanged(nameof(IsEmpty));
    }

    public IReadOnlyList<string> NamesExcept(long? sessionId) =>
        Items.Where(item => item.Id != sessionId).Select(item => item.Name).ToArray();

    public SessionListItemViewModel? Find(long sessionId) =>
        Items.FirstOrDefault(item => item.Id == sessionId);

    private Task CreateNewSessionAsync() => _newSession();
}
