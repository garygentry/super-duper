using Microsoft.UI.Xaml;
using SuperDuper.Converters;

namespace SuperDuper.Models;

/// <summary>Extended group info from direct SQLite queries (vs FFI pagination).</summary>
public class DbGroupInfo
{
    public long GroupId { get; set; }
    public long ContentHash { get; set; }
    public long FileSize { get; set; }
    public int FileCount { get; set; }
    public long WastedBytes { get; set; }
    public string SampleFileName { get; set; } = "";
}

/// <summary>Extended file info from direct SQLite queries (vs FFI pagination).</summary>
public class DbFileInfo
{
    public long FileId { get; set; }
    public string CanonicalPath { get; set; } = "";
    public string FileName { get; set; } = "";
    public string ParentDir { get; set; } = "";
    public string DriveLetter { get; set; } = "";
    public long FileSize { get; set; }
    public string LastModified { get; set; } = "";
    public long PartialHash { get; set; }
    public long ContentHash { get; set; }
    public bool IsDuplicate { get; set; }
    public int CopyCount { get; set; }
    public long GroupId { get; set; }

    public string DisplayPath => CanonicalPath.StartsWith(@"\\?\") ? CanonicalPath[4..] : CanonicalPath;
    public string Extension => Path.GetExtension(FileName).TrimStart('.').ToUpperInvariant();
    public string FormattedSize => Converters.FileSizeConverter.FormatBytes(FileSize);
    public double IsDuplicateOpacity => IsDuplicate ? 1.0 : 0.5;

    // Enriched computed properties for FileListControl
    public string FileTypeGlyph => Extension switch
    {
        "JPG" or "JPEG" or "PNG" or "GIF" or "BMP" or "WEBP" or "SVG" => "\uEB9F",
        "MP4" or "AVI" or "MKV" or "MOV" or "WMV" => "\uE714",
        "MP3" or "FLAC" or "WAV" or "AAC" or "OGG" or "WMA" => "\uE8D6",
        "PDF" => "\uEA90",
        "ZIP" or "RAR" or "7Z" or "GZ" or "TAR" => "\uF012",
        "DOC" or "DOCX" or "TXT" or "RTF" or "ODT" => "\uE8A5",
        "XLS" or "XLSX" or "CSV" => "\uE80A",
        _ => "\uE7C3"
    };
    public string FormattedDate => string.IsNullOrEmpty(LastModified) ? "" :
        DateTime.TryParse(LastModified, out var dt) ? dt.ToString("yyyy-MM-dd") : LastModified;
    public Visibility IsDuplicateVisibility => IsDuplicate ? Visibility.Visible : Visibility.Collapsed;
    public string ReviewStatusGlyph { get; set; } = "";
}

/// <summary>A high-impact duplicate group surfaced as a suggested quick action on the dashboard.</summary>
public record QuickWinItem(
    string Category,
    string Description,
    long TotalBytes,
    int ItemCount,
    object? Payload
)
{
    public string FormattedSize => Converters.FileSizeConverter.FormatBytes(TotalBytes);
    public string ActionLabel => Category switch
    {
        "Identical Directories" => "Review pair",
        "Largest Duplicate Groups" => "Review copies",
        "Single-Drive Cluster" => "Auto-resolve",
        _ => "Review"
    };
}

/// <summary>Treemap node for dashboard storage visualization.</summary>
public class TreemapNode
{
    public string Path { get; set; } = "";
    public string DisplayName { get; set; } = "";
    public long TotalBytes { get; set; }
    public double DupeDensity { get; set; }  // 0.0 – 1.0
    public int DupeCount { get; set; }
    public int TotalCount { get; set; }
}
