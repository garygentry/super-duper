using System.IO;
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
#if DEBUG
        ApplyIsolatedUiDevConfiguration();
#endif
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

#if DEBUG
    private static void ApplyIsolatedUiDevConfiguration()
    {
        if (!string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("SUPER_DUPER_DB_PATH"))) return;
        var configurationPath = Path.ChangeExtension(Environment.ProcessPath, ".uidev");
        if (configurationPath is null || !File.Exists(configurationPath)) return;

        var repository = new DirectoryInfo(AppContext.BaseDirectory);
        while (repository is not null && !File.Exists(Path.Combine(repository.FullName, "Cargo.toml")))
            repository = repository.Parent;
        if (repository is null) throw new InvalidOperationException("UI development repository not found.");

        var lines = File.ReadAllLines(configurationPath);
        if (lines.Length != 2) throw new InvalidOperationException("UI development configuration is incomplete.");
        var worker = Path.GetFullPath(lines[0]);
        var state = Path.GetFullPath(lines[1]);
        var stateRoot = Path.Combine(repository.FullName, "artifacts", "ui-dev-session") + Path.DirectorySeparatorChar;
        var expectedWorker = Path.Combine(repository.FullName, "target", "debug", "super-duper-worker.exe");
        if (!state.StartsWith(stateRoot, StringComparison.OrdinalIgnoreCase)
            || !Directory.Exists(state)
            || !string.Equals(worker, expectedWorker, StringComparison.OrdinalIgnoreCase)
            || !File.Exists(worker))
            throw new InvalidOperationException("UI development configuration must use an isolated state and matching Debug worker.");

        Environment.SetEnvironmentVariable("SUPER_DUPER_WORKER_PATH", worker);
        Environment.SetEnvironmentVariable("SUPER_DUPER_DB_PATH", Path.Combine(state, "super_duper.db"));
        Environment.SetEnvironmentVariable("SUPER_DUPER_STATUS_DB_PATH", Path.Combine(state, "scan_status.db"));
        Environment.SetEnvironmentVariable("HASH_CACHE_PATH", Path.Combine(state, "hash-cache"));
        Environment.SetEnvironmentVariable("LOG_FILE_PATH", Path.Combine(state, "app.log"));
    }
#endif

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
