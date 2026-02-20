namespace SuperDuper.Models;

public class ScanProfile
{
    public string Id { get; set; } = Guid.NewGuid().ToString();
    public string Name { get; set; } = string.Empty;
    public List<string> RootPaths { get; set; } = new();
    public List<string> IgnorePatterns { get; set; } = new();
    public long MinFileSize { get; set; } = 0;
    public string HashAlgorithm { get; set; } = "xxHash64";
    public bool IncludeHiddenFiles { get; set; } = false;
    public int CpuThreads { get; set; } = Environment.ProcessorCount;
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
    public DateTime UpdatedAt { get; set; } = DateTime.UtcNow;
}
