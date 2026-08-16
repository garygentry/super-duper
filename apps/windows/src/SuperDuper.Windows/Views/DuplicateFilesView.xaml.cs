using System.ComponentModel;
using System.Windows.Controls;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Views;

public partial class DuplicateFilesView : UserControl
{
    public DuplicateFilesView() => InitializeComponent();

    private async void OnGroupsSorting(object sender, DataGridSortingEventArgs e)
    {
        if (DataContext is not DuplicateFilesViewModel viewModel)
        {
            return;
        }
        e.Handled = true;
        var field = e.Column.SortMemberPath switch
        {
            "GroupSize" => DuplicateFileGroupSortField.GroupSize,
            "CopyCount" => DuplicateFileGroupSortField.CopyCount,
            "RepresentativeName" => DuplicateFileGroupSortField.RepresentativeName,
            _ => DuplicateFileGroupSortField.RecoverableBytes,
        };
        var direction = e.Column.SortDirection == ListSortDirection.Ascending
            ? ListSortDirection.Descending
            : ListSortDirection.Ascending;
        foreach (var column in GroupsGrid.Columns)
        {
            column.SortDirection = null;
        }
        e.Column.SortDirection = direction;
        await viewModel.ApplySortAsync(
            field,
            direction == ListSortDirection.Ascending
                ? WorkerSortDirection.Ascending
                : WorkerSortDirection.Descending);
    }
}
