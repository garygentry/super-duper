using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using SuperDuper.Models;
using SuperDuper.Services;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class GroupsPage : Page
{
    public GroupsViewModel ViewModel { get; }

    public GroupsPage()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);
        ViewModel = App.Services.GetRequiredService<GroupsViewModel>();
        this.DataContext = this;

        // Wire events (XAML compiler pass 2 doesn't generate IComponentConnector)
        SortCombo.SelectionChanged += SortCombo_SelectionChanged;

        // SplitButton and MenuFlyoutItems aren't in the visual tree (FindName can't find them).
        // Wire them after Loaded when the flyout items are accessible.
        this.Loaded += (_, _) => WireNonVisualTreeEvents();

        // React to session changes while this page is alive
        var scanService = App.Services.GetRequiredService<ScanService>();
        scanService.ActiveSessionChanged += (_, sessionId) =>
        {
            if (sessionId.HasValue)
                _ = ViewModel.LoadInitialAsync(sessionId.Value);
        };
    }

    private void WireNonVisualTreeEvents()
    {
        // Auto-select button
        if (FindName("AutoSelectButton") is SplitButton autoBtn)
            autoBtn.Click += (s, e) => AutoSelect_Click(s, new RoutedEventArgs());

        // Wire flyout items by walking the MenuFlyout Items collection
        if (FindName("AutoSelectNewest") is MenuFlyoutItem newest)
            newest.Click += AutoSelectKeepNewest_Click;
        if (FindName("AutoSelectShortest") is MenuFlyoutItem shortest)
            shortest.Click += AutoSelectKeepShortest_Click;

        // Filter flyout items
        var filterNames = new[]
        {
            "FilterImages", "FilterDocuments", "FilterVideo", "FilterAudio", "FilterArchives"
        };
        foreach (var name in filterNames)
        {
            if (FindName(name) is MenuFlyoutItem item)
                item.Click += AddFileTypeFilter_Click;
        }

        var statusNames = new[] { "FilterUnreviewed", "FilterPartial", "FilterDecided" };
        foreach (var name in statusNames)
        {
            if (FindName(name) is MenuFlyoutItem item)
                item.Click += AddStatusFilter_Click;
        }
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        var ss = App.Services.GetRequiredService<ScanService>();
        var sessionId = ss.ActiveSessionId ?? 0;

        if (e.Parameter is string searchText && !string.IsNullOrWhiteSpace(searchText))
        {
            ViewModel.ActiveFilters.Add(new FilterChip
            {
                FilterType = FilterType.TextSearch,
                DisplayLabel = $"Search: {searchText}",
                Value = searchText
            });
        }

        _ = ViewModel.LoadInitialAsync(sessionId);
    }

    private void SortCombo_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (SortCombo.SelectedItem is ComboBoxItem item && item.Tag is string tag)
            ViewModel.SetSort(tag);
    }

    private void FilterChip_Remove(object sender, RoutedEventArgs e)
    {
        if (sender is Button btn && btn.Tag is FilterChip chip)
        {
            ViewModel.ActiveFilters.Remove(chip);
            _ = ViewModel.ApplyFiltersAsync();
        }
    }

    private void AddFileTypeFilter_Click(object sender, RoutedEventArgs e)
    {
        if (sender is MenuFlyoutItem item && item.Tag is string tag)
        {
            // Remove existing file type filter
            var existing = ViewModel.ActiveFilters.FirstOrDefault(f => f.FilterType == FilterType.FileType);
            if (existing != null) ViewModel.ActiveFilters.Remove(existing);

            ViewModel.ActiveFilters.Add(new FilterChip
            {
                FilterType = FilterType.FileType,
                DisplayLabel = $"Type: {item.Text}",
                Value = tag
            });
            _ = ViewModel.ApplyFiltersAsync();
        }
    }

    private void AddStatusFilter_Click(object sender, RoutedEventArgs e)
    {
        if (sender is MenuFlyoutItem item && item.Tag is string tag)
        {
            // Remove existing status filter
            var existing = ViewModel.ActiveFilters.FirstOrDefault(f => f.FilterType == FilterType.ReviewStatus);
            if (existing != null) ViewModel.ActiveFilters.Remove(existing);

            ViewModel.ActiveFilters.Add(new FilterChip
            {
                FilterType = FilterType.ReviewStatus,
                DisplayLabel = $"Status: {item.Text}",
                Value = tag
            });
            _ = ViewModel.ApplyFiltersAsync();
        }
    }

    private long GetActiveSessionId()
        => App.Services.GetRequiredService<ScanService>().ActiveSessionId ?? 0;

    private async void AutoSelect_Click(object sender, RoutedEventArgs e)
    {
        await ViewModel.ApplyAutoSelectAsync("newest", GetActiveSessionId());
    }

    private async void AutoSelectKeepNewest_Click(object sender, RoutedEventArgs e)
    {
        await ViewModel.ApplyAutoSelectAsync("newest", GetActiveSessionId());
    }

    private async void AutoSelectKeepShortest_Click(object sender, RoutedEventArgs e)
    {
        await ViewModel.ApplyAutoSelectAsync("shortest", GetActiveSessionId());
    }
}
