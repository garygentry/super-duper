using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Windows.AppNotifications;
using SuperDuper.NativeMethods;
using SuperDuper.Services;
using SuperDuper.Services.Platform;
using SuperDuper.Services.Platform.Windows;
using SuperDuper.ViewModels;

namespace SuperDuper;

public partial class App : Application
{
    public static Window? MainWindow { get; private set; }
    public static IServiceProvider Services { get; private set; } = null!;

    private const string DbPath = "super_duper.db";

    public App()
    {
        // InitializeComponent → LoadComponent processes App.xaml which declares
        // all resources: XamlControlsResources, Colors.xaml, SharedStyles.xaml,
        // and converter instances. Our XamlTypeInfo.cs provides IXamlMetadataProvider
        // so the runtime parser can resolve all types (WinUI + app-local).
        // NOTE: Application.Resources getter/setter throw COMException (E_UNEXPECTED)
        // on .NET 10 when accessed from code-behind, so ALL resources must be in XAML.
        this.InitializeComponent();

        Services = ConfigureServices();

        this.UnhandledException += (_, e) =>
        {
            System.Diagnostics.Debug.WriteLine($"UNHANDLED: {e.Exception}");
            System.IO.File.AppendAllText("super_duper_nav_errors.log",
                $"[{DateTime.Now:HH:mm:ss}] UNHANDLED: {e.Exception}\n---\n");
            e.Handled = true;
        };

        // Register for toast notification activation
        AppNotificationManager.Default.NotificationInvoked += OnNotificationInvoked;
        AppNotificationManager.Default.Register();
    }

    private static IServiceProvider ConfigureServices()
    {
        var services = new ServiceCollection();

        // Core infrastructure (singletons)
        services.AddSingleton<EngineWrapper>(_ => new EngineWrapper(DbPath));
        services.AddSingleton<IDatabaseService>(_ => new DatabaseService(DbPath));
        services.AddSingleton<IUndoService, UndoService>();
        services.AddSingleton<SettingsService>();
        services.AddSingleton<SuggestionEngine>();
        services.AddSingleton<DriveColorService>();
        services.AddSingleton<ScanService>();

        // Platform services (Windows implementations)
        services.AddSingleton<IShellIntegrationService, WindowsShellService>();
        services.AddSingleton<INotificationService, WindowsNotificationService>();
        services.AddSingleton<IFilePickerService, WindowsFilePickerService>();

        // ViewModels (transient — each page gets a fresh instance)
        services.AddTransient<ShellViewModel>();
        services.AddTransient<DashboardViewModel>();
        services.AddTransient<ScanDialogViewModel>();
        services.AddTransient<ExplorerViewModel>();
        services.AddTransient<GroupsViewModel>();
        services.AddTransient<DirectoriesViewModel>();

        return services.BuildServiceProvider();
    }

    protected override async void OnLaunched(LaunchActivatedEventArgs args)
    {
        MainWindow = new MainWindow();
        MainWindow.Activate();

        // Set DispatcherQueue on ScanService so progress callbacks marshal to UI thread
        var scanService = Services.GetRequiredService<ScanService>();
        scanService.SetDispatcherQueue(MainWindow.DispatcherQueue);

        var error = EngineWrapper.ValidateNativeLibrary();
        if (error != null)
        {
            var dialog = new ContentDialog
            {
                Title = "Failed to Load Native Library",
                Content = error,
                CloseButtonText = "Exit",
                XamlRoot = MainWindow.Content.XamlRoot,
            };
            await dialog.ShowAsync();
            MainWindow.Close();
            return;
        }

        // Initialize C#-managed schema (creates review_decisions, undo_log, etc.)
        var db = Services.GetRequiredService<IDatabaseService>();
        await db.EnsureSchemaAsync();
    }

    private void OnNotificationInvoked(AppNotificationManager sender, AppNotificationActivatedEventArgs args)
    {
        // Bring window to foreground when toast "Open Results" is clicked
        if (args.Arguments.TryGetValue("action", out var action) && action == "openResults")
        {
            MainWindow?.DispatcherQueue.TryEnqueue(() => MainWindow.Activate());
        }
    }
}
