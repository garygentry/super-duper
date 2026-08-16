using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class RunHistoryViewModel : ObservableObject
{
    private const int PageSize = 500;
    private readonly IWorkerClient _workerClient;
    private long? _sessionId;
    private RunListItemViewModel? _selectedRun;
    private bool _isLoading;
    private string? _errorMessage;

    public RunHistoryViewModel(IWorkerClient workerClient)
    {
        _workerClient = workerClient;
        RefreshCommand = new AsyncRelayCommand(RefreshAsync, () => SessionId is not null && !IsLoading);
    }

    public ObservableCollection<RunListItemViewModel> Runs { get; } = [];

    public long? SessionId
    {
        get => _sessionId;
        private set
        {
            if (SetProperty(ref _sessionId, value))
            {
                RefreshCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public RunListItemViewModel? SelectedRun
    {
        get => _selectedRun;
        set
        {
            if (SetProperty(ref _selectedRun, value))
            {
                SelectedRunChanged?.Invoke(this, value?.Run);
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
                RefreshCommand.NotifyCanExecuteChanged();
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

    public bool IsEmpty => !IsLoading && Runs.Count == 0;

    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);

    public IAsyncRelayCommand RefreshCommand { get; }

    public event EventHandler<WorkerRun?>? SelectedRunChanged;

    public async Task LoadAsync(long sessionId, CancellationToken cancellationToken = default)
    {
        SessionId = sessionId;
        IsLoading = true;
        ErrorMessage = null;
        try
        {
            var runs = new List<WorkerRun>();
            long offset = 0;
            while (true)
            {
                var page = await _workerClient.ListRunsAsync(sessionId, offset, PageSize, cancellationToken);
                runs.AddRange(page.Runs);
                offset += page.Runs.Count;
                if (offset >= page.Total || page.Runs.Count == 0)
                {
                    break;
                }
            }

            var selectedId = SelectedRun?.Id;
            Runs.Clear();
            foreach (var run in runs)
            {
                Runs.Add(new RunListItemViewModel(run));
            }
            SelectedRun = selectedId is long id
                ? Runs.FirstOrDefault(item => item.Id == id) ?? Runs.FirstOrDefault()
                : Runs.FirstOrDefault();
            OnPropertyChanged(nameof(IsEmpty));
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception)
        {
            ErrorMessage = exception.Message;
        }
        finally
        {
            IsLoading = false;
        }
    }

    public void Clear()
    {
        SessionId = null;
        Runs.Clear();
        SelectedRun = null;
        ErrorMessage = null;
        OnPropertyChanged(nameof(IsEmpty));
    }

    public void Upsert(WorkerRun run, bool select)
    {
        if (SessionId != run.SessionId)
        {
            return;
        }
        var item = Runs.FirstOrDefault(existing => existing.Id == run.Id);
        if (item is null)
        {
            item = new RunListItemViewModel(run);
            Runs.Insert(0, item);
        }
        else
        {
            item.Update(run);
        }
        if (select)
        {
            SelectedRun = item;
        }
        OnPropertyChanged(nameof(IsEmpty));
    }

    private Task RefreshAsync() => SessionId is long sessionId
        ? LoadAsync(sessionId)
        : Task.CompletedTask;
}
