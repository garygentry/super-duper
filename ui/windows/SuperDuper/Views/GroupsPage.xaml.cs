using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.Models;
using SuperDuper.ViewModels;

namespace SuperDuper.Views;

public sealed partial class GroupsPage : Page
{
    public GroupsViewModel ViewModel { get; }

    public GroupsPage()
    {
        this.InitializeComponent();
        ViewModel = App.Services.GetRequiredService<GroupsViewModel>();
    }

    private void SortCombo_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (SortCombo.SelectedItem is ComboBoxItem item && item.Tag is string tag)
            ViewModel.SetSort(tag);
    }

    private void AddFilter_Click(object sender, RoutedEventArgs e)
    {
        // TODO: Open filter flyout
    }

    private void FilterChip_Remove(object sender, RoutedEventArgs e)
    {
        if (sender is Button btn && btn.Tag is FilterChip chip)
            ViewModel.ActiveFilters.Remove(chip);
    }

    private async void AutoSelect_Click(object sender, RoutedEventArgs e)
    {
        await ViewModel.ApplyAutoSelectAsync("newest", 0);
    }

    private async void AutoSelectKeepNewest_Click(object sender, RoutedEventArgs e)
    {
        await ViewModel.ApplyAutoSelectAsync("newest", 0);
    }

    private async void AutoSelectKeepShortest_Click(object sender, RoutedEventArgs e)
    {
        await ViewModel.ApplyAutoSelectAsync("shortest", 0);
    }
}
