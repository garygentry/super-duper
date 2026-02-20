using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.Models;

namespace SuperDuper.Controls;

public sealed partial class DirectoryTreeControl : UserControl
{
    public DirectoryTreeControl()
    {
        this.InitializeComponent();
    }

    public event EventHandler<string?>? SelectedDirectoryChanged;

    private void DirTree_ItemInvoked(TreeView sender, TreeViewItemInvokedEventArgs args)
    {
        if (args.InvokedItem is DirectoryNodeViewModel node)
            SelectedDirectoryChanged?.Invoke(this, node.Path);
    }

    // Lazy-load children on expand
    // Full implementation in Phase 3
}

public class DirectoryNodeViewModel
{
    public string Path { get; set; } = "";
    public string Name { get; set; } = "";
    public string DensityBadgeText { get; set; } = "";
    public List<DirectoryNodeViewModel> Children { get; set; } = new();
    public bool HasChildren { get; set; }
}
