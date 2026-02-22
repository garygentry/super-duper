using CommunityToolkit.Mvvm.ComponentModel;
using Microsoft.UI.Dispatching;
using SuperDuper.NativeMethods;
using System.Runtime.InteropServices;

namespace SuperDuper.Services;

/// <summary>
/// Centralized singleton owning scan execution, progress state, and active session.
/// All ViewModels and pages bind to this instead of managing scan state independently.
/// Replaces SessionStateService and absorbs scan execution from ScanDialogViewModel.
/// </summary>
public partial class ScanService : ObservableObject
{
    private readonly EngineWrapper _engine;
    private readonly SettingsService _settings;
    private bool _hasScanPaths;

    public ScanService(EngineWrapper engine, SettingsService settings)
    {
        _engine = engine;
        _settings = settings;
    }

    // ── Dispatcher ─────────────────────────────────────────────────────

    public DispatcherQueue? DispatcherQueue { get; private set; }

    public void SetDispatcherQueue(DispatcherQueue queue) => DispatcherQueue = queue;

    // ── Active session ─────────────────────────────────────────────────

    [ObservableProperty]
    public partial long? ActiveSessionId { get; set; }

    public event EventHandler<long?>? ActiveSessionChanged;

    partial void OnActiveSessionIdChanged(long? value)
        => ActiveSessionChanged?.Invoke(this, value);

    public bool TrySetActiveSession(long sessionId)
    {
        try
        {
            _engine.SetActiveSession(sessionId);
            ActiveSessionId = sessionId;
            return true;
        }
        catch { return false; }
    }

    // Overload accepting engine for backwards compat (callers that already have it)
    public bool TrySetActiveSession(EngineWrapper engine, long sessionId)
        => TrySetActiveSession(sessionId);

    public void ClearActiveSession() => ActiveSessionId = null;

    // ── Scan state ─────────────────────────────────────────────────────

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsNotScanning), nameof(CanScan))]
    public partial bool IsScanning { get; set; }

    public bool IsNotScanning => !IsScanning;

    public bool CanScan => !IsScanning && _hasScanPaths;

    public void UpdateCanScan(bool hasPaths)
    {
        _hasScanPaths = hasPaths;
        OnPropertyChanged(nameof(CanScan));
    }

    // ── Progress properties ────────────────────────────────────────────

    [ObservableProperty]
    public partial string ScanPhaseLabel { get; set; } = "";

    [ObservableProperty]
    public partial string ScanCountLabel { get; set; } = "";

    [ObservableProperty]
    public partial double ScanProgressMax { get; set; } = 1;

    [ObservableProperty]
    public partial double ScanProgressValue { get; set; }

    [ObservableProperty]
    public partial bool ScanProgressIndeterminate { get; set; } = true;

    [ObservableProperty]
    public partial string CurrentFilePath { get; set; } = "";

    // ── Events ─────────────────────────────────────────────────────────

    public event EventHandler? ScanCompleted;
    public event EventHandler<string>? ScanError;

    // ── Scan execution ─────────────────────────────────────────────────

    public async Task StartScanAsync(string[] paths, string[] ignorePatterns)
    {
        if (IsScanning || paths.Length == 0) return;

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

                DispatcherQueue?.TryEnqueue(() =>
                {
                    ScanPhaseLabel = phaseLabel;
                    ScanCountLabel = countLabel;
                    ScanProgressIndeterminate = indeterminate;
                    ScanProgressMax = max;
                    ScanProgressValue = value;
                    if (!string.IsNullOrEmpty(filePath)) CurrentFilePath = filePath;
                });
            });

            await Task.Run(() =>
            {
                _engine.SetScanPaths(paths);
                if (ignorePatterns.Length > 0)
                    _engine.SetIgnorePatterns(ignorePatterns);
                _engine.StartScan();
            });

            _engine.ClearProgressCallback();

            // Save paths to settings for next time
            _settings.ScanPaths = paths.ToList();
            _settings.IgnorePatterns = ignorePatterns.ToList();

            ScanCompleted?.Invoke(this, EventArgs.Empty);
        }
        catch (Exception ex)
        {
            if (!ex.Message.Contains("Cancelled"))
                ScanError?.Invoke(this, ex.Message);
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

    public void CancelScan() => _engine.CancelScan();
}
