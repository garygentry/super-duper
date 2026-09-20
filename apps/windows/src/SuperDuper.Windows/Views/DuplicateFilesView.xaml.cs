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
    internal const double NarrowWorkspaceWidth = 960;
    internal const double NarrowWorkspaceHeight = 500;

    private DuplicateFilesViewModel? _model;
    private bool _isNarrow;
    private bool _showNarrowDetail;
    private bool _syncingSort;
    private GridLength _wideSetWidth = new(36, GridUnitType.Star);
    private GridLength _wideDetailWidth = new(64, GridUnitType.Star);

    public DuplicateFilesView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
        PreviewKeyDown += OnWorkspaceKeyDown;
        Loaded += (_, _) =>
        {
            AccessKeyManager.Register("v", ValidateFilePageButton);
            AccessKeyManager.Register("p", PreviousSetButton);
            AccessKeyManager.Register("n", NextSetButton);
        };
        Unloaded += (_, _) =>
        {
            AccessKeyManager.Unregister("v", ValidateFilePageButton);
            AccessKeyManager.Unregister("p", PreviousSetButton);
            AccessKeyManager.Unregister("n", NextSetButton);
        };
    }

    private void OnWorkspaceKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.F && Keyboard.Modifiers == ModifierKeys.Control)
        {
            FileSearch.Focus();
            FileSearch.SelectAll();
            e.Handled = true;
        }
        else if (e.Key == Key.Escape && FileFiltersExpander.IsExpanded
            && FileFiltersExpander.IsKeyboardFocusWithin)
        {
            FileFiltersExpander.IsExpanded = false;
            FileFiltersToggle.Focus();
            e.Handled = true;
        }
    }

    private async void OnFilterEditorKeyDown(object sender, KeyEventArgs e)
    {
        // Enter in rule controls retains the rule's own meaning.
        if (e.Key != Key.Enter || Keyboard.Modifiers != ModifierKeys.None
            || e.OriginalSource is not TextBox field
            || (field != FileSearch && field != FileMinimumSize && field != FileExtension)
            || DataContext is not DuplicateFilesViewModel model
            || !model.ApplyFiltersCommand.CanExecute(null)) return;
        e.Handled = true;
        await model.ApplyFiltersCommand.ExecuteAsync(null);
    }

    private void OnFiltersCollapsed(object sender, RoutedEventArgs e)
    {
        if (ReferenceEquals(e.OriginalSource, FileFiltersExpander) && FileFiltersExpander.IsKeyboardFocusWithin)
            FileFiltersToggle.Focus();
    }

    private async void OnRemoveFilterClick(object sender, RoutedEventArgs e)
    {
        if (sender is not Button { Tag: FileFilterChip chip } button
            || DataContext is not DuplicateFilesViewModel model
            || !model.RemoveFilterCommand.CanExecute(chip)) return;
        // Move before removing the focused chip, never on asynchronous query completion.
        if (button.IsKeyboardFocusWithin) FileSearch.Focus();
        await model.RemoveFilterCommand.ExecuteAsync(chip);
    }

    private void OnDataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        if (_model is not null) _model.PropertyChanged -= OnQueryPropertyChanged;
        _model = e.NewValue as DuplicateFilesViewModel;
        if (_model is not null) _model.PropertyChanged += OnQueryPropertyChanged;
        _showNarrowDetail = false;
        UpdateResponsiveLayout(ActualWidth, ActualHeight);
        UpdateSortIndicators();
    }

    private void OnQueryPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(DuplicateFilesViewModel.SortField) or nameof(DuplicateFilesViewModel.SortDirection))
            UpdateSortIndicators();
        else if (e.PropertyName == nameof(DuplicateFilesViewModel.Groups))
            // DataGrid clears sort indicators when ItemsSource changes. Restore the current
            // server sort after that binding has run, without touching keyboard focus.
            _ = Dispatcher.BeginInvoke(new Action(UpdateSortIndicators), DispatcherPriority.Loaded);
    }

    private void UpdateSortIndicators()
    {
        if (_model is null) return;
        var path = _model.SortField switch
        {
            DuplicateFileGroupSortField.GroupSize => "GroupSize",
            DuplicateFileGroupSortField.CopyCount => "CopyCount",
            DuplicateFileGroupSortField.RepresentativeName => "RepresentativeName",
            _ => "RecoverableBytes",
        };
        foreach (var column in GroupsGrid.Columns)
            column.SortDirection = column.SortMemberPath == path
                ? ServerSortInteraction.ToListDirection(_model.SortDirection) : null;
        var tag = $"{path}:{_model.SortDirection}";
        var selected = FileSetSort.Items.OfType<ComboBoxItem>()
            .FirstOrDefault(item => string.Equals(item.Tag?.ToString(), tag, StringComparison.Ordinal));
        if (selected is null || ReferenceEquals(FileSetSort.SelectedItem, selected)) return;
        _syncingSort = true;
        FileSetSort.SelectedItem = selected;
        _syncingSort = false;
    }

    private void OnWorkspaceSizeChanged(object sender, SizeChangedEventArgs e) =>
        UpdateResponsiveLayout(e.NewSize.Width, e.NewSize.Height);

    private void UpdateResponsiveLayout(double width, double height)
    {
        if (SetPaneColumn is null || width <= 0) return;
        var narrow = width < NarrowWorkspaceWidth
            || (height > 0 && height < NarrowWorkspaceHeight);
        if (narrow && !_isNarrow)
        {
            _wideSetWidth = SetPaneColumn.Width;
            _wideDetailWidth = DetailPaneColumn.Width;
        }
        _isNarrow = narrow;
        if (!narrow)
        {
            SetPaneHeading.Visibility = Visibility.Visible;
            DetailContextSummary.Visibility = Visibility.Visible;
            SetPane.Visibility = Visibility.Visible;
            DetailPane.Visibility = Visibility.Visible;
            ComparisonSplitter.Visibility = Visibility.Visible;
            CompareSelectedSetButton.Visibility = Visibility.Collapsed;
            BackToSetsButton.Visibility = Visibility.Collapsed;
            SetPaneColumn.MinWidth = 280;
            DetailPaneColumn.MinWidth = 360;
            SetPaneColumn.Width = _wideSetWidth;
            ComparisonSplitterColumn.Width = new GridLength(12);
            DetailPaneColumn.Width = _wideDetailWidth;
            UpdateMemberPresentation();
            return;
        }

        SetPaneHeading.Visibility = Visibility.Collapsed;
        DetailContextSummary.Visibility = Visibility.Collapsed;
        SetPaneColumn.MinWidth = 0;
        DetailPaneColumn.MinWidth = 0;
        ComparisonSplitter.Visibility = Visibility.Collapsed;
        ComparisonSplitterColumn.Width = new GridLength(0);
        CompareSelectedSetButton.Visibility = _showNarrowDetail ? Visibility.Collapsed : Visibility.Visible;
        BackToSetsButton.Visibility = _showNarrowDetail ? Visibility.Visible : Visibility.Collapsed;
        SetPane.Visibility = _showNarrowDetail ? Visibility.Collapsed : Visibility.Visible;
        DetailPane.Visibility = _showNarrowDetail ? Visibility.Visible : Visibility.Collapsed;
        SetPaneColumn.Width = _showNarrowDetail ? new GridLength(0) : new GridLength(1, GridUnitType.Star);
        DetailPaneColumn.Width = _showNarrowDetail ? new GridLength(1, GridUnitType.Star) : new GridLength(0);
        UpdateMemberPresentation();
    }

    private void UpdateMemberPresentation()
    {
        if (MembersGrid is null || SelectedCopyPanel is null || BackToCopiesButton is null) return;
        var selectedCopyDetail = _isNarrow && _showNarrowDetail && _model?.SelectedMember is not null;
        MembersGrid.Visibility = selectedCopyDetail ? Visibility.Collapsed : Visibility.Visible;
        BackToCopiesButton.Visibility = selectedCopyDetail ? Visibility.Visible : Visibility.Collapsed;
        NarrowFileAlerts.Visibility = selectedCopyDetail ? Visibility.Visible : Visibility.Collapsed;
        DetailCommandRegion.Visibility = selectedCopyDetail ? Visibility.Collapsed : Visibility.Visible;
        Grid.SetRow(SelectedCopyPanel, selectedCopyDetail ? 1 : 2);
        SelectedCopyPanel.Margin = selectedCopyDetail ? new Thickness(0) : new Thickness(0, 6, 0, 0);
        SelectedCopyPanel.Padding = selectedCopyDetail ? new Thickness(2) : new Thickness(10);
        SelectedCopyPanel.MaxHeight = selectedCopyDetail ? double.PositiveInfinity : 148;
    }

    private async void OnReviewDecisionClick(object sender, RoutedEventArgs e)
    {
        if (sender is not Button button || _model is not { } model) return;
        var runId = model.Run?.Id;
        var groupId = model.SelectedGroup?.Id;
        var memberId = model.SelectedMember?.Id;
        await DecisionActionFocus.PreserveAsync(button, SelectedSetHeading,
            () => ReferenceEquals(_model, model) && model.Run?.Id == runId
                && model.SelectedGroup?.Id == groupId && model.SelectedMember?.Id == memberId);
    }

    private async void OnCompareSelectedSetClick(object sender, RoutedEventArgs e)
    {
        if (!_isNarrow || _model?.SelectedGroup is null) return;
        _model.SelectedMember = null;
        _showNarrowDetail = true;
        UpdateResponsiveLayout(ActualWidth, ActualHeight);
        await FocusWhenVisibleAsync(SelectedSetHeading);
    }

    private void OnMemberSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        UpdateMemberPresentation();
        if (_isNarrow && _showNarrowDetail && _model?.SelectedMember is not null)
            _ = Dispatcher.BeginInvoke(
                new Action(SelectedCopyScrollViewer.ScrollToTop),
                DispatcherPriority.ContextIdle);
    }

    private async void OnBackToCopiesClick(object sender, RoutedEventArgs e)
    {
        if (!_isNarrow || _model is null) return;
        _model.SelectedMember = null;
        UpdateMemberPresentation();
        await RestoreMemberGridFocusAsync();
    }

    private async void OnBackToSetsClick(object sender, RoutedEventArgs e)
    {
        if (!_isNarrow) return;
        _showNarrowDetail = false;
        UpdateResponsiveLayout(ActualWidth, ActualHeight);
        await RestoreGroupGridFocusAsync();
    }

    private async void OnSetSortChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_syncingSort || _model is null || FileSetSort.SelectedItem is not ComboBoxItem item
            || item.Tag?.ToString()?.Split(':') is not [var fieldText, var directionText]
            || !Enum.TryParse(fieldText, out DuplicateFileGroupSortField field)
            || !Enum.TryParse(directionText, out WorkerSortDirection direction)
            || (_model.SortField == field && _model.SortDirection == direction)) return;
        await _model.ApplySortAsync(field, direction);
    }

    private async Task<bool> FocusWhenVisibleAsync(FrameworkElement heading)
    {
        for (var attempt = 0; attempt < SetNavigationFocusAttemptLimit; attempt++)
        {
            var focused = await Dispatcher.InvokeAsync(() =>
            {
                if (!heading.IsVisible || !heading.IsLoaded) return false;
                heading.BringIntoView();
                return Keyboard.Focus(heading) is not null;
            }, DispatcherPriority.ContextIdle);
            if (focused) return true;
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
        if (_isNarrow && _showNarrowDetail)
            await FocusWhenVisibleAsync(SelectedSetHeading);
        else
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

    private async void OnReconcileDirtyRootClick(object sender, RoutedEventArgs e)
    {
        if (DataContext is not DuplicateFilesViewModel viewModel)
        {
            return;
        }
        await ExecuteDirtyRootReconciliationCommandAsync(
            () => viewModel.ReconcileDirtyRootCommand.ExecuteAsync(null));
    }

    internal Task ExecuteDirtyRootReconciliationCommandAsync(Func<Task> operation) =>
        ExecuteLiveValidationCommandAsync(operation);

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
            if (await Dispatcher.InvokeAsync(
                    () =>
                    {
                        if (_isNarrow && _showNarrowDetail && _model?.SelectedMember is not null)
                            return BackToCopiesButton.Focus() && BackToCopiesButton.IsKeyboardFocused;
                        return MembersGrid.Focus() && MembersGrid.IsKeyboardFocusWithin;
                    },
                    SetNavigationFocusPriority))
            {
                return true;
            }
            await Dispatcher.InvokeAsync(static () => { }, DispatcherPriority.ContextIdle);
        }
        return false;
    }

    internal async Task<bool> RestoreGroupGridFocusAsync(Func<bool>? isCurrent = null)
    {
        for (var attempt = 0; attempt < SetNavigationFocusAttemptLimit; attempt++)
        {
            if (isCurrent?.Invoke() == false) return false;
            if (await Dispatcher.InvokeAsync(
                    () => isCurrent?.Invoke() != false
                        && RestoreGroupGridFocus()
                        && GroupsGrid.IsKeyboardFocusWithin,
                    SetNavigationFocusPriority))
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
