using CommunityToolkit.Mvvm.ComponentModel;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class SessionListItemViewModel : ObservableObject
{
    private string _name;
    private string _statusText = "Ready";

    public SessionListItemViewModel(WorkerSessionDefinition session)
    {
        Id = session.Id;
        _name = session.Name;
    }

    public long Id { get; }

    public string Name
    {
        get => _name;
        private set => SetProperty(ref _name, value);
    }

    public string StatusText
    {
        get => _statusText;
        set => SetProperty(ref _statusText, value);
    }

    public void Update(WorkerSessionDefinition session) => Name = session.Name;
}
