using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Dispatching;
using SuperDuper.NativeMethods;
using System.Collections.ObjectModel;
using System.IO;
using System.Runtime.InteropServices;
using System.Text.Json;
using static SuperDuper.NativeMethods.SuperDuperEngine;

namespace SuperDuper.ViewModels;

public partial class MainViewModel : ObservableObject
{
    private EngineWrapper? _engine;
    private DispatcherQueue? _dispatcherQueue;

    public MainViewModel()
    {
        LoadUserConfig();
        ScanPaths.CollectionChanged += (_, _) => SaveUserConfig();
        IgnorePatterns.CollectionChanged += (_, _) => SaveUserConfig();

        // Open the engine eagerly so existing DB data is visible without requiring a scan
        _engine = new EngineWrapper();
        var (sessions, _) = _engine.ListSessions(0, 1);
        if (sessions.Count > 0)
            TotalFilesScanned = (int)sessions[0].FilesScanned;
        _ = LoadDuplicateGroupsAsync();
    }

    /// <summary>
    /// Exposes the engine so sub-pages can receive it via navigation parameter.
    /// </summary>
    public EngineWrapper? Engine => _engine;

    /// <summary>
    /// Must be called from the UI thread to capture the dispatcher.
    /// </summary>
    public void SetDispatcherQueue(DispatcherQueue queue)
    {
        _dispatcherQueue = queue;
    }

    /// <summary>
    /// Raised when an error occurs. Tuple is (title, detail).
    /// </summary>
    public event EventHandler<(string Title, string Detail)>? ErrorOccurred;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsNotScanning))]
    private bool _isScanning;

    public bool IsNotScanning => !IsScanning;

    [ObservableProperty]
    private string _statusMessage = "Ready";

    [ObservableProperty]
    private double _scanProgressMax = 1;

    [ObservableProperty]
    private double _scanProgressValue = 0;

    [ObservableProperty]
    private bool _scanProgressIndeterminate = true;

    [ObservableProperty]
    private string _scanPhaseLabel = "";

    [ObservableProperty]
    private string _scanCountLabel = "";

    [ObservableProperty]
    private int _totalDuplicateGroups;

    [ObservableProperty]
    private long _totalWastedBytes;

    [ObservableProperty]
    private int _totalFilesScanned;

    public string FormattedWastedBytes => FormatBytes(TotalWastedBytes);

    partial void OnTotalWastedBytesChanged(long value)
    {
        OnPropertyChanged(nameof(FormattedWastedBytes));
    }

    public ObservableCollection<string> ScanPaths { get; } = new();
    public ObservableCollection<string> IgnorePatterns { get; } = new()
    {
        "**/node_modules/**",
        "**/.git/**",
        "*/$RECYCLE.BIN",
    };
    public ObservableCollection<DuplicateGroupInfo> DuplicateGroups { get; } = new();

    [ObservableProperty]
    private string _newScanPath = "";

    [ObservableProperty]
    private string _newIgnorePattern = "";

    [RelayCommand]
    private void AddScanPath()
    {
        var path = NewScanPath.Trim();
        if (string.IsNullOrEmpty(path) || ScanPaths.Contains(path))
            return;

        if (!Directory.Exists(path))
        {
            StatusMessage = $"Warning: \"{path}\" does not exist or is not a directory.";
            ErrorOccurred?.Invoke(this, ("Invalid Path", $"The path \"{path}\" does not exist or is not a directory."));
            return;
        }

        ScanPaths.Add(path);
        NewScanPath = "";
    }

    public void AddScanPathDirect(string path)
    {
        if (string.IsNullOrEmpty(path) || ScanPaths.Contains(path))
            return;

        if (!Directory.Exists(path))
        {
            ErrorOccurred?.Invoke(this, ("Invalid Path", $"The path \"{path}\" does not exist or is not a directory."));
            return;
        }

        ScanPaths.Add(path);
    }

    [RelayCommand]
    private void RemoveScanPath(string path)
    {
        ScanPaths.Remove(path);
    }

    [RelayCommand]
    private void AddIgnorePattern()
    {
        var pattern = NewIgnorePattern.Trim();
        if (!string.IsNullOrEmpty(pattern) && !IgnorePatterns.Contains(pattern))
        {
            IgnorePatterns.Add(pattern);
            NewIgnorePattern = "";
        }
    }

    [RelayCommand]
    private void RemoveIgnorePattern(string pattern)
    {
        IgnorePatterns.Remove(pattern);
    }

    [RelayCommand]
    private void CancelScan()
    {
        _engine?.CancelScan();
        StatusMessage = "Cancelling...";
    }

    [RelayCommand]
    private async Task StartScanAsync()
    {
        if (IsScanning) return;
        if (ScanPaths.Count == 0)
        {
            StatusMessage = "Add at least one scan path before starting.";
            return;
        }

        // Validate all paths exist
        var invalidPaths = ScanPaths.Where(p => !Directory.Exists(p)).ToList();
        if (invalidPaths.Count > 0)
        {
            var pathList = string.Join("\n", invalidPaths);
            StatusMessage = $"{invalidPaths.Count} scan path(s) not found.";
            ErrorOccurred?.Invoke(this, ("Invalid Scan Paths",
                $"The following paths do not exist or are not directories:\n\n{pathList}"));
            return;
        }

        try
        {
            IsScanning = true;
            StatusMessage = "Scanning...";

            _engine?.Dispose();
            _engine = new EngineWrapper();

            // Set up progress callback to update status on UI thread
            _engine.SetProgressCallback((phase, current, total, messagePtr) =>
            {
                string phaseLabel, countLabel;
                double max = 1, value = 0;
                bool indeterminate = true;

                switch (phase)
                {
                    case 0:
                        phaseLabel = "Scanning for files...";
                        countLabel = $"{current:N0} files found";
                        break;
                    case 1:
                        phaseLabel = "Computing checksums...";
                        indeterminate = total == 0;
                        max = total > 0 ? total : 1;
                        value = current;
                        countLabel = total > 0 ? $"{current:N0} / {total:N0}" : $"{current:N0}";
                        break;
                    case 2:
                        phaseLabel = "Writing to database...";
                        countLabel = "";
                        break;
                    case 3:
                        phaseLabel = "Analyzing directories...";
                        countLabel = "";
                        break;
                    default:
                        phaseLabel = "";
                        countLabel = messagePtr != IntPtr.Zero
                            ? Marshal.PtrToStringUTF8(messagePtr) ?? ""
                            : "";
                        break;
                }

                _dispatcherQueue?.TryEnqueue(() =>
                {
                    ScanPhaseLabel = phaseLabel;
                    ScanCountLabel = countLabel;
                    ScanProgressIndeterminate = indeterminate;
                    ScanProgressMax = max;
                    ScanProgressValue = value;
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
            StatusMessage = "Scan complete. Loading results...";

            var (sessions, _) = _engine.ListSessions(0, 1);
            if (sessions.Count > 0)
                TotalFilesScanned = (int)sessions[0].FilesScanned;

            await LoadDuplicateGroupsAsync();
        }
        catch (Exception ex)
        {
            var message = ex.Message;
            if (message.Contains("Cancelled"))
            {
                StatusMessage = "Scan cancelled.";
            }
            else
            {
                StatusMessage = $"Error: {message}";
                ErrorOccurred?.Invoke(this, ("Scan Failed", message));
            }
        }
        finally
        {
            IsScanning = false;
            ScanPhaseLabel = "";
            ScanCountLabel = "";
            ScanProgressIndeterminate = true;
            ScanProgressMax = 1;
            ScanProgressValue = 0;
        }
    }

    [RelayCommand]
    private async Task LoadDuplicateGroupsAsync()
    {
        if (_engine == null) return;

        DuplicateGroups.Clear();
        TotalWastedBytes = 0;
        var (groups, total) = _engine.QueryDuplicateGroups(0, 100);
        TotalDuplicateGroups = total;

        foreach (var group in groups)
        {
            DuplicateGroups.Add(group);
            TotalWastedBytes += group.WastedBytes;
        }

        StatusMessage = $"{TotalDuplicateGroups} duplicate groups found, {FormatBytes(TotalWastedBytes)} wasted";
    }

    [RelayCommand]
    private void MarkForDeletion(long fileId)
    {
        _engine?.MarkForDeletion(fileId);
    }

    [RelayCommand]
    private void UnmarkForDeletion(long fileId)
    {
        _engine?.UnmarkForDeletion(fileId);
    }

    private const string UserConfigPath = "user_config.json";

    private void LoadUserConfig()
    {
        try
        {
            if (!File.Exists(UserConfigPath)) return;
            var json = File.ReadAllText(UserConfigPath);
            var config = JsonSerializer.Deserialize<UserConfig>(json);
            if (config == null) return;
            ScanPaths.Clear();
            foreach (var path in config.ScanPaths ?? [])
                ScanPaths.Add(path);
            IgnorePatterns.Clear();
            foreach (var pattern in config.IgnorePatterns ?? [])
                IgnorePatterns.Add(pattern);
        }
        catch { }
    }

    private void SaveUserConfig()
    {
        try
        {
            var config = new UserConfig
            {
                ScanPaths = ScanPaths.ToArray(),
                IgnorePatterns = IgnorePatterns.ToArray(),
            };
            File.WriteAllText(UserConfigPath,
                JsonSerializer.Serialize(config, new JsonSerializerOptions { WriteIndented = true }));
        }
        catch { }
    }

    private sealed class UserConfig
    {
        public string[]? ScanPaths { get; set; }
        public string[]? IgnorePatterns { get; set; }
    }

    private static string FormatBytes(long bytes)
    {
        string[] sizes = { "B", "KB", "MB", "GB", "TB" };
        double len = bytes;
        int order = 0;
        while (len >= 1024 && order < sizes.Length - 1)
        {
            order++;
            len /= 1024;
        }
        return $"{len:0.##} {sizes[order]}";
    }

    public void Dispose()
    {
        _engine?.Dispose();
    }
}
