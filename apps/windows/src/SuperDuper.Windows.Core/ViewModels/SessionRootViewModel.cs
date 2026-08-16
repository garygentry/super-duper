using CommunityToolkit.Mvvm.ComponentModel;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class SessionRootViewModel : ObservableObject
{
    private string _path;

    public SessionRootViewModel(string path = "") => _path = path;

    public string Path
    {
        get => _path;
        set => SetProperty(ref _path, value);
    }
}
