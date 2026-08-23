using System.ComponentModel;
using System.Windows;
using System.Windows.Threading;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows;

public partial class MainWindow : Window
{
    internal static DispatcherPriority ShutdownDispatcherPriority => DispatcherPriority.Normal;

    private readonly IWorkerClient _workerClient;
    private readonly CancellationTokenSource _lifetime = new();
    private Task _initialization = Task.CompletedTask;
    private bool _shutdownStarted;
    private bool _shutdownComplete;

    public MainWindow(ShellViewModel viewModel, IWorkerClient workerClient)
    {
        InitializeComponent();
        ViewModel = viewModel;
        _workerClient = workerClient;
        DataContext = viewModel;
        ViewModel.PropertyChanged += OnViewModelPropertyChanged;
        Closing += OnClosing;
    }

    public ShellViewModel ViewModel { get; }

    public bool IsShutdownRequested => _shutdownStarted;

    public Task InitializeAsync()
    {
        _initialization = ViewModel.InitializeAsync(_lifetime.Token);
        return _initialization;
    }

    private void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(ShellViewModel.FocusRequestVersion)
            || ViewModel.FocusTarget != "start-scan")
        {
            return;
        }
        _ = Dispatcher.BeginInvoke(
            () => StartScanButton.Focus(),
            DispatcherPriority.Background);
    }

    private void OnClosing(object? sender, CancelEventArgs e)
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
        _ = ShutdownAsync();
    }

    private async Task ShutdownAsync()
    {
        if (!await ViewModel.ConfirmCancelAndExitAsync())
        {
            _shutdownStarted = false;
            return;
        }

        try
        {
            IsEnabled = false;
            _lifetime.Cancel();
            try
            {
                await _initialization;
            }
            catch (OperationCanceledException) when (_lifetime.IsCancellationRequested)
            {
            }
            await _workerClient.DisposeAsync();
        }
        catch (Exception exception)
        {
            MessageBox.Show(
                this,
                $"Super Duper could not shut down its owned worker safely.\n\n{exception.Message}",
                "Shutdown failed",
                MessageBoxButton.OK,
                MessageBoxImage.Error);
            IsEnabled = true;
            _shutdownStarted = false;
            return;
        }

        _ = Dispatcher.BeginInvoke(
            () =>
            {
                _shutdownComplete = true;
                Application.Current.Shutdown();
            },
            ShutdownDispatcherPriority);
    }
}
