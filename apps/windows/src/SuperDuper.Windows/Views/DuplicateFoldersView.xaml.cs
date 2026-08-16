using System.ComponentModel;
using System.Windows.Controls;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Views;

public partial class DuplicateFoldersView : UserControl
{
    public DuplicateFoldersView() => InitializeComponent();

    private async void OnGroupsSorting(object sender, DataGridSortingEventArgs e)
    {
        if (DataContext is not DuplicateFoldersViewModel viewModel) return;
        e.Handled = true;
        var field = e.Column.SortMemberPath switch
        {
            "CopyCount" => DuplicateFolderGroupSortField.CopyCount,
            "FileCount" => DuplicateFolderGroupSortField.FileCount,
            "RepresentativePath" => DuplicateFolderGroupSortField.RepresentativePath,
            _ => DuplicateFolderGroupSortField.TotalBytes,
        };
        var direction = e.Column.SortDirection == ListSortDirection.Ascending
            ? ListSortDirection.Descending
            : ListSortDirection.Ascending;
        foreach (var column in GroupsGrid.Columns) column.SortDirection = null;
        e.Column.SortDirection = direction;
        await viewModel.ApplySortAsync(
            field,
            direction == ListSortDirection.Ascending
                ? WorkerSortDirection.Ascending
                : WorkerSortDirection.Descending);
    }
}
