using System.ComponentModel;
using System.Windows;
using System.Windows.Threading;
using SuperDuper.Windows.Accessibility;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Infrastructure;

namespace SuperDuper.Windows;

public partial class MainWindow : Window
{
    internal static DispatcherPriority ShutdownDispatcherPriority => DispatcherPriority.Normal;

    private readonly IWorkerClient _workerClient;
    private readonly CancellationTokenSource _lifetime = new();
    private Task _initialization = Task.CompletedTask;
    private bool _shutdownStarted;
    private bool _shutdownComplete;
    private long _focusNavigationGeneration;

    public MainWindow(ShellViewModel viewModel, IWorkerClient workerClient)
        : this(viewModel, workerClient, ownsWorkerLifetime: true) { }

    // The isolated presentation fixture supplies fake services and manages their lifetime.
    internal MainWindow(ShellViewModel viewModel, IWorkerClient workerClient, bool ownsWorkerLifetime,
        ITextScaleSource? textScaleSource = null)
    {
        InitializeComponent();
        var textScale = new WindowTextScale(this, textScaleSource ?? new WindowsTextScaleSource());
        Closed += (_, _) => textScale.Dispose();
        ViewModel = viewModel;
        _workerClient = workerClient;
        DataContext = viewModel;
        ViewModel.PropertyChanged += OnViewModelPropertyChanged;
        Closed += (_, _) => ViewModel.PropertyChanged -= OnViewModelPropertyChanged;
        if (ownsWorkerLifetime) Closing += OnClosing;
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
        if (e.PropertyName == nameof(ShellViewModel.HasSetupDeparture))
        {
            _ = Dispatcher.BeginInvoke(() =>
            {
                if (ViewModel.HasSetupDeparture) StayInSetupButton.Focus();
                else (ScanTabs.SelectedItem as System.Windows.Controls.TabItem)?.Focus();
            }, DispatcherPriority.Background);
        }
        if (e.PropertyName == nameof(ShellViewModel.SelectedDestination)) _focusNavigationGeneration++;
        if (e.PropertyName != nameof(ShellViewModel.FocusRequestVersion))
        {
            return;
        }
        var version = ViewModel.FocusRequestVersion;
        var target = ViewModel.FocusTarget;
        var destination = ViewModel.SelectedDestination;
        var navigation = _focusNavigationGeneration;
        bool IsCurrent() => IsLoaded && ViewModel.FocusRequestVersion == version
            && ViewModel.SelectedDestination == destination && _focusNavigationGeneration == navigation;
        _ = Dispatcher.BeginInvoke(() =>
        {
            if (!IsCurrent()) return;
            if (target == "start-scan") StartScanButton.Focus();
            else if (target is "results-navigation" or "scan-navigation")
            {
                var tabs = target == "results-navigation" ? ResultsTabs : ScanTabs;
                (tabs.SelectedItem as System.Windows.Controls.TabItem)?.Focus();
            }
            else if (target == "duplicate-file-groups")
                _ = DuplicateFilesWorkspace.RestoreGroupGridFocusAsync(IsCurrent);
            else if (target == "duplicate-folder-groups")
                _ = DuplicateFoldersWorkspace.RestoreGroupGridFocusAsync();
            else if (target == "progress-warnings")
                _ = ProgressWorkspace.RestoreWarningEntryFocus();
            else if (target == "summary-warnings")
                _ = SummaryWorkspace.RestoreWarningEntryFocus();
        }, DispatcherPriority.Background);
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
