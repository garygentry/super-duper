namespace SuperDuper.Models;

/// <summary>
/// Canonical filter category keys shared between the UI (GroupsPage filter chips)
/// and the query layer (DatabaseService extension mapping).
/// </summary>
public static class FileTypeFilters
{
    public const string Images = "images";
    public const string Documents = "documents";
    public const string Video = "video";
    public const string Audio = "audio";
    public const string Archives = "archives";
}
