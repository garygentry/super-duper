using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using SuperDuper.Windows.Core.ViewModels;

namespace SuperDuper.Windows.Views;

public partial class LocationPreferencesView : UserControl
{
    private PreferenceRulesViewModel? _viewModel;
    private bool _applicationConfirmationWasVisible;
    private bool _reversalConfirmationWasVisible;

    public LocationPreferencesView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        if (_viewModel is not null)
        {
            _viewModel.PropertyChanged -= OnViewModelPropertyChanged;
        }
        _viewModel = e.NewValue as PreferenceRulesViewModel;
        if (_viewModel is not null)
        {
            _viewModel.PropertyChanged += OnViewModelPropertyChanged;
        }
        _applicationConfirmationWasVisible = _viewModel?.IsApplicationConfirmationVisible == true;
        _reversalConfirmationWasVisible = _viewModel?.IsReversalConfirmationVisible == true;
    }

    private void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (_viewModel is null)
        {
            return;
        }
        if (e.PropertyName == nameof(PreferenceRulesViewModel.IsApplicationConfirmationVisible))
        {
            RestoreConfirmationFocus(
                _viewModel.IsApplicationConfirmationVisible,
                _applicationConfirmationWasVisible,
                PreferenceApplicationConfirmationHeading,
                PreferenceApplyRuleButton);
            _applicationConfirmationWasVisible = _viewModel.IsApplicationConfirmationVisible;
        }
        else if (e.PropertyName == nameof(PreferenceRulesViewModel.IsReversalConfirmationVisible))
        {
            RestoreConfirmationFocus(
                _viewModel.IsReversalConfirmationVisible,
                _reversalConfirmationWasVisible,
                PreferenceReversalConfirmationHeading,
                PreferenceReverseApplicationButton);
            _reversalConfirmationWasVisible = _viewModel.IsReversalConfirmationVisible;
        }
    }

    private void RestoreConfirmationFocus(
        bool isVisible,
        bool wasVisible,
        FrameworkElement heading,
        FrameworkElement returnTarget)
    {
        if (isVisible)
        {
            _ = Dispatcher.BeginInvoke(() =>
            {
                heading.BringIntoView();
                Keyboard.Focus(heading);
            }, DispatcherPriority.ContextIdle);
        }
        else if (wasVisible)
        {
            _ = Dispatcher.BeginInvoke(() =>
            {
                returnTarget.BringIntoView();
                Keyboard.Focus(returnTarget);
            }, DispatcherPriority.Input);
        }
    }
}
