using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.NativeMethods;
using System;
using System.Collections.ObjectModel;
using System.Text.Json;
using System.Threading.Tasks;

namespace SuperDuper.ViewModels;

public partial class SessionsViewModel : ObservableObject
{
    private EngineWrapper? _engine;
    private MainViewModel? _mainViewModel;

    [ObservableProperty]
    private bool _isLoading;

    [ObservableProperty]
    private int _totalSessions;

    [ObservableProperty]
    private string _databasePath = string.Empty;

    public ObservableCollection<SessionItemViewModel> Sessions { get; } = new();

    /// <summary>
    /// Pass-through so SettingsPage can bind a toggle to the persisted MainViewModel setting.
    /// </summary>
    public bool UseTrash
    {
        get => _mainViewModel?.UseTrash ?? true;
        set { if (_mainViewModel != null) _mainViewModel.UseTrash = value; }
    }

    public void Initialize(EngineWrapper engine, MainViewModel? mainViewModel = null)
    {
        _engine = engine;
        _mainViewModel = mainViewModel;
        DatabasePath = System.IO.Path.GetFullPath("super_duper.db");
        _ = LoadSessionsAsync();
    }

    public async Task ClearScanHistoryAsync()
    {
        if (_engine is null) return;
        await Task.Run(() => _engine.TruncateDatabase());
        _mainViewModel?.OnDatabaseCleared();
        await LoadSessionsAsync();
    }

    public async Task FullResetAsync()
    {
        if (_engine is null) return;
        await Task.Run(() =>
        {
            _engine.TruncateDatabase();
            _engine.ClearHashCache();
        });
        _mainViewModel?.OnDatabaseCleared();
        await LoadSessionsAsync();
    }

    [RelayCommand]
    private async Task LoadSessionsAsync()
    {
        if (_engine is null) return;

        IsLoading = true;
        Sessions.Clear();
        try
        {
            var (sessions, total) = await Task.Run(() => _engine.ListSessions(0, 100));
            TotalSessions = total;
            foreach (var s in sessions)
                Sessions.Add(new SessionItemViewModel(s, _engine, this));
        }
        catch (Exception)
        {
            // Sessions list is informational — silently ignore errors
        }
        finally
        {
            IsLoading = false;
        }
    }

    internal void Reload() => _ = LoadSessionsAsync();
}

public partial class SessionItemViewModel : ObservableObject
{
    private readonly EngineWrapper _engine;
    private readonly SessionsViewModel _parent;

    public long Id { get; }
    public string DisplayDate { get; }
    public string RootPaths { get; }
    public long FilesScanned { get; }
    public long GroupCount { get; }
    public string Status { get; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsNotActive))]
    private bool _isActive;

    public bool IsNotActive => !IsActive;

    public SessionItemViewModel(SessionInfo info, EngineWrapper engine, SessionsViewModel parent)
    {
        _engine = engine;
        _parent = parent;
        Id = info.Id;
        DisplayDate = FormatDate(info.CompletedAt ?? info.StartedAt);
        RootPaths = FormatPaths(info.RootPaths);
        FilesScanned = info.FilesScanned;
        GroupCount = info.GroupCount;
        Status = info.Status;
        _isActive = info.IsActive;
    }

    [RelayCommand]
    private void SetActive()
    {
        _engine.SetActiveSession(Id);
        _parent.Reload();
    }

    [RelayCommand]
    private void Delete()
    {
        _engine.DeleteSession(Id);
        _parent.Reload();
    }

    private static string FormatDate(string isoDate)
    {
        return DateTime.TryParse(isoDate, out var dt)
            ? dt.ToLocalTime().ToString("g")
            : isoDate;
    }

    private static string FormatPaths(string pathsJson)
    {
        try
        {
            var paths = JsonSerializer.Deserialize<string[]>(pathsJson);
            if (paths is { Length: > 0 })
                return string.Join(", ", paths);
        }
        catch { }
        return pathsJson;
    }
}
