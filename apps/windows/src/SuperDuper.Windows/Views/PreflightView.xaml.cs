using System.ComponentModel;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using SuperDuper.Windows.Core.ViewModels;

namespace SuperDuper.Windows.Views;

public partial class PreflightView : UserControl
{
    private System.Windows.FrameworkElement? _pendingPreflightFocus;
    private System.Windows.Window? _focusWindow;

    public PreflightView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
        Loaded += OnLoaded;
        Unloaded += OnUnloaded;
    }

    private void OnLoaded(object sender, System.Windows.RoutedEventArgs e)
    {
        _focusWindow = System.Windows.Window.GetWindow(this);
        if (_focusWindow is not null)
        {
            _focusWindow.Activated += OnFocusWindowActivated;
        }
    }

    private void OnUnloaded(object sender, System.Windows.RoutedEventArgs e)
    {
        LayoutUpdated -= OnPendingFocusLayoutUpdated;
        if (_focusWindow is not null)
        {
            _focusWindow.Activated -= OnFocusWindowActivated;
            _focusWindow = null;
        }
    }

    private void OnFocusWindowActivated(object? sender, EventArgs e)
    {
        _ = Dispatcher.BeginInvoke(TryRestorePendingPreflightFocus, DispatcherPriority.Background);
    }

    private void OnPendingFocusLayoutUpdated(object? sender, EventArgs e)
    {
        TryRestorePendingPreflightFocus();
    }

    private void OnDataContextChanged(object sender, System.Windows.DependencyPropertyChangedEventArgs e)
    {
        if (e.OldValue is INotifyPropertyChanged oldValue)
        {
            oldValue.PropertyChanged -= OnViewModelPropertyChanged;
        }
        if (e.OldValue is PreflightViewModel oldPreflight)
        {
            oldPreflight.Operation.PropertyChanged -= OnRecycleOperationPropertyChanged;
            oldPreflight.Operation.RecoveryReview.PropertyChanged -= OnRecoveryReviewPropertyChanged;
        }
        if (e.NewValue is INotifyPropertyChanged newValue)
        {
            newValue.PropertyChanged += OnViewModelPropertyChanged;
        }
        if (e.NewValue is PreflightViewModel newPreflight)
        {
            newPreflight.Operation.PropertyChanged += OnRecycleOperationPropertyChanged;
            newPreflight.Operation.RecoveryReview.PropertyChanged += OnRecoveryReviewPropertyChanged;
        }
    }

    private void OnRecycleOperationPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(RecycleOperationViewModel.FocusRequestVersion)
            || sender is not RecycleOperationViewModel viewModel
            || viewModel.FocusTarget != "operation-items")
        {
            return;
        }
        _ = Dispatcher.BeginInvoke(
            () => RecycleOperationItemsList.Focus(),
            DispatcherPriority.Background);
    }

    private void OnRecoveryReviewPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(RecoveryReviewViewModel.FocusRequestVersion)
            || sender is not RecoveryReviewViewModel viewModel)
        {
            return;
        }
        _ = Dispatcher.BeginInvoke(
            () =>
            {
                if (viewModel.FocusTarget == "history")
                {
                    RecoveryReviewHistoryList.Focus();
                }
                else if (viewModel.FocusTarget == "status")
                {
                    RecoveryReviewStatusHeading.Focus();
                }
                else if (viewModel.FocusTarget == "observation-kind")
                {
                    RecoveryReviewObservationKind.Focus();
                }
            },
            DispatcherPriority.Background);
    }

    private void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(PreflightViewModel.FocusRequestVersion)
            || DataContext is not PreflightViewModel viewModel)
        {
            return;
        }
        _ = Dispatcher.BeginInvoke(
            () =>
            {
                if (viewModel.FocusTarget == "progress")
                {
                    RequestPreflightFocus(PreflightProgressBar);
                }
                else if (viewModel.FocusTarget == "summary")
                {
                    RequestPreflightFocus(PreflightSummaryHeading);
                }
            },
            DispatcherPriority.Background);
    }

    private void OnPreviewKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Home && Keyboard.Modifiers.HasFlag(ModifierKeys.Control))
        {
            RestorePreflightFocus(PreflightSummaryHeading);
            e.Handled = true;
        }
    }

    internal static void RestorePreflightFocus(System.Windows.FrameworkElement target)
    {
        target.BringIntoView();
        var focusScope = FocusManager.GetFocusScope(target);
        FocusManager.SetFocusedElement(focusScope, target);
        target.Focus();
        Keyboard.Focus(target);
    }

    private void RequestPreflightFocus(System.Windows.FrameworkElement target)
    {
        _pendingPreflightFocus = target;
        LayoutUpdated -= OnPendingFocusLayoutUpdated;
        LayoutUpdated += OnPendingFocusLayoutUpdated;
        _ = Dispatcher.BeginInvoke(TryRestorePendingPreflightFocus, DispatcherPriority.Background);
    }

    private void TryRestorePendingPreflightFocus()
    {
        if (_pendingPreflightFocus is not { } target)
        {
            return;
        }

        if (!target.IsLoaded || !target.IsVisible)
        {
            return;
        }

        RestorePreflightFocus(target);
        if (target.IsKeyboardFocusWithin)
        {
            _pendingPreflightFocus = null;
            LayoutUpdated -= OnPendingFocusLayoutUpdated;
        }
    }
}
