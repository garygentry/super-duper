using SuperDuper.NativeMethods;
using System.Text.Json;

namespace SuperDuper.ViewModels;

public sealed class SessionPickerItem
{
    public long?    SessionId    { get; init; }   // null = "New Scan" sentinel
    public bool     IsNewScan    => SessionId is null;
    public bool     IsAborted    { get; init; }
    public string[] RootPaths   { get; init; } = [];
    public long     FilesScanned { get; init; }
    public long     GroupCount   { get; init; }
    public string   DisplayLabel { get; init; } = "";

    public static readonly SessionPickerItem NewScan = new()
    {
        SessionId    = null,
        DisplayLabel = "New Scan",
    };

    public static SessionPickerItem FromSession(SessionInfo s)
    {
        var paths   = ParseRootPaths(s.RootPaths);
        bool aborted = s.Status == "running";
        string date = DateTime.TryParse(s.CompletedAt ?? s.StartedAt, out var dt)
            ? dt.ToLocalTime().ToString("g") : (s.CompletedAt ?? s.StartedAt);
        string shortPath = paths.Length == 0 ? ""
            : Path.GetFileName(paths[0].TrimEnd('\\', '/'))
              + (paths.Length > 1 ? $" +{paths.Length - 1}" : "");

        string label = aborted
            ? $"[Aborted] {date} — {shortPath}"
            : $"{date}  •  {s.FilesScanned:N0} files  •  {s.GroupCount:N0} groups";

        return new SessionPickerItem
        {
            SessionId    = s.Id,
            IsAborted    = aborted,
            RootPaths    = paths,
            FilesScanned = s.FilesScanned,
            GroupCount   = s.GroupCount,
            DisplayLabel = label,
        };
    }

    private static string[] ParseRootPaths(string json)
    {
        try   { return JsonSerializer.Deserialize<string[]>(json) ?? []; }
        catch { return []; }
    }
}
