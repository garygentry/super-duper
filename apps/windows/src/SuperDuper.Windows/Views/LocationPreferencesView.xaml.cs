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
    private bool _applicationChangedDuringConfirmation;
    private bool _applicationChangedDuringReversal;
    private bool _updatingStages;

    public LocationPreferencesView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
        PreviewKeyDown += OnPreviewKeyDown;
    }

    private void OnPreviewKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key != Key.Escape || _viewModel is null)
        {
            return;
        }
        var cancel = _viewModel.IsApplicationConfirmationVisible
            ? _viewModel.CancelApplicationCommand
            : _viewModel.IsReversalConfirmationVisible ? _viewModel.CancelReversalCommand : null;
        if (cancel?.CanExecute(null) == true)
        {
            cancel.Execute(null);
            e.Handled = true;
        }
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
        _applicationChangedDuringConfirmation = false;
        _applicationChangedDuringReversal = false;
        OpenStage(_viewModel?.IsApplicationConfirmationVisible == true
            ? PreferencePreviewStage
            : _viewModel?.IsReversalConfirmationVisible == true || _viewModel?.LatestApplication is not null
                ? PreferenceAppliedStage
                : _viewModel?.HasPreview == true ? PreferencePreviewStage : PreferenceSetupStage);
    }

    private void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (_viewModel is null)
        {
            return;
        }
        if (e.PropertyName == nameof(PreferenceRulesViewModel.HasPreview) && _viewModel.HasPreview
            && !_viewModel.IsReversalConfirmationVisible)
        {
            OpenStage(PreferencePreviewStage);
        }
        else if (e.PropertyName == nameof(PreferenceRulesViewModel.LatestApplication)
                 && _viewModel.LatestApplication is not null)
        {
            _applicationChangedDuringConfirmation |= _viewModel.IsApplicationConfirmationVisible;
            _applicationChangedDuringReversal |= _viewModel.IsReversalConfirmationVisible;
            if (!_viewModel.IsApplicationConfirmationVisible)
            {
                OpenStage(PreferenceAppliedStage);
            }
        }
        if (e.PropertyName == nameof(PreferenceRulesViewModel.IsApplicationConfirmationVisible))
        {
            if (_viewModel.IsApplicationConfirmationVisible)
            {
                _applicationChangedDuringConfirmation = false;
                OpenStage(PreferencePreviewStage);
            }
            else if (_applicationChangedDuringConfirmation)
            {
                OpenStage(PreferenceAppliedStage);
            }
            RestoreConfirmationFocus(
                _viewModel.IsApplicationConfirmationVisible,
                _applicationConfirmationWasVisible,
                PreferenceApplicationConfirmationHeading,
                _applicationChangedDuringConfirmation ? PreferenceAppliedStage : PreferenceApplyRuleButton);
            _applicationConfirmationWasVisible = _viewModel.IsApplicationConfirmationVisible;
            if (!_applicationConfirmationWasVisible)
            {
                _applicationChangedDuringConfirmation = false;
            }
        }
        else if (e.PropertyName == nameof(PreferenceRulesViewModel.IsReversalConfirmationVisible))
        {
            if (_viewModel.IsReversalConfirmationVisible)
            {
                _applicationChangedDuringReversal = false;
                OpenStage(PreferenceAppliedStage);
            }
            RestoreConfirmationFocus(
                _viewModel.IsReversalConfirmationVisible,
                _reversalConfirmationWasVisible,
                PreferenceReversalConfirmationHeading,
                _applicationChangedDuringReversal ? PreferenceAppliedStage : PreferenceReverseApplicationButton);
            _reversalConfirmationWasVisible = _viewModel.IsReversalConfirmationVisible;
            if (!_reversalConfirmationWasVisible)
            {
                _applicationChangedDuringReversal = false;
            }
        }
    }

    private void OnStageExpanded(object sender, RoutedEventArgs e)
    {
        if (!_updatingStages && sender is Expander expanded)
        {
            OpenStage(_viewModel?.IsApplicationConfirmationVisible == true
                ? PreferencePreviewStage
                : _viewModel?.IsReversalConfirmationVisible == true
                    ? PreferenceAppliedStage
                    : expanded);
        }
    }

    private void OnStageCollapsed(object sender, RoutedEventArgs e)
    {
        if (_updatingStages) return;
        if (_viewModel?.IsApplicationConfirmationVisible == true)
        {
            OpenStage(PreferencePreviewStage);
        }
        else if (_viewModel?.IsReversalConfirmationVisible == true)
        {
            OpenStage(PreferenceAppliedStage);
        }
    }

    private void OpenStage(Expander stage)
    {
        if (!Dispatcher.CheckAccess())
        {
            _ = Dispatcher.BeginInvoke(() => OpenStage(stage), DispatcherPriority.Normal);
            return;
        }
        // Stage one can raise Expanded during InitializeComponent before the other named stages exist.
        if (PreferenceSetupStage is null || PreferencePreviewStage is null || PreferenceAppliedStage is null)
        {
            return;
        }
        _updatingStages = true;
        try
        {
            foreach (var candidate in new[] { PreferenceSetupStage, PreferencePreviewStage, PreferenceAppliedStage })
            {
                candidate.IsExpanded = ReferenceEquals(candidate, stage);
            }
        }
        finally
        {
            _updatingStages = false;
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
