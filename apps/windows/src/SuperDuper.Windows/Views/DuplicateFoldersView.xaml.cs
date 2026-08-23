using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Views;

public partial class DuplicateFoldersView : UserControl
{
    internal const int LocationCardFocusAttemptLimit = 8;

    public DuplicateFoldersView() => InitializeComponent();

    private async void OnFolderCardPageClick(object sender, RoutedEventArgs e)
    {
        if (DataContext is not DuplicateFoldersViewModel viewModel)
        {
            return;
        }

        var command = ReferenceEquals(sender, PreviousFolderCardsButton)
            ? viewModel.PreviousMemberPageCommand
            : viewModel.NextMemberPageCommand;
        await command.ExecuteAsync(null);
        await RestoreLocationCardFocusAsync();
    }

    private void OnLocationCardsPreviewKeyDown(object sender, KeyEventArgs e)
    {
        if (MoveLocationCardSelection(e.Key))
        {
            e.Handled = true;
        }
    }

    internal bool MoveLocationCardSelection(Key key)
    {
        if (LocationCards.Items.Count == 0)
        {
            return false;
        }

        var current = Math.Max(0, LocationCards.SelectedIndex);
        var next = key switch
        {
            Key.Left => Math.Max(0, current - 1),
            Key.Right => Math.Min(LocationCards.Items.Count - 1, current + 1),
            Key.Home => 0,
            Key.End => LocationCards.Items.Count - 1,
            _ => -1,
        };
        if (next < 0)
        {
            return false;
        }

        LocationCards.SelectedIndex = next;
        return FocusSelectedLocationCard();
    }

    internal async Task<bool> RestoreLocationCardFocusAsync()
    {
        for (var attempt = 0; attempt < LocationCardFocusAttemptLimit; attempt++)
        {
            if (await Dispatcher.InvokeAsync(FocusSelectedLocationCard, DispatcherPriority.Background)
                && LocationCards.IsKeyboardFocusWithin)
            {
                return true;
            }
            await Dispatcher.InvokeAsync(static () => { }, DispatcherPriority.ContextIdle);
        }
        return false;
    }

    internal bool FocusSelectedLocationCard()
    {
        if (LocationCards.Items.Count == 0)
        {
            return LocationCards.Focus();
        }
        if (LocationCards.SelectedIndex < 0)
        {
            LocationCards.SelectedIndex = 0;
        }

        var selected = LocationCards.SelectedItem;
        LocationCards.ScrollIntoView(selected);
        LocationCards.UpdateLayout();
        return LocationCards.ItemContainerGenerator.ContainerFromItem(selected) is ListBoxItem item
            ? item.Focus()
            : LocationCards.Focus();
    }

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
        var direction = ServerSortInteraction.NextDirection(
            viewModel.SortField,
            viewModel.SortDirection,
            field);
        foreach (var column in GroupsGrid.Columns) column.SortDirection = null;
        e.Column.SortDirection = ServerSortInteraction.ToListDirection(direction);
        await viewModel.ApplySortAsync(field, direction);
    }
}
