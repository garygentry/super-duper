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
        var direction = ServerSortInteraction.NextDirection(
            viewModel.SortField,
            viewModel.SortDirection,
            field);
        foreach (var column in GroupsGrid.Columns)
        {
            column.SortDirection = null;
        }
        e.Column.SortDirection = ServerSortInteraction.ToListDirection(direction);
        await viewModel.ApplySortAsync(field, direction);
    }
}
