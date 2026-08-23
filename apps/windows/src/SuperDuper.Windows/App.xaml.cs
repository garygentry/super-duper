using System.Windows;
using Microsoft.Extensions.DependencyInjection;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Infrastructure;
using SuperDuper.Windows.Services;

namespace SuperDuper.Windows;

public partial class App : Application
{
    private readonly ServiceProvider _services;

    public App()
    {
        var services = new ServiceCollection();
        services.AddSingleton<IWorkerClient>(
            _ => new WorkerClient(WorkerExecutableLocator.Resolve()));
        services.AddSingleton<IFolderPickerService, FolderPickerService>();
        services.AddSingleton<IUserConfirmationService, UserConfirmationService>();
        services.AddSingleton<IUiDispatcher>(_ => new WpfUiDispatcher(Dispatcher));
        services.AddSingleton<IClipboardService, WpfClipboardService>();
        services.AddSingleton<IExplorerService, WindowsExplorerService>();
        services.AddSingleton<IRecycleBinService, WindowsRecycleBinService>();
        services.AddSingleton<ICloudLocationService>(_ => CreateCloudLocationService());
        services.AddSingleton<IRecycleOperationCapabilityExecutor, DisabledRecycleOperationCapabilityExecutor>();
        services.AddSingleton<ShellViewModel>();
        services.AddSingleton<MainWindow>();
        _services = services.BuildServiceProvider(validateScopes: true);
    }

    internal static ICloudLocationService CreateCloudLocationService() =>
        string.Equals(
            Environment.GetEnvironmentVariable("SUPER_DUPER_DISABLE_CLOUD_REGISTRATION_DISCOVERY"),
            "1",
            StringComparison.Ordinal)
            ? new UnavailableCloudLocationService(
                "Registered cloud location detection is disabled for this diagnostic run.")
            : new WindowsCloudLocationService();

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        var window = _services.GetRequiredService<MainWindow>();
        MainWindow = window;
        window.Show();

        try
        {
            await window.InitializeAsync();
        }
        catch (OperationCanceledException) when (window.IsShutdownRequested)
        {
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _services.Dispose();
        base.OnExit(e);
    }
}
