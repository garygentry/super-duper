using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.Models;
using SuperDuper.Services;
using System.Collections.ObjectModel;

namespace SuperDuper.Controls;

public sealed partial class FileListControl : UserControl
{
    private readonly IDatabaseService _db;
    private bool _dupesOnly;

    public ObservableCollection<DbFileInfo> Files { get; } = new();

    public event EventHandler<DbFileInfo?>? SelectedFileChanged;

    public FileListControl()
    {
        this.InitializeComponent();
        _db = App.Services.GetRequiredService<IDatabaseService>();
    }

    private string? _directoryPath;
    public string? DirectoryPath
    {
        get => _directoryPath;
        set
        {
            _directoryPath = value;
            _ = ReloadAsync();
        }
    }

    private async Task ReloadAsync()
    {
        Files.Clear();
        if (_directoryPath == null) return;

        // Session ID — simplify to 0 (use active session lookup later)
        var result = await _db.QueryFilesInDirectoryAsync(_directoryPath, 0, 0, 200);
        foreach (var f in result.Items)
        {
            if (_dupesOnly && !f.IsDuplicate) continue;
            Files.Add(f);
        }
    }

    private void AllFiles_Checked(object sender, RoutedEventArgs e)
    {
        _dupesOnly = false;
        _ = ReloadAsync();
    }

    private void DupesOnly_Checked(object sender, RoutedEventArgs e)
    {
        _dupesOnly = true;
        _ = ReloadAsync();
    }
}
