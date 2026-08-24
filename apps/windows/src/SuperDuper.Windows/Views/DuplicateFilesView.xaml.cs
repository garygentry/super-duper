using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using System.ComponentModel;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Views;

public partial class DuplicateFilesView : UserControl
{
    internal const DispatcherPriority SetNavigationFocusPriority = DispatcherPriority.Background;
    internal const int SetNavigationFocusAttemptLimit = 8;

    private PreferenceRulesViewModel? _preferenceRules;
    private bool _applicationConfirmationWasVisible;
    private bool _reversalConfirmationWasVisible;

    public DuplicateFilesView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        if (_preferenceRules is not null)
        {
            _preferenceRules.PropertyChanged -= OnPreferenceRulesPropertyChanged;
        }
        _preferenceRules = (e.NewValue as DuplicateFilesViewModel)?.PreferenceRules;
        if (_preferenceRules is not null)
        {
            _preferenceRules.PropertyChanged += OnPreferenceRulesPropertyChanged;
        }
    }

    private void OnPreferenceRulesPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (_preferenceRules is null)
        {
            return;
        }
        if (e.PropertyName == nameof(PreferenceRulesViewModel.IsApplicationConfirmationVisible))
        {
            var visible = _preferenceRules.IsApplicationConfirmationVisible;
            if (visible)
            {
                _ = FocusWhenVisibleAsync(PreferenceApplicationConfirmationHeading);
            }
            else if (_applicationConfirmationWasVisible)
            {
                _ = Dispatcher.BeginInvoke(
                    new Action(() => PreferenceApplyRuleButton.Focus()),
                    DispatcherPriority.Input);
            }
            _applicationConfirmationWasVisible = visible;
        }
        else if (e.PropertyName == nameof(PreferenceRulesViewModel.IsReversalConfirmationVisible))
        {
            var visible = _preferenceRules.IsReversalConfirmationVisible;
            if (visible)
            {
                _ = FocusWhenVisibleAsync(PreferenceReversalConfirmationHeading);
            }
            else if (_reversalConfirmationWasVisible)
            {
                _ = Dispatcher.BeginInvoke(
                    new Action(() => PreferenceReverseApplicationButton.Focus()),
                    DispatcherPriority.Input);
            }
            _reversalConfirmationWasVisible = visible;
        }
    }

    private async Task<bool> FocusWhenVisibleAsync(FrameworkElement heading)
    {
        for (var attempt = 0; attempt < SetNavigationFocusAttemptLimit; attempt++)
        {
            await Dispatcher.InvokeAsync(static () => { }, DispatcherPriority.ContextIdle);
            if (!heading.IsVisible || !heading.IsLoaded)
            {
                continue;
            }
            heading.BringIntoView();
            return Keyboard.Focus(heading) is not null;
        }
        return false;
    }

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
        await RestoreGroupGridFocusAsync();
    }

    private async void OnValidateFilePageClick(object sender, RoutedEventArgs e)
    {
        if (DataContext is not DuplicateFilesViewModel viewModel)
        {
            return;
        }
        await ExecuteLiveValidationCommandAsync(
            () => viewModel.ValidateVisiblePageCommand.ExecuteAsync(null));
    }

    internal async Task ExecuteLiveValidationCommandAsync(Func<Task> operation)
    {
        try
        {
            await operation();
        }
        finally
        {
            await RestoreMemberGridFocusAsync();
        }
    }

    internal async Task<bool> RestoreMemberGridFocusAsync()
    {
        for (var attempt = 0; attempt < SetNavigationFocusAttemptLimit; attempt++)
        {
            if (await Dispatcher.InvokeAsync(() => MembersGrid.Focus(), SetNavigationFocusPriority)
                && MembersGrid.IsKeyboardFocusWithin)
            {
                return true;
            }
            await Dispatcher.InvokeAsync(static () => { }, DispatcherPriority.ContextIdle);
        }
        return false;
    }

    internal async Task<bool> RestoreGroupGridFocusAsync()
    {
        for (var attempt = 0; attempt < SetNavigationFocusAttemptLimit; attempt++)
        {
            if (await Dispatcher.InvokeAsync(RestoreGroupGridFocus, SetNavigationFocusPriority)
                && GroupsGrid.IsKeyboardFocusWithin)
            {
                return true;
            }
            await Dispatcher.InvokeAsync(static () => { }, DispatcherPriority.ContextIdle);
        }
        return false;
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
