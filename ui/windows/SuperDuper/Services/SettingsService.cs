using SuperDuper.Models;
using System.Text.Json;

namespace SuperDuper.Services;

/// <summary>
/// Persistent app settings stored in %LocalAppData%\SuperDuper\settings.json.
/// Extends the original raw JSON reads in MainViewModel with typed properties.
/// </summary>
public class SettingsService
{
    private readonly string _settingsPath;
    private AppSettings _settings = new();

    private static readonly string DefaultDir =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "SuperDuper");

    public SettingsService()
    {
        Directory.CreateDirectory(DefaultDir);
        _settingsPath = Path.Combine(DefaultDir, "settings.json");
        Load();
    }

    // General
    public AppTheme Theme
    {
        get => _settings.Theme;
        set { _settings.Theme = value; Save(); }
    }

    public DeletionMode DefaultDeletionMode
    {
        get => _settings.DefaultDeletionMode;
        set { _settings.DefaultDeletionMode = value; Save(); }
    }

    public bool SmartSuggestionsEnabled
    {
        get => _settings.SmartSuggestionsEnabled;
        set { _settings.SmartSuggestionsEnabled = value; Save(); }
    }

    public string DeletionLogPath
    {
        get => _settings.DeletionLogPath ?? Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
            "Super Duper");
        set { _settings.DeletionLogPath = value; Save(); }
    }

    // Display
    public string DateFormat
    {
        get => _settings.DateFormat;
        set { _settings.DateFormat = value; Save(); }
    }

    public SizeDisplayMode SizeDisplayMode
    {
        get => _settings.SizeDisplayMode;
        set { _settings.SizeDisplayMode = value; Save(); }
    }

    public string DefaultGroupSort
    {
        get => _settings.DefaultGroupSort;
        set { _settings.DefaultGroupSort = value; Save(); }
    }

    // Tree annotations
    public bool ShowDensityBadges
    {
        get => _settings.ShowDensityBadges;
        set { _settings.ShowDensityBadges = value; Save(); }
    }

    public bool ShowReviewRings
    {
        get => _settings.ShowReviewRings;
        set { _settings.ShowReviewRings = value; Save(); }
    }

    public bool ShowDriveStripes
    {
        get => _settings.ShowDriveStripes;
        set { _settings.ShowDriveStripes = value; Save(); }
    }

    // Scanning defaults
    public List<string> DefaultIgnorePatterns
    {
        get => _settings.DefaultIgnorePatterns;
        set { _settings.DefaultIgnorePatterns = value; Save(); }
    }

    public long DefaultMinFileSize
    {
        get => _settings.DefaultMinFileSize;
        set { _settings.DefaultMinFileSize = value; Save(); }
    }

    public string DefaultHashAlgorithm
    {
        get => _settings.DefaultHashAlgorithm;
        set { _settings.DefaultHashAlgorithm = value; Save(); }
    }

    public int DefaultCpuThreads
    {
        get => _settings.DefaultCpuThreads > 0 ? _settings.DefaultCpuThreads : Environment.ProcessorCount;
        set { _settings.DefaultCpuThreads = value; Save(); }
    }

    public bool UseTrashForDeletion
    {
        get => _settings.UseTrashForDeletion;
        set { _settings.UseTrashForDeletion = value; Save(); }
    }

    public bool ContextMenuRegistered
    {
        get => _settings.ContextMenuRegistered;
        set { _settings.ContextMenuRegistered = value; Save(); }
    }

    // Scan paths (for the active scan configuration)
    public List<string> ScanPaths
    {
        get => _settings.ScanPaths;
        set { _settings.ScanPaths = value; Save(); }
    }

    public List<string> IgnorePatterns
    {
        get => _settings.IgnorePatterns;
        set { _settings.IgnorePatterns = value; Save(); }
    }

    private void Load()
    {
        try
        {
            if (File.Exists(_settingsPath))
            {
                var json = File.ReadAllText(_settingsPath);
                _settings = JsonSerializer.Deserialize<AppSettings>(json) ?? new AppSettings();
            }
        }
        catch
        {
            _settings = new AppSettings();
        }
    }

    private void Save()
    {
        try
        {
            var json = JsonSerializer.Serialize(_settings, new JsonSerializerOptions { WriteIndented = true });
            File.WriteAllText(_settingsPath, json);
        }
        catch { /* swallow — don't crash the app on a settings write failure */ }
    }

    private class AppSettings
    {
        public AppTheme Theme { get; set; } = AppTheme.System;
        public DeletionMode DefaultDeletionMode { get; set; } = DeletionMode.RecycleBin;
        public bool SmartSuggestionsEnabled { get; set; } = true;
        public string? DeletionLogPath { get; set; }
        public string DateFormat { get; set; } = "MMM d, yyyy";
        public SizeDisplayMode SizeDisplayMode { get; set; } = SizeDisplayMode.Decimal;
        public string DefaultGroupSort { get; set; } = "wasted_bytes";
        public bool ShowDensityBadges { get; set; } = true;
        public bool ShowReviewRings { get; set; } = true;
        public bool ShowDriveStripes { get; set; } = true;
        public List<string> DefaultIgnorePatterns { get; set; } = new();
        public long DefaultMinFileSize { get; set; } = 0;
        public string DefaultHashAlgorithm { get; set; } = "xxHash64";
        public int DefaultCpuThreads { get; set; } = 0;  // 0 = use all
        public bool UseTrashForDeletion { get; set; } = true;
        public bool ContextMenuRegistered { get; set; } = false;
        public List<string> ScanPaths { get; set; } = new();
        public List<string> IgnorePatterns { get; set; } = new();
    }
}
