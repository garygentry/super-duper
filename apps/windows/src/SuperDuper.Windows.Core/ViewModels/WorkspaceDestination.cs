namespace SuperDuper.Windows.Core.ViewModels;

// Destinations are independent of the order in which WPF presents them.
public enum WorkspaceDestination
{
    ScanSetup,
    ScanProgress,
    ScanSummary,
    History,
    FileResults,
    FolderResults,
    Review,
    Performance,
}

public enum WorkspaceArea
{
    Scan,
    Results,
    Review,
    History,
}
