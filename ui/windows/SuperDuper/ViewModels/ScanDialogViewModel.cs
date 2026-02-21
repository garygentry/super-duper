using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Dispatching;
using SuperDuper.NativeMethods;
using SuperDuper.Services;
using SuperDuper.Services.Platform;
using System.Collections.ObjectModel;
using System.Runtime.InteropServices;

namespace SuperDuper.ViewModels;

/// <summary>
/// Drives the scan dialog. Owns scan paths (read-only from settings), options, and scan execution.
/// Scan target editing is now on the Dashboard; this dialog confirms and starts.
/// </summary>
public partial class ScanDialogViewModel : ObservableObject
{
    private readonly EngineWrapper _engine;
    private readonly IDatabaseService _db;
    private readonly INotificationService _notifications;
    private readonly SettingsService _settings;
    private DispatcherQueue? _dispatcherQueue;

    public ScanDialogViewModel(
        EngineWrapper engine,
        IDatabaseService db,
        INotificationService notifications,
        SettingsService settings)
    {
        _engine = engine;
        _db = db;
        _notifications = notifications;
        _settings = settings;

        // Load defaults from settings
        foreach (var p in settings.ScanPaths) ScanPaths.Add(p);
        foreach (var p in settings.IgnorePatterns) IgnorePatterns.Add(p);
        MinFileSize = settings.DefaultMinFileSize;
        CpuThreads = settings.DefaultCpuThreads;
    }

    public void SetDispatcherQueue(DispatcherQueue queue) => _dispatcherQueue = queue;

    // ── Step 1: Scan paths (read-only, loaded from settings) ─────────

    public ObservableCollection<string> ScanPaths { get; } = new();

    // ── Step 2: Options ───────────────────────────────────────────────

    public ObservableCollection<string> IgnorePatterns { get; } = new();

    [ObservableProperty]
    private long _minFileSize;

    [ObservableProperty]
    private string _selectedHashAlgorithm = "xxHash64";

    [ObservableProperty]
    private bool _includeHiddenFiles;

    [ObservableProperty]
    private int _cpuThreads = Environment.ProcessorCount;

    [ObservableProperty]
    private string _newIgnorePattern = "";

    // ── Step 3: Save profile ──────────────────────────────────────────

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CanSaveProfile))]
    private string _profileName = "";

    public bool CanSaveProfile => !string.IsNullOrWhiteSpace(ProfileName);

    // ── Scan progress ─────────────────────────────────────────────────

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsNotScanning))]
    private bool _isScanning;

    public bool IsNotScanning => !IsScanning;

    [ObservableProperty]
    private string _scanPhaseLabel = "";

    [ObservableProperty]
    private string _scanCountLabel = "";

    [ObservableProperty]
    private double _scanProgressMax = 1;

    [ObservableProperty]
    private double _scanProgressValue;

    [ObservableProperty]
    private bool _scanProgressIndeterminate = true;

    [ObservableProperty]
    private string _currentFilePath = "";

    public event EventHandler? ScanCompleted;
    public event EventHandler<string>? ErrorOccurred;

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
    private void CancelScan() => _engine.CancelScan();

    [RelayCommand]
    private async Task StartScanAsync()
    {
        if (IsScanning || ScanPaths.Count == 0) return;

        try
        {
            IsScanning = true;

            _engine.SetProgressCallback((phase, current, total, messagePtr) =>
            {
                string phaseLabel;
                double max = 1, value = 0;
                bool indeterminate = true;

                switch (phase)
                {
                    case 0:
                        phaseLabel = "Phase 1 of 4: Scanning for files...";
                        break;
                    case 1:
                        phaseLabel = "Phase 2 of 4: Computing checksums...";
                        indeterminate = total == 0;
                        max = total > 0 ? total : 1;
                        value = current;
                        break;
                    case 2:
                        phaseLabel = "Phase 3 of 4: Writing to database...";
                        break;
                    case 3:
                        phaseLabel = "Phase 4 of 4: Analyzing directories...";
                        break;
                    default:
                        phaseLabel = "";
                        break;
                }

                var countLabel = total > 0
                    ? $"{current:N0} / {total:N0}"
                    : current > 0 ? $"{current:N0}" : "";

                var filePath = messagePtr != IntPtr.Zero
                    ? Marshal.PtrToStringUTF8(messagePtr) ?? ""
                    : "";

                _dispatcherQueue?.TryEnqueue(() =>
                {
                    ScanPhaseLabel = phaseLabel;
                    ScanCountLabel = countLabel;
                    ScanProgressIndeterminate = indeterminate;
                    ScanProgressMax = max;
                    ScanProgressValue = value;
                    if (!string.IsNullOrEmpty(filePath)) CurrentFilePath = filePath;
                });
            });

            var paths = ScanPaths.ToArray();
            var patterns = IgnorePatterns.ToArray();

            await Task.Run(() =>
            {
                _engine.SetScanPaths(paths);
                if (patterns.Length > 0)
                    _engine.SetIgnorePatterns(patterns);
                _engine.StartScan();
            });

            _engine.ClearProgressCallback();

            // Save paths to settings for next time
            _settings.ScanPaths = ScanPaths.ToList();
            _settings.IgnorePatterns = IgnorePatterns.ToList();

            ScanCompleted?.Invoke(this, EventArgs.Empty);
        }
        catch (Exception ex)
        {
            if (!ex.Message.Contains("Cancelled"))
                ErrorOccurred?.Invoke(this, ex.Message);
        }
        finally
        {
            _engine.ClearProgressCallback();
            IsScanning = false;
            ScanPhaseLabel = "";
            ScanCountLabel = "";
            ScanProgressIndeterminate = true;
            ScanProgressMax = 1;
            ScanProgressValue = 0;
            CurrentFilePath = "";
        }
    }

}
