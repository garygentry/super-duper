using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.NativeMethods;
using System.Collections.ObjectModel;

namespace SuperDuper.ViewModels;

public partial class MainViewModel : ObservableObject
{
    private EngineWrapper? _engine;

    [ObservableProperty]
    private bool _isScanning;

    [ObservableProperty]
    private string _statusMessage = "Ready";

    [ObservableProperty]
    private int _totalDuplicateGroups;

    [ObservableProperty]
    private long _totalWastedBytes;

    [ObservableProperty]
    private int _totalFilesScanned;

    public ObservableCollection<DuplicateGroupInfo> DuplicateGroups { get; } = new();

    [RelayCommand]
    private async Task StartScanAsync()
    {
        if (IsScanning) return;

        try
        {
            IsScanning = true;
            StatusMessage = "Scanning...";

            _engine?.Dispose();
            _engine = new EngineWrapper();

            // Run scan on background thread
            await Task.Run(() =>
            {
                _engine.SetScanPaths(new[] { @"C:\Users" }); // TODO: configurable
                _engine.StartScan();
            });

            StatusMessage = "Scan complete. Loading results...";
            await LoadDuplicateGroupsAsync();
        }
        catch (Exception ex)
        {
            StatusMessage = $"Error: {ex.Message}";
        }
        finally
        {
            IsScanning = false;
        }
    }

    [RelayCommand]
    private async Task LoadDuplicateGroupsAsync()
    {
        if (_engine == null) return;

        DuplicateGroups.Clear();
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
