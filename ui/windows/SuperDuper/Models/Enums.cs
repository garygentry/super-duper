namespace SuperDuper.Models;

public enum ReviewStatus
{
    Unreviewed,
    Partial,
    Decided
}

public enum ReviewAction
{
    Keep,
    Delete,
    Skip
}

public enum DensityLevel
{
    Low,
    Medium,
    High
}

public enum ScanPhase
{
    Idle,
    Scanning,
    Hashing,
    WritingDatabase,
    Analyzing,
    Complete
}

public enum AppTheme
{
    System,
    Light,
    Dark
}

public enum SizeDisplayMode
{
    Decimal,  // GB (1000^3)
    Binary    // GiB (1024^3)
}

public enum DeletionMode
{
    RecycleBin,
    Permanent
}
