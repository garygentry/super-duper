using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using SuperDuper.Models;
using SuperDuper.Services;
using System.Collections.ObjectModel;

namespace SuperDuper.Controls;

public sealed partial class FileListControl : UserControl
{
    private readonly IDatabaseService _db;
    private bool _dupesOnly;
    private string _sortColumn = "file_name";

    public ObservableCollection<DbFileInfo> Files { get; } = new();

    public event EventHandler<DbFileInfo?>? SelectedFileChanged;

    public FileListControl()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);
        this.DataContext = this;
        _db = App.Services.GetRequiredService<IDatabaseService>();

        // Wire events (XAML compiler pass 2 doesn't generate IComponentConnector)
        SortCombo.SelectionChanged += SortCombo_SelectionChanged;
        FileRepeater.ElementPrepared += FileRepeater_ElementPrepared;
        // RadioButtons don't have x:Name — wire via Loaded to find them
        this.Loaded += (_, _) => WireRadioButtons();

        // React to session changes while this control is alive
        var scanService = App.Services.GetRequiredService<ScanService>();
        scanService.ActiveSessionChanged += (_, _) =>
        {
            if (_directoryPath != null)
                _ = ReloadAsync();
        };
    }

    private void FileRepeater_ElementPrepared(ItemsRepeater sender, ItemsRepeaterElementPreparedEventArgs args)
    {
        if (args.Element is FrameworkElement element)
        {
            element.Tapped -= FileRow_Tapped;
            element.Tapped += FileRow_Tapped;
        }
    }

    private void FileRow_Tapped(object sender, TappedRoutedEventArgs e)
    {
        if (sender is FrameworkElement el && el.DataContext is DbFileInfo file)
            SelectedFileChanged?.Invoke(this, file);
    }

    private void WireRadioButtons()
    {
        // Walk the visual tree to find RadioButtons by GroupName
        foreach (var rb in FindChildren<RadioButton>(this))
        {
            if (rb.GroupName == "FileMode")
            {
                if (rb.Content?.ToString() == "All Files")
                    rb.Checked += AllFiles_Checked;
                else if (rb.Content?.ToString() == "Duplicates Only")
                    rb.Checked += DupesOnly_Checked;
            }
        }
    }

    private static IEnumerable<T> FindChildren<T>(DependencyObject parent) where T : DependencyObject
    {
        var count = VisualTreeHelper.GetChildrenCount(parent);
        for (int i = 0; i < count; i++)
        {
            var child = VisualTreeHelper.GetChild(parent, i);
            if (child is T t) yield return t;
            foreach (var sub in FindChildren<T>(child)) yield return sub;
        }
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

        var sessionId = App.Services.GetRequiredService<ScanService>().ActiveSessionId ?? 0;
        var result = await _db.QueryFilesInDirectoryAsync(
            _directoryPath, sessionId, 0, 200, _sortColumn, _sortColumn != "file_size");
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

    private void SortCombo_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (SortCombo.SelectedItem is ComboBoxItem item && item.Tag is string tag)
        {
            _sortColumn = tag;
            _ = ReloadAsync();
        }
    }
}
