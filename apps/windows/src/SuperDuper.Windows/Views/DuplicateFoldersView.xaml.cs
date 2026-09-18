using System.ComponentModel;
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
    internal const double NarrowWorkspaceWidth = 960;
    internal const double NarrowWorkspaceHeight = 500;
    internal const double NarrowSelectedDetailMinimumHeight = 80;

    private DuplicateFoldersViewModel? _model;
    private bool _isNarrow;
    private bool _showNarrowDetail;
    private bool _syncingSort;
    private GridLength _wideSetWidth = new(36, GridUnitType.Star);
    private GridLength _wideDetailWidth = new(64, GridUnitType.Star);

    public DuplicateFoldersView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        if (_model is not null)
        {
            _model.PropertyChanged -= OnModelPropertyChanged;
        }
        _model = e.NewValue as DuplicateFoldersViewModel;
        if (_model is not null)
        {
            _model.PropertyChanged += OnModelPropertyChanged;
        }
        _showNarrowDetail = false;
        UpdateResponsiveLayout(ActualWidth, ActualHeight);
        UpdateSortIndicator();
    }

    private void OnModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(DuplicateFoldersViewModel.SortField)
            or nameof(DuplicateFoldersViewModel.SortDirection))
        {
            UpdateSortIndicator();
        }
    }

    private void UpdateSortIndicator()
    {
        if (_model is null || FolderSetSort is null)
        {
            return;
        }
        var tag = $"{_model.SortField}:{_model.SortDirection}";
        var selected = FolderSetSort.Items.OfType<ComboBoxItem>()
            .FirstOrDefault(item => string.Equals(item.Tag?.ToString(), tag, StringComparison.Ordinal));
        if (selected is null || ReferenceEquals(selected, FolderSetSort.SelectedItem))
        {
            return;
        }
        _syncingSort = true;
        FolderSetSort.SelectedItem = selected;
        _syncingSort = false;
    }

    private void OnWorkspaceSizeChanged(object sender, SizeChangedEventArgs e) =>
        UpdateResponsiveLayout(e.NewSize.Width, e.NewSize.Height);

    private void UpdateResponsiveLayout(double width, double height)
    {
        if (FolderSetPaneColumn is null || width <= 0)
        {
            return;
        }
        var narrow = width < NarrowWorkspaceWidth
            || (height > 0 && height < NarrowWorkspaceHeight);
        if (narrow && !_isNarrow)
        {
            _wideSetWidth = FolderSetPaneColumn.Width;
            _wideDetailWidth = FolderDetailPaneColumn.Width;
        }
        _isNarrow = narrow;
        if (!narrow)
        {
            FolderSetPaneHeading.Visibility = Visibility.Visible;
            FolderReviewDetails.Visibility = Visibility.Visible;
            FolderSetPane.Visibility = Visibility.Visible;
            FolderDetailPane.Visibility = Visibility.Visible;
            FolderComparisonSplitter.Visibility = Visibility.Visible;
            CompareSelectedFolderSetButton.Visibility = Visibility.Collapsed;
            BackToFolderSetsButton.Visibility = Visibility.Collapsed;
            FolderSetPaneColumn.MinWidth = 280;
            FolderDetailPaneColumn.MinWidth = 360;
            FolderSetPaneColumn.Width = _wideSetWidth;
            FolderComparisonSplitterColumn.Width = new GridLength(12);
            FolderDetailPaneColumn.Width = _wideDetailWidth;
            UpdateMemberPresentation();
            return;
        }

        FolderSetPaneHeading.Visibility = Visibility.Collapsed;
        FolderReviewDetails.Visibility = Visibility.Collapsed;
        FolderSetPaneColumn.MinWidth = 0;
        FolderDetailPaneColumn.MinWidth = 0;
        FolderComparisonSplitter.Visibility = Visibility.Collapsed;
        FolderComparisonSplitterColumn.Width = new GridLength(0);
        CompareSelectedFolderSetButton.Visibility = _showNarrowDetail ? Visibility.Collapsed : Visibility.Visible;
        BackToFolderSetsButton.Visibility = _showNarrowDetail ? Visibility.Visible : Visibility.Collapsed;
        FolderSetPane.Visibility = _showNarrowDetail ? Visibility.Collapsed : Visibility.Visible;
        FolderDetailPane.Visibility = _showNarrowDetail ? Visibility.Visible : Visibility.Collapsed;
        FolderSetPaneColumn.Width = _showNarrowDetail ? new GridLength(0) : new GridLength(1, GridUnitType.Star);
        FolderDetailPaneColumn.Width = _showNarrowDetail ? new GridLength(1, GridUnitType.Star) : new GridLength(0);
        UpdateMemberPresentation();
    }

    private void UpdateMemberPresentation()
    {
        if (LocationCards is null || SelectedFolderCopyPanel is null || BackToFolderCopiesButton is null)
        {
            return;
        }
        var selectedCopyDetail = _isNarrow && _showNarrowDetail && _model?.SelectedMember is not null;
        FolderDetailPane.MinHeight = selectedCopyDetail ? NarrowSelectedDetailMinimumHeight : 0;
        FolderDetailHeader.Visibility = selectedCopyDetail ? Visibility.Collapsed : Visibility.Visible;
        LocationCards.Visibility = selectedCopyDetail ? Visibility.Collapsed : Visibility.Visible;
        BackToFolderCopiesButton.Visibility = selectedCopyDetail ? Visibility.Visible : Visibility.Collapsed;
        NarrowFolderAlerts.Visibility = selectedCopyDetail ? Visibility.Visible : Visibility.Collapsed;
        FolderDetailCommandRegion.Visibility = selectedCopyDetail ? Visibility.Collapsed : Visibility.Visible;
        FolderDetailHeaderRow.Height = selectedCopyDetail ? new GridLength(0) : GridLength.Auto;
        FolderMemberListRow.Height = selectedCopyDetail ? new GridLength(0) : new GridLength(1, GridUnitType.Star);
        FolderSelectedCopyRow.Height = selectedCopyDetail ? new GridLength(1, GridUnitType.Star) : GridLength.Auto;
        FolderDetailCommandRow.Height = selectedCopyDetail ? new GridLength(0) : GridLength.Auto;
        SelectedFolderCopyPanel.Margin = selectedCopyDetail ? new Thickness(0) : new Thickness(0, 6, 0, 0);
        SelectedFolderCopyPanel.Padding = selectedCopyDetail ? new Thickness(2) : new Thickness(10);
        SelectedFolderCopyPanel.MaxHeight = selectedCopyDetail ? double.PositiveInfinity : 148;
    }

    private async void OnCompareSelectedSetClick(object sender, RoutedEventArgs e)
    {
        if (!_isNarrow || _model?.SelectedGroup is null)
        {
            return;
        }
        _model.SelectedMember = null;
        _showNarrowDetail = true;
        UpdateResponsiveLayout(ActualWidth, ActualHeight);
        FolderWorkspaceRoot.UpdateLayout();
        await FocusWhenVisibleAsync(SelectedFolderSetHeading);
    }

    private void OnFolderSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        UpdateMemberPresentation();
        if (_isNarrow && _showNarrowDetail && _model?.SelectedMember is not null)
        {
            _ = Dispatcher.BeginInvoke(
                new Action(SelectedFolderCopyScrollViewer.ScrollToTop),
                DispatcherPriority.ContextIdle);
        }
    }

    private async void OnBackToCopiesClick(object sender, RoutedEventArgs e)
    {
        if (!_isNarrow || _model is null)
        {
            return;
        }
        _model.SelectedMember = null;
        UpdateMemberPresentation();
        await RestoreLocationCardFocusAsync();
    }

    private async void OnBackToSetsClick(object sender, RoutedEventArgs e)
    {
        if (!_isNarrow)
        {
            return;
        }
        _showNarrowDetail = false;
        UpdateResponsiveLayout(ActualWidth, ActualHeight);
        await RestoreGroupGridFocusAsync();
    }

    private async void OnReviewDecisionClick(object sender, RoutedEventArgs e)
    {
        if (sender is not Button button || _model is not { } model) return;
        var runId = model.Run?.Id;
        var groupId = model.SelectedGroup?.Id;
        var memberId = model.SelectedMember?.Id;
        await DecisionActionFocus.PreserveAsync(button, FolderDetailPane,
            () => ReferenceEquals(_model, model) && model.Run?.Id == runId
                && model.SelectedGroup?.Id == groupId && model.SelectedMember?.Id == memberId);
    }

    private async void OnSetSortChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_syncingSort || _model is null || FolderSetSort.SelectedItem is not ComboBoxItem item
            || item.Tag?.ToString()?.Split(':') is not [var fieldText, var directionText]
            || !Enum.TryParse(fieldText, out DuplicateFolderGroupSortField field)
            || !Enum.TryParse(directionText, out WorkerSortDirection direction)
            || (_model.SortField == field && _model.SortDirection == direction))
        {
            return;
        }
        await _model.ApplySortAsync(field, direction);
    }

    private async void OnFilterKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key != Key.Enter || _model is null)
        {
            return;
        }
        e.Handled = true;
        await _model.ApplyFiltersCommand.ExecuteAsync(null);
    }

    private async void OnFolderGroupPageClick(object sender, RoutedEventArgs e)
    {
        if (_model is null)
        {
            return;
        }
        var command = ReferenceEquals(sender, PreviousFolderGroupPageButton)
            ? _model.PreviousPageCommand
            : _model.NextPageCommand;
        await command.ExecuteAsync(null);
        await RestoreGroupGridFocusAsync();
    }

    private async void OnFolderCardPageClick(object sender, RoutedEventArgs e)
    {
        if (_model is null)
        {
            return;
        }
        var command = ReferenceEquals(sender, PreviousFolderCardsButton)
            ? _model.PreviousMemberPageCommand
            : _model.NextMemberPageCommand;
        await command.ExecuteAsync(null);
        await RestoreLocationCardFocusAsync();
    }

    private async void OnSelectFolderPageInExplorerClick(object sender, RoutedEventArgs e) =>
        await SelectCurrentPageInExplorerAsync();

    internal Task SelectCurrentPageInExplorerAsync() =>
        _model is null
            ? Task.CompletedTask
            : ExecuteExplorerCommandAsync(() => _model.SelectPageInExplorerCommand.ExecuteAsync(null));

    private async void OnLocationCardsPreviewKeyDown(object sender, KeyEventArgs e)
    {
        if (IsRevealShortcut(e.Key, e.SystemKey, Keyboard.Modifiers))
        {
            e.Handled = true;
            await RevealSelectedLocationAsync();
            return;
        }

        if (IsSelectPageShortcut(e.Key, e.SystemKey, Keyboard.Modifiers))
        {
            e.Handled = true;
            await SelectCurrentPageInExplorerAsync();
        }
    }

    internal static bool IsRevealShortcut(Key key, Key systemKey, ModifierKeys modifiers) =>
        (key == Key.System ? systemKey : key) == Key.E
        && modifiers.HasFlag(ModifierKeys.Alt);

    // The page-selection action is icon-only, so it carries no access text; it still advertises
    // Alt+G through AutomationProperties.AccessKey and its help text, so implement that here.
    internal static bool IsSelectPageShortcut(Key key, Key systemKey, ModifierKeys modifiers) =>
        (key == Key.System ? systemKey : key) == Key.G
        && modifiers.HasFlag(ModifierKeys.Alt);

    private async void OnRevealInExplorerClick(object sender, RoutedEventArgs e)
    {
        if (sender is not FrameworkElement { DataContext: DuplicateFolderMemberListItemViewModel member })
        {
            return;
        }
        LocationCards.SelectedItem = member;
        await RevealLocationAsync(member);
    }

    internal Task RevealSelectedLocationAsync() =>
        LocationCards.SelectedItem is DuplicateFolderMemberListItemViewModel member
            ? RevealLocationAsync(member)
            : Task.CompletedTask;

    private Task RevealLocationAsync(DuplicateFolderMemberListItemViewModel member) =>
        _model is null
            ? Task.CompletedTask
            : ExecuteExplorerCommandAsync(() => _model.RevealInExplorerCommand.ExecuteAsync(member));

    internal async Task ExecuteExplorerCommandAsync(Func<Task> operation)
    {
        try
        {
            await operation();
        }
        finally
        {
            await RestoreLocationCardFocusAsync();
        }
    }

    internal async Task<bool> RestoreLocationCardFocusAsync()
    {
        for (var attempt = 0; attempt < LocationCardFocusAttemptLimit; attempt++)
        {
            var focused = await Dispatcher.InvokeAsync(
                () =>
                {
                    if (_isNarrow && _showNarrowDetail && _model?.SelectedMember is not null)
                    {
                        return BackToFolderCopiesButton.Focus() && BackToFolderCopiesButton.IsKeyboardFocused;
                    }
                    return FocusSelectedLocationCard();
                },
                DispatcherPriority.Background);
            if (focused)
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
        LocationCards.ScrollIntoView(LocationCards.SelectedItem ?? LocationCards.Items[0]);
        LocationCards.UpdateLayout();
        return LocationCards.Focus();
    }

    internal async Task<bool> RestoreGroupGridFocusAsync()
    {
        for (var attempt = 0; attempt < LocationCardFocusAttemptLimit; attempt++)
        {
            var focused = await Dispatcher.InvokeAsync(
                () =>
                {
                    GroupsGrid.ScrollIntoView(GroupsGrid.SelectedItem ?? GroupsGrid.Items.Cast<object>().FirstOrDefault());
                    GroupsGrid.UpdateLayout();
                    return GroupsGrid.Focus() && GroupsGrid.IsKeyboardFocusWithin;
                },
                DispatcherPriority.Background);
            if (focused)
            {
                return true;
            }
            await Dispatcher.InvokeAsync(static () => { }, DispatcherPriority.ContextIdle);
        }
        return false;
    }

    private async Task<bool> FocusWhenVisibleAsync(FrameworkElement target)
    {
        for (var attempt = 0; attempt < LocationCardFocusAttemptLimit; attempt++)
        {
            var focused = await Dispatcher.InvokeAsync(() =>
            {
                if (!target.IsVisible || !target.IsLoaded)
                {
                    return false;
                }
                target.BringIntoView();
                return Keyboard.Focus(target) is not null;
            }, DispatcherPriority.ContextIdle);
            if (focused)
            {
                return true;
            }
        }
        return false;
    }
}
