namespace SuperDuper.Models;

public enum FilterType
{
    FileType,
    Drive,
    ReviewStatus,
    SizeRange,
    TextSearch
}

public class FilterChip
{
    public FilterType FilterType { get; set; }
    public string DisplayLabel { get; set; } = string.Empty;
    public object? Value { get; set; }
}
