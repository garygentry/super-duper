using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Xaml.Media;
using SuperDuper.Models;
using SuperDuper.Services;
using SuperDuper.Services.Platform;

namespace SuperDuper.ViewModels;

/// <summary>
/// ViewModel for a single duplicate copy card in the ComparisonPane.
/// Decision changes are pushed via the DecisionChanged event so that
/// ComparisonPaneViewModel can enforce the single-Keep invariant.
/// </summary>
public partial class DuplicateCardViewModel : ObservableObject
{
    private readonly IDatabaseService _db;
    private readonly IUndoService _undo;
    private readonly IShellIntegrationService _shell;

    public long FileId { get; }
    public long GroupId { get; }
    public string CanonicalPath { get; }
    public string LastModified { get; }
    public string Created { get; }
    public string Accessed { get; }
    public bool IsNewest { get; }
    public bool IsOldest { get; }
    public string? HeuristicLabel { get; }
    public string? SiblingsText { get; }
    public SolidColorBrush DriveColorBrush { get; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(DecisionGlyph))]
    private ReviewAction? _currentDecision;

    public string DecisionGlyph => CurrentDecision switch
    {
        ReviewAction.Keep => "\uE73E",
        ReviewAction.Delete => "\uE711",
        ReviewAction.Skip => "\uE73A",
        _ => ""
    };

    /// <summary>
    /// Fired after a decision is set so ComparisonPaneViewModel can enforce single-Keep.
    /// </summary>
    public event EventHandler<(DuplicateCardViewModel Card, ReviewAction Action)>? DecisionChanged;

    public DuplicateCardViewModel(
        long fileId, long groupId, string canonicalPath,
        string lastModified, string created, string accessed,
        bool isNewest, bool isOldest,
        ReviewAction? currentDecision,
        SolidColorBrush driveColor,
        string? heuristicLabel,
        string? siblingsText,
        IDatabaseService db, IUndoService undo, IShellIntegrationService shell)
    {
        _db = db;
        _undo = undo;
        _shell = shell;
        FileId = fileId;
        GroupId = groupId;
        CanonicalPath = canonicalPath;
        LastModified = lastModified;
        Created = created;
        Accessed = accessed;
        IsNewest = isNewest;
        IsOldest = isOldest;
        CurrentDecision = currentDecision;
        DriveColorBrush = driveColor;
        HeuristicLabel = heuristicLabel;
        SiblingsText = siblingsText;
    }

    [RelayCommand]
    private async Task SetDecisionAsync(ReviewAction action)
    {
        var old = CurrentDecision;
        CurrentDecision = action;

        var undoAction = new SetDecisionAction(
            _db, FileId, GroupId, action, old, null,
            Path.GetFileName(CanonicalPath));
        _undo.Push(undoAction);
        await _db.UpsertDecisionAsync(FileId, GroupId, action);

        DecisionChanged?.Invoke(this, (this, action));
    }

    [RelayCommand]
    private void Reveal() => _shell.RevealInExplorer(CanonicalPath);

    [RelayCommand]
    private void Open() => _shell.OpenFile(CanonicalPath);
}
