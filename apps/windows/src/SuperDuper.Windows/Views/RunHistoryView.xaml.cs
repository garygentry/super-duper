using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Views;

public partial class RunHistoryView : UserControl
{
    internal const int FocusAttemptLimit = 8;

    public RunHistoryView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        if (e.OldValue is RunHistoryViewModel oldViewModel)
        {
            oldViewModel.PropertyChanged -= OnViewModelPropertyChanged;
        }
        if (e.NewValue is RunHistoryViewModel newViewModel)
        {
            newViewModel.PropertyChanged += OnViewModelPropertyChanged;
        }
    }

    private void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(RunHistoryViewModel.FocusRequestVersion)
            || sender is not RunHistoryViewModel viewModel)
        {
            return;
        }
        Dispatcher.BeginInvoke(DispatcherPriority.Background, () =>
        {
            if (viewModel.FocusTarget == "warnings")
            {
                RunWarningGrid.Focus();
                Keyboard.Focus(RunWarningGrid);
            }
            else if (viewModel.FocusTarget == "history")
            {
                _ = RestoreHistoryGridFocusAsync();
            }
        });
    }

    internal async Task<bool> RestoreHistoryGridFocusAsync()
    {
        for (var attempt = 0; attempt < FocusAttemptLimit; attempt++)
        {
            if (await Dispatcher.InvokeAsync(RestoreHistoryGridFocus, DispatcherPriority.Background)
                && RunHistoryGrid.IsKeyboardFocusWithin)
            {
                return true;
            }
            await Dispatcher.InvokeAsync(static () => { }, DispatcherPriority.ContextIdle);
        }
        return false;
    }

    internal bool RestoreHistoryGridFocus()
    {
        var selected = RunHistoryGrid.SelectedItem;
        if (selected is null)
        {
            return RunHistoryGrid.Focus();
        }
        RunHistoryGrid.ScrollIntoView(selected);
        RunHistoryGrid.UpdateLayout();
        if (RunHistoryGrid.ItemContainerGenerator.ContainerFromItem(selected) is DataGridRow row)
        {
            if (RunHistoryGrid.Columns.FirstOrDefault() is { } firstColumn)
            {
                RunHistoryGrid.CurrentCell = new DataGridCellInfo(selected, firstColumn);
                RunHistoryGrid.ScrollIntoView(selected, firstColumn);
                RunHistoryGrid.UpdateLayout();
                if (firstColumn.GetCellContent(row) is { } content
                    && FindVisualParent<DataGridCell>(content) is { } cell
                    && cell.Focus())
                {
                    return true;
                }
            }
            return row.Focus() || RunHistoryGrid.Focus();
        }
        return RunHistoryGrid.Focus();
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

    private async void OnWarningsSorting(object sender, DataGridSortingEventArgs e)
    {
        if (DataContext is not RunHistoryViewModel viewModel)
        {
            return;
        }
        e.Handled = true;
        var field = e.Column.SortMemberPath switch
        {
            "Phase" => RunWarningSortField.Phase,
            "Message" => RunWarningSortField.Message,
            _ => RunWarningSortField.OccurrenceCount,
        };
        var direction = ServerSortInteraction.NextDirection(
            viewModel.WarningSortField,
            viewModel.WarningSortDirection,
            field);
        foreach (var column in RunWarningGrid.Columns)
        {
            column.SortDirection = null;
        }
        e.Column.SortDirection = ServerSortInteraction.ToListDirection(direction);
        await viewModel.ApplyWarningSortAsync(field, direction);
    }
}
