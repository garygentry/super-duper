using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Threading;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Views;

public partial class DuplicateFilesView : UserControl
{
    internal const DispatcherPriority SetNavigationFocusPriority = DispatcherPriority.Background;
    internal static readonly TimeSpan SetNavigationFocusRetryDelay = TimeSpan.FromMilliseconds(50);

    public DuplicateFilesView() => InitializeComponent();

    private async void OnSetNavigationClick(object sender, RoutedEventArgs e)
    {
        if (DataContext is not DuplicateFilesViewModel viewModel)
        {
            return;
        }
        var command = ReferenceEquals(sender, PreviousSetButton)
            ? viewModel.PreviousSetCommand
            : viewModel.NextSetCommand;
        await command.ExecuteAsync(null);
        await Dispatcher.InvokeAsync(RestoreGroupGridFocus, SetNavigationFocusPriority);
        await Task.Delay(SetNavigationFocusRetryDelay);
        await Dispatcher.InvokeAsync(RestoreGroupGridFocus, SetNavigationFocusPriority);
    }

    internal bool RestoreGroupGridFocus()
    {
        var selectedItem = GroupsGrid.SelectedItem;
        if (selectedItem is null)
        {
            return GroupsGrid.Focus();
        }
        GroupsGrid.ScrollIntoView(selectedItem);
        GroupsGrid.UpdateLayout();
        if (GroupsGrid.ItemContainerGenerator.ContainerFromItem(selectedItem) is DataGridRow row)
        {
            if (GroupsGrid.Columns.FirstOrDefault() is { } firstColumn)
            {
                GroupsGrid.CurrentCell = new DataGridCellInfo(selectedItem, firstColumn);
                GroupsGrid.ScrollIntoView(selectedItem, firstColumn);
                GroupsGrid.UpdateLayout();
                if (firstColumn.GetCellContent(row) is { } content
                    && FindVisualParent<DataGridCell>(content) is { } cell
                    && cell.Focus())
                {
                    return true;
                }
            }
            return row.Focus() || GroupsGrid.Focus();
        }
        return GroupsGrid.Focus();
    }

    private static T? FindVisualParent<T>(DependencyObject child)
        where T : DependencyObject
    {
        for (var current = child; current is not null; current = VisualTreeHelper.GetParent(current))
        {
            if (current is T match)
            {
                return match;
            }
        }
        return null;
    }

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
