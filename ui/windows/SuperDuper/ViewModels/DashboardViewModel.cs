using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Dispatching;
using SuperDuper.Models;
using SuperDuper.NativeMethods;
using SuperDuper.Services;
using System.Collections.ObjectModel;
using System.Text.Json;

namespace SuperDuper.ViewModels;

/// <summary>
/// Drives the Dashboard page. Owns session selection, metrics display,
/// review progress, and quick wins. Scan initiation delegates to ScanDialogViewModel.
/// </summary>
public partial class DashboardViewModel : ObservableObject
{
    private readonly EngineWrapper _engine;
    private readonly IDatabaseService _db;
    private bool _suppressPickerSideEffects;
    private DispatcherQueue? _dispatcherQueue;

    public DashboardViewModel(EngineWrapper engine, IDatabaseService db)
    {
        _engine = engine;
        _db = db;
        _ = LoadSessionPickerAsync();
    }

    public void SetDispatcherQueue(DispatcherQueue queue)
    {
        _dispatcherQueue = queue;
    }

    // ── Session picker ────────────────────────────────────────────────

    public ObservableCollection<SessionPickerItem> SessionPickerItems { get; } = new();

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsNewScanSelected))]
    private SessionPickerItem? _selectedSession;

    public bool IsNewScanSelected => SelectedSession?.IsNewScan ?? true;

    partial void OnSelectedSessionChanged(SessionPickerItem? value)
    {
        if (_suppressPickerSideEffects) return;
        if (value is null || value.IsNewScan)
        {
            TotalFilesScanned = 0;
            TotalDuplicateGroups = 0;
            TotalWastedBytes = 0;
            StatusMessage = "Click \"New Scan\" to scan for duplicates.";
            return;
        }
        if (value.IsAborted)
        {
            StatusMessage = "This scan was aborted. Re-run to get results.";
            return;
        }
        try { _engine.SetActiveSession(value.SessionId!.Value); }
        catch (Exception ex) { StatusMessage = $"Could not activate session: {ex.Message}"; return; }

        TotalFilesScanned = (int)value.FilesScanned;
        TotalDuplicateGroups = (int)value.GroupCount;
        _ = RefreshMetricsAsync(value.SessionId!.Value);
    }

    // ── Metrics ───────────────────────────────────────────────────────

    [ObservableProperty]
    private string _statusMessage = "Ready";

    [ObservableProperty]
    private int _totalFilesScanned;

    [ObservableProperty]
    private int _totalDuplicateGroups;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(FormattedWastedBytes))]
    private long _totalWastedBytes;

    public string FormattedWastedBytes => Converters.FileSizeConverter.FormatBytes(TotalWastedBytes);

    // ── Review Progress ───────────────────────────────────────────────

    [ObservableProperty]
    private int _reviewedCount;

    [ObservableProperty]
    private int _totalReviewable;

    [ObservableProperty]
    private double _reviewProgressPercent;

    // ── Quick Wins ────────────────────────────────────────────────────

    public ObservableCollection<QuickWinItem> QuickWins { get; } = new();
    public bool HasQuickWins => QuickWins.Count > 0;

    // ── Treemap nodes ─────────────────────────────────────────────────

    public ObservableCollection<TreemapNode> TreemapNodes { get; } = new();

    // ── Commands ──────────────────────────────────────────────────────

    [RelayCommand]
    private void OpenNewScanDialog()
    {
        NewScanDialogRequested?.Invoke(this, EventArgs.Empty);
    }

    public event EventHandler? NewScanDialogRequested;

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

        _dispatcherQueue?.TryEnqueue(() =>
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
        });
    }

    private async Task RefreshMetricsAsync(long sessionId)
    {
        // Wasted bytes from duplicate groups
        var (groups, total) = await Task.Run(() => _engine.QueryDuplicateGroups(0, 500));
        TotalWastedBytes = groups.Sum(g => g.WastedBytes);
        TotalDuplicateGroups = total;
        StatusMessage = $"{total:N0} duplicate groups, {FormattedWastedBytes} wasted";

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
    }
}
