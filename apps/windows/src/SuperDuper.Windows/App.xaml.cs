using System.Windows;
using Microsoft.Extensions.DependencyInjection;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Infrastructure;

namespace SuperDuper.Windows;

public partial class App : Application
{
    private readonly ServiceProvider _services;

    public App()
    {
        var services = new ServiceCollection();
        services.AddSingleton<IWorkerClient>(
            _ => new WorkerClient(WorkerExecutableLocator.Resolve()));
        services.AddSingleton<ShellViewModel>();
        services.AddSingleton<MainWindow>();
        _services = services.BuildServiceProvider(validateScopes: true);
    }

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        var window = _services.GetRequiredService<MainWindow>();
        MainWindow = window;
        window.Show();

        await window.ViewModel.InitializeAsync();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _services.Dispose();
        base.OnExit(e);
    }
}
