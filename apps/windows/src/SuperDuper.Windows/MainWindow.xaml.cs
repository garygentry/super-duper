using System.ComponentModel;
using System.Windows;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows;

public partial class MainWindow : Window
{
    private readonly IWorkerClient _workerClient;
    private bool _shutdownStarted;
    private bool _shutdownComplete;

    public MainWindow(ShellViewModel viewModel, IWorkerClient workerClient)
    {
        InitializeComponent();
        ViewModel = viewModel;
        _workerClient = workerClient;
        DataContext = viewModel;
        Closing += OnClosing;
    }

    public ShellViewModel ViewModel { get; }

    private async void OnClosing(object? sender, CancelEventArgs e)
    {
        if (_shutdownComplete)
        {
            return;
        }

        e.Cancel = true;
        if (_shutdownStarted)
        {
            return;
        }

        _shutdownStarted = true;

        if (!await ViewModel.ConfirmCancelAndExitAsync())
        {
            _shutdownStarted = false;
            return;
        }

        try
        {
            IsEnabled = false;
            await _workerClient.DisposeAsync();
        }
        finally
        {
            _shutdownComplete = true;
            Close();
        }
    }
}
