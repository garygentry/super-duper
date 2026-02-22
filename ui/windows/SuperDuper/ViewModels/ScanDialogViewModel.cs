using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.NativeMethods;
using SuperDuper.Services;
using SuperDuper.Services.Platform;
using System.Collections.ObjectModel;

namespace SuperDuper.ViewModels;

/// <summary>
/// Drives the advanced scan dialog. Owns scan options and ignore pattern management.
/// Scan execution and progress are delegated to ScanService.
/// Scan target editing is on the Dashboard; this dialog confirms and starts.
/// </summary>
public partial class ScanDialogViewModel : ObservableObject
{
    private readonly EngineWrapper _engine;
    private readonly IDatabaseService _db;
    private readonly INotificationService _notifications;
    private readonly SettingsService _settings;

    public ScanService ScanService { get; }

    public ScanDialogViewModel(
        EngineWrapper engine,
        IDatabaseService db,
        INotificationService notifications,
        SettingsService settings,
        ScanService scanService)
    {
        _engine = engine;
        _db = db;
        _notifications = notifications;
        _settings = settings;
        ScanService = scanService;

        // Load defaults from settings
        foreach (var p in settings.ScanPaths) ScanPaths.Add(p);
        foreach (var p in settings.IgnorePatterns) IgnorePatterns.Add(p);
        MinFileSize = settings.DefaultMinFileSize;
        CpuThreads = settings.DefaultCpuThreads;
    }

    // ── Step 1: Scan paths (read-only, loaded from settings) ─────────

    public ObservableCollection<string> ScanPaths { get; } = new();

    // ── Step 2: Options ───────────────────────────────────────────────

    public ObservableCollection<string> IgnorePatterns { get; } = new();

    [ObservableProperty]
    public partial long MinFileSize { get; set; }

    [ObservableProperty]
    public partial string SelectedHashAlgorithm { get; set; } = "xxHash64";

    [ObservableProperty]
    public partial bool IncludeHiddenFiles { get; set; }

    [ObservableProperty]
    public partial int CpuThreads { get; set; } = Environment.ProcessorCount;

    [ObservableProperty]
    public partial string NewIgnorePattern { get; set; } = "";

    // ── Step 3: Save profile ──────────────────────────────────────────

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CanSaveProfile))]
    public partial string ProfileName { get; set; } = "";

    public bool CanSaveProfile => !string.IsNullOrWhiteSpace(ProfileName);

    // ── Commands ──────────────────────────────────────────────────────

    [RelayCommand]
    private void AddIgnorePattern()
    {
        var p = NewIgnorePattern.Trim();
        if (!string.IsNullOrEmpty(p) && !IgnorePatterns.Contains(p))
        {
            IgnorePatterns.Add(p);
            NewIgnorePattern = "";
        }
    }

    [RelayCommand]
    private void RemoveIgnorePattern(string pattern) => IgnorePatterns.Remove(pattern);

    [RelayCommand]
    private async Task StartScanAsync()
    {
        var paths = ScanPaths.ToArray();
        var patterns = IgnorePatterns.ToArray();
        // TODO: Apply advanced options (minSize, hashAlgo, threads) to engine when FFI supports them
        await ScanService.StartScanAsync(paths, patterns);
    }
}
