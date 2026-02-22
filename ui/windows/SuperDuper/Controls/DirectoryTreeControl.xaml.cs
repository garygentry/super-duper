using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using SuperDuper.Models;
using SuperDuper.NativeMethods;
using SuperDuper.Services;
using Windows.UI.Text;

namespace SuperDuper.Controls;

public sealed partial class DirectoryTreeControl : UserControl
{
    private readonly EngineWrapper _engine;

    public DirectoryTreeControl()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);
        this.DataContext = this;

        _engine = App.Services.GetRequiredService<EngineWrapper>();

        // Wire events (XAML compiler pass 2 doesn't generate IComponentConnector)
        DirTree.ItemInvoked += DirTree_ItemInvoked;
        DirTree.Expanding += DirTree_Expanding;

        // React to session changes while this control is alive
        var scanService = App.Services.GetRequiredService<ScanService>();
        scanService.ActiveSessionChanged += (_, _) => _ = LoadRootsAsync();

        _ = LoadRootsAsync();
    }

    public event EventHandler<string?>? SelectedDirectoryChanged;

    public async Task LoadRootsAsync()
    {
        DirTree.RootNodes.Clear();

        var nodes = await Task.Run(() => _engine.QueryDirectoryChildren(-1, 0, 200));
        foreach (var n in nodes)
        {
            var vm = new DirectoryNodeViewModel
            {
                Id = n.Id,
                Path = n.Path,
                Name = n.Name,
                HasChildren = true,
                TotalFiles = (int)n.FileCount,
            };
            DirTree.RootNodes.Add(new TreeViewNode
            {
                Content = vm,
                HasUnrealizedChildren = true,
            });
        }
    }

    private void DirTree_ItemInvoked(TreeView sender, TreeViewItemInvokedEventArgs args)
    {
        // With node-based TreeView, InvokedItem is the TreeViewNode
        if (args.InvokedItem is TreeViewNode treeNode && treeNode.Content is DirectoryNodeViewModel node)
            SelectedDirectoryChanged?.Invoke(this, node.Path);
        else if (args.InvokedItem is DirectoryNodeViewModel directNode)
            SelectedDirectoryChanged?.Invoke(this, directNode.Path);
    }

    private async void DirTree_Expanding(TreeView sender, TreeViewExpandingEventArgs args)
    {
        if (!args.Node.HasUnrealizedChildren) return;
        args.Node.HasUnrealizedChildren = false;

        if (args.Node.Content is DirectoryNodeViewModel parent)
        {
            var children = await Task.Run(() => _engine.QueryDirectoryChildren(parent.Id, 0, 200));
            foreach (var c in children)
            {
                var vm = new DirectoryNodeViewModel
                {
                    Id = c.Id,
                    Path = c.Path,
                    Name = c.Name,
                    HasChildren = true,
                    TotalFiles = (int)c.FileCount,
                };
                args.Node.Children.Add(new TreeViewNode
                {
                    Content = vm,
                    HasUnrealizedChildren = true,
                });
            }
        }
    }
}

public class DirectoryNodeViewModel
{
    public long Id { get; set; }
    public string Path { get; set; } = "";
    public string Name { get; set; } = "";
    public string DensityBadgeText { get; set; } = "";
    public List<DirectoryNodeViewModel> Children { get; set; } = new();
    public bool HasChildren { get; set; }

    // Enriched properties for UI compliance
    public int DuplicateCount { get; set; }
    public int TotalFiles { get; set; }
    public int ReviewedCount { get; set; }
    public DensityLevel DensityLevel { get; set; }
    public string DriveLetter { get; set; } = "";

    // Computed properties
    public bool HasDuplicates => DuplicateCount > 0;
    public double ReviewProgress => DuplicateCount > 0 ? (double)ReviewedCount / DuplicateCount * 100 : 0;
    public bool IsReviewComplete => DuplicateCount > 0 && ReviewedCount >= DuplicateCount;
    public SolidColorBrush DriveColorBrush { get; set; } = new(Microsoft.UI.Colors.Gray);
    public FontWeight NameWeight => HasDuplicates ? FontWeights.SemiBold : FontWeights.Normal;
    public Visibility DuplicateVisibility => HasDuplicates ? Visibility.Visible : Visibility.Collapsed;
}
