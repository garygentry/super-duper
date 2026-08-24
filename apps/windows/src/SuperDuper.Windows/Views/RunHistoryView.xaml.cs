using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using SuperDuper.Windows.Core.ViewModels;

namespace SuperDuper.Windows.Views;

public partial class RunHistoryView : UserControl
{
    public RunHistoryView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        if (e.OldValue is RunHistoryViewModel oldViewModel)
        {
            oldViewModel.PropertyChanged -= OnViewModelPropertyChanged;
        }
        if (e.NewValue is RunHistoryViewModel newViewModel)
        {
            newViewModel.PropertyChanged += OnViewModelPropertyChanged;
        }
    }

    private void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(RunHistoryViewModel.FocusRequestVersion)
            || sender is not RunHistoryViewModel viewModel)
        {
            return;
        }
        Dispatcher.BeginInvoke(() =>
        {
            if (viewModel.FocusTarget == "warnings")
            {
                RunWarningGrid.Focus();
                Keyboard.Focus(RunWarningGrid);
            }
            else if (viewModel.FocusTarget == "history")
            {
                RunHistoryGrid.Focus();
                Keyboard.Focus(RunHistoryGrid);
            }
        });
    }
}
