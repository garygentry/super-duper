using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.Models;
using SuperDuper.ViewModels;

namespace SuperDuper.Controls;

public sealed partial class DuplicateCard : UserControl
{
    public DuplicateCardViewModel? ViewModel { get; private set; }

    public DuplicateCard()
    {
        this.InitializeComponent();
    }

    public void Bind(DuplicateCardViewModel vm)
    {
        ViewModel = vm;

        // Drive color header bar
        DriveColorBar.Background = vm.DriveColorBrush;

        // Path
        PathText.Text = vm.CanonicalPath;
        AutomationProperties.SetName(KeepButton, $"Keep copy at {System.IO.Path.GetFileName(vm.CanonicalPath)}");
        AutomationProperties.SetName(DeleteButton, $"Delete copy at {System.IO.Path.GetFileName(vm.CanonicalPath)}");
        AutomationProperties.SetName(SkipButton, $"Skip {System.IO.Path.GetFileName(vm.CanonicalPath)}");

        // Age badges
        NewestBadge.Visibility = vm.IsNewest ? Visibility.Visible : Visibility.Collapsed;
        OldestBadge.Visibility = vm.IsOldest ? Visibility.Visible : Visibility.Collapsed;

        // Dates
        ModifiedText.Text = vm.LastModified;
        CreatedText.Text = vm.Created;
        AccessedText.Text = vm.Accessed;

        // Heuristic label
        HeuristicText.Text = vm.HeuristicLabel ?? "";
        HeuristicText.Visibility = string.IsNullOrEmpty(vm.HeuristicLabel)
            ? Visibility.Collapsed : Visibility.Visible;

        // Siblings
        SiblingsText.Text = vm.SiblingsText ?? "";
        SiblingsText.Visibility = string.IsNullOrEmpty(vm.SiblingsText)
            ? Visibility.Collapsed : Visibility.Visible;

        // Decision state
        vm.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(DuplicateCardViewModel.CurrentDecision))
                UpdateDecisionVisuals();
        };
        UpdateDecisionVisuals();
    }

    private void UpdateDecisionVisuals()
    {
        if (ViewModel == null) return;

        // Highlight the active action button
        KeepButton.Style = ViewModel.CurrentDecision == ReviewAction.Keep
            ? (Style)Application.Current.Resources["AccentButtonStyle"] : null;
        DeleteButton.Style = ViewModel.CurrentDecision == ReviewAction.Delete
            ? (Style)Application.Current.Resources["AccentButtonStyle"] : null;
        SkipButton.Style = null; // Skip uses default style always
    }

    private void KeepButton_Click(object sender, RoutedEventArgs e) =>
        ViewModel?.SetDecisionCommand.Execute(ReviewAction.Keep);

    private void DeleteButton_Click(object sender, RoutedEventArgs e) =>
        ViewModel?.SetDecisionCommand.Execute(ReviewAction.Delete);

    private void SkipButton_Click(object sender, RoutedEventArgs e) =>
        ViewModel?.SetDecisionCommand.Execute(ReviewAction.Skip);

    private void RevealButton_Click(object sender, RoutedEventArgs e) =>
        ViewModel?.RevealCommand.Execute(null);

    private void OpenButton_Click(object sender, RoutedEventArgs e) =>
        ViewModel?.OpenCommand.Execute(null);
}
