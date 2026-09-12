using CommunityToolkit.Mvvm.ComponentModel;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class SessionRootViewModel : ObservableObject
{
    private string _path;
    private string _status = "Enter an absolute folder or drive path.";
    public string Status { get => _status; internal set => SetProperty(ref _status, value); }

    public SessionRootViewModel(string path = "") => _path = path;

    public string Path
    {
        get => _path;
        set => SetProperty(ref _path, value);
    }
}
