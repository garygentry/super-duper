using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Extensions.DependencyInjection;
using SuperDuper.Models;
using SuperDuper.NativeMethods;
using SuperDuper.Services;
using SuperDuper.Services.Platform;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.Text.Json;

namespace SuperDuper.ViewModels;

/// <summary>
/// Drives the Dashboard page. Owns session selection, metrics display,
/// review progress, quick wins, scan target management, and inline scan initiation.
/// Scan execution delegates to ScanService (centralized singleton).
/// </summary>
public partial class DashboardViewModel : ObservableObject
{
    private readonly EngineWrapper _engine;
    private readonly IDatabaseService _db;
    private readonly SettingsService _settings;
    private readonly IFilePickerService _filePicker;
    private bool _suppressPickerSideEffects;

    public ScanService ScanService { get; }

    public DashboardViewModel(EngineWrapper engine, IDatabaseService db, SettingsService settings, IFilePickerService filePicker, ScanService scanService)
    {
        _engine = engine;
        _db = db;
        _settings = settings;
        _filePicker = filePicker;
        ScanService = scanService;

        // Load saved scan paths
        foreach (var p in settings.ScanPaths) ScanPaths.Add(p);
        ScanPaths.CollectionChanged += OnScanPathsChanged;
        ScanService.UpdateCanScan(ScanPaths.Count > 0);

        // When ScanService.IsScanning changes, update our CanScan binding
        ScanService.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(Services.ScanService.IsScanning))
            {
                ScanCommand.NotifyCanExecuteChanged();
                OnPropertyChanged(nameof(CanScan));
            }
        };

        // When scan completes, refresh session picker + metrics
        ScanService.ScanCompleted += async (_, _) =>
        {
            await LoadSessionPickerAsync();
        };

        LoadSessionPickerAsync().FireAndForget(nameof(DashboardViewModel) + ".ctor");
    }

    private void OnScanPathsChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        OnPropertyChanged(nameof(HasScanPaths));
        ScanService.UpdateCanScan(ScanPaths.Count > 0);
        ScanCommand.NotifyCanExecuteChanged();
        OnPropertyChanged(nameof(CanScan));
    }

    // ── Session picker ────────────────────────────────────────────────

    public ObservableCollection<SessionPickerItem> SessionPickerItems { get; } = new();

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsNewScanSelected))]
    public partial SessionPickerItem? SelectedSession { get; set; }

    public bool IsNewScanSelected => SelectedSession?.IsNewScan ?? true;

    partial void OnSelectedSessionChanged(SessionPickerItem? value)
    {
        if (_suppressPickerSideEffects) return;
        ActivateSelectedSessionAsync().FireAndForget(nameof(DashboardViewModel) + "." + nameof(OnSelectedSessionChanged));
    }

    private void ClearAllMetrics()
    {
        TotalFilesScanned = 0;
        TotalDuplicateGroups = 0;
        TotalWastedBytes = 0;
        ReviewedCount = 0;
        TotalReviewable = 0;
        ReviewProgressPercent = 0;
        QuickWins.Clear();
        OnPropertyChanged(nameof(HasQuickWins));
        TreemapNodes.Clear();
    }

    private async Task ActivateSelectedSessionAsync()
    {
        var value = SelectedSession;
        if (value is null || value.IsNewScan)
        {
            ScanService.ClearActiveSession();
            ClearAllMetrics();
            StatusMessage = "Add scan targets and click Scan to find duplicates.";
            return;
        }
        if (value.IsAborted)
        {
            ScanService.ClearActiveSession();
            ClearAllMetrics();
            StatusMessage = "This scan was aborted. Re-run to get results.";
            return;
        }
        if (!ScanService.TrySetActiveSession(value.SessionId!.Value))
        {
            ClearAllMetrics();
            StatusMessage = "Could not activate session.";
            return;
        }

        TotalFilesScanned = (int)value.FilesScanned;
        TotalDuplicateGroups = (int)value.GroupCount;
        await RefreshMetricsAsync(value.SessionId!.Value);
    }

    // ── Metrics ───────────────────────────────────────────────────────

    [ObservableProperty]
    public partial string StatusMessage { get; set; } = "Ready";

    [ObservableProperty]
    public partial int TotalFilesScanned { get; set; }

    [ObservableProperty]
    public partial int TotalDuplicateGroups { get; set; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(FormattedWastedBytes))]
    public partial long TotalWastedBytes { get; set; }

    public string FormattedWastedBytes => Converters.FileSizeConverter.FormatBytes(TotalWastedBytes);

    // ── Review Progress ───────────────────────────────────────────────

    [ObservableProperty]
    public partial int ReviewedCount { get; set; }

    [ObservableProperty]
    public partial int TotalReviewable { get; set; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ReviewProgressAccessibleName))]
    public partial double ReviewProgressPercent { get; set; }

    public string ReviewProgressAccessibleName => $"{(int)ReviewProgressPercent}% reviewed";

    // ── Quick Wins ────────────────────────────────────────────────────

    public ObservableCollection<QuickWinItem> QuickWins { get; } = new();
    public bool HasQuickWins => QuickWins.Count > 0;

    // ── Treemap nodes ─────────────────────────────────────────────────

    public ObservableCollection<TreemapNode> TreemapNodes { get; } = new();

    // ── Scan Targets ─────────────────────────────────────────────────

    public ObservableCollection<string> ScanPaths { get; } = new();

    [ObservableProperty]
    public partial string NewScanPath { get; set; } = "";

    public bool HasScanPaths => ScanPaths.Count > 0;

    [RelayCommand]
    private void AddScanPath()
    {
        var path = NewScanPath.Trim();
        if (!string.IsNullOrEmpty(path) && !ScanPaths.Contains(path))
        {
            ScanPaths.Add(path);
            NewScanPath = "";
            SaveScanPaths();
        }
    }

    [RelayCommand]
    private async Task BrowseFolderAsync()
    {
        var path = await _filePicker.PickFolderAsync();
        if (path != null && !ScanPaths.Contains(path))
        {
            ScanPaths.Add(path);
            SaveScanPaths();
        }
    }

    [RelayCommand]
    private void RemoveScanPath(string path)
    {
        ScanPaths.Remove(path);
        SaveScanPaths();
    }

    private void SaveScanPaths() => _settings.ScanPaths = ScanPaths.ToList();

    public void ReloadScanPaths()
    {
        ScanPaths.Clear();
        foreach (var p in _settings.ScanPaths) ScanPaths.Add(p);
    }

    // ── Scan command ───────────────────────────────────────────────────

    public bool CanScan => ScanService.CanScan;

    [RelayCommand(CanExecute = nameof(CanScan))]
    private async Task ScanAsync()
    {
        var paths = ScanPaths.ToArray();
        var patterns = _settings.IgnorePatterns.ToArray();
        await ScanService.StartScanAsync(paths, patterns);
    }

    // ── Commands ──────────────────────────────────────────────────────

    [RelayCommand]
    private async Task RefreshAsync()
    {
        await LoadSessionPickerAsync();
        if (SelectedSession?.SessionId.HasValue == true)
            await RefreshMetricsAsync(SelectedSession.SessionId.Value);
    }

    // ── Internal ──────────────────────────────────────────────────────

    public async Task LoadSessionPickerAsync()
    {
        var (sessions, _) = await Task.Run(() => _engine.ListSessions(0, 50));

        ScanService.DispatcherQueue?.TryEnqueue(() =>
        {
            _suppressPickerSideEffects = true;
            SessionPickerItems.Clear();
            SessionPickerItems.Add(SessionPickerItem.NewScan);

            SessionPickerItem? activeItem = null;
            foreach (var s in sessions)
            {
                var item = SessionPickerItem.FromSession(s);
                SessionPickerItems.Add(item);
                if (s.IsActive && !item.IsAborted)
                    activeItem = item;
            }

            SelectedSession = activeItem
                ?? (SessionPickerItems.Count > 1 ? SessionPickerItems[1] : SessionPickerItem.NewScan);
            _suppressPickerSideEffects = false;
            OnPropertyChanged(nameof(IsNewScanSelected));

            // Manually activate since OnSelectedSessionChanged was suppressed
            ActivateSelectedSessionAsync().FireAndForget(nameof(DashboardViewModel) + "." + nameof(LoadSessionPickerAsync));
        });
    }

    private async Task RefreshMetricsAsync(long sessionId)
    {
        // Wasted bytes from duplicate groups (SQL aggregate — no FFI marshalling)
        var (groupCount, wastedBytes) = await _db.GetSessionMetricsAsync(sessionId);
        TotalWastedBytes = wastedBytes;
        TotalDuplicateGroups = groupCount;
        StatusMessage = $"{groupCount:N0} duplicate groups, {FormattedWastedBytes} wasted";

        // Review progress
        var (reviewed, reviewTotal) = await _db.GetReviewProgressAsync(sessionId);
        ReviewedCount = reviewed;
        TotalReviewable = reviewTotal;
        ReviewProgressPercent = reviewTotal > 0 ? (double)reviewed / reviewTotal * 100 : 0;

        // Quick wins
        var wins = await _db.GetQuickWinsAsync(sessionId);
        QuickWins.Clear();
        foreach (var w in wins)
            QuickWins.Add(w);
        OnPropertyChanged(nameof(HasQuickWins));

        // Treemap nodes
        var nodes = await _db.GetTreemapNodesAsync(sessionId);
        TreemapNodes.Clear();
        foreach (var n in nodes)
            TreemapNodes.Add(n);
    }
}
