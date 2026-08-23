using System.ComponentModel;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using SuperDuper.Windows.Core.ViewModels;

namespace SuperDuper.Windows.Views;

public partial class PreflightView : UserControl
{
    public PreflightView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
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
                    PreflightProgressBar.Focus();
                }
                else if (viewModel.FocusTarget == "summary")
                {
                    PreflightSummaryHeading.Focus();
                }
            },
            DispatcherPriority.Background);
    }

    private void OnPreviewKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Home && Keyboard.Modifiers.HasFlag(ModifierKeys.Control))
        {
            PreflightSummaryHeading.Focus();
            e.Handled = true;
        }
    }
}
