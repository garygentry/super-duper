using CommunityToolkit.Mvvm.ComponentModel;
using SuperDuper.Models;
using SuperDuper.NativeMethods;
using SuperDuper.Services;
using SuperDuper.Services.Platform;
using System.Collections.ObjectModel;

namespace SuperDuper.ViewModels;

/// <summary>
/// Manages the right-hand ComparisonPane for a selected file's duplicate group.
/// Enforces the single-Keep invariant: only one card may be in the Keep state at a time.
/// </summary>
public partial class ComparisonPaneViewModel : ObservableObject
{
    private readonly IDatabaseService _db;
    private readonly IUndoService _undo;
    private readonly IShellIntegrationService _shell;
    private readonly DriveColorService _driveColors;
    private readonly SuggestionEngine _suggestions;

    public ObservableCollection<DuplicateCardViewModel> Cards { get; } = new();

    [ObservableProperty]
    private bool _isEmpty = true;

    [ObservableProperty]
    private string? _suggestionMessage;

    public ComparisonPaneViewModel(
        IDatabaseService db, IUndoService undo, IShellIntegrationService shell,
        DriveColorService driveColors, SuggestionEngine suggestions)
    {
        _db = db;
        _undo = undo;
        _shell = shell;
        _driveColors = driveColors;
        _suggestions = suggestions;
    }

    public async Task LoadForFileAsync(EngineWrapper engine, long groupId, long sessionId)
    {
        Cards.Clear();
        IsEmpty = true;
        SuggestionMessage = null;

        var files = await Task.Run(() => engine.QueryFilesInGroup(groupId));
        if (files.Count == 0) return;

        // Build DbFileInfo wrappers for SuggestionEngine (CanonicalPath is sufficient for heuristics)
        var dbInfos = files.Select(f => new DbFileInfo
        {
            FileId = f.Id,
            CanonicalPath = f.CanonicalPath,
            FileName = f.FileName,
            FileSize = f.FileSize,
        }).ToList();

        var suggestion = _suggestions.Suggest(dbInfos);

        foreach (var f in files)
        {
            var decision = await _db.GetDecisionAsync(f.Id);
            var siblings = await _db.GetSiblingContextAsync(f.Id);

            // Extract drive letter from path (e.g. C:\... → 'C')
            var driveLetter = f.CanonicalPath.Length >= 2 && f.CanonicalPath[1] == ':' ?
                f.CanonicalPath[0] : 'C';
            var driveColor = new Microsoft.UI.Xaml.Media.SolidColorBrush(
                _driveColors.GetColor(driveLetter));

            string? heuristicLabel = null;
            if (suggestion != null)
            {
                if (f.Id == suggestion.SuggestedKeepFileId)
                    heuristicLabel = $"Suggested: Keep — {suggestion.HeuristicLabel}";
                else if (f.Id == suggestion.SuggestedDeleteFileId)
                    heuristicLabel = $"Suggested: Delete — {suggestion.HeuristicLabel}";
            }

            var siblingsText = siblings.SiblingDupeCount > 0
                ? $"{siblings.SiblingDupeCount} of {siblings.SiblingTotalCount} siblings also duplicated"
                : null;

            var card = new DuplicateCardViewModel(
                f.Id, groupId, f.CanonicalPath,
                lastModified: "", created: "", accessed: "",
                isNewest: false, isOldest: false,
                decision, driveColor,
                heuristicLabel, siblingsText,
                _db, _undo, _shell);

            card.DecisionChanged += OnCardDecisionChanged;
            Cards.Add(card);
        }

        IsEmpty = false;

        if (suggestion != null)
        {
            var keepFile = files.FirstOrDefault(f => f.Id == suggestion.SuggestedKeepFileId);
            SuggestionMessage = keepFile != null
                ? $"Suggestion: keep {Path.GetFileName(keepFile.CanonicalPath)} — {suggestion.Reason}"
                : null;
        }
    }

    private void OnCardDecisionChanged(object? sender, (DuplicateCardViewModel Card, ReviewAction Action) e)
    {
        // Enforce single-Keep invariant: clear Keep from all other cards
        if (e.Action == ReviewAction.Keep)
        {
            foreach (var card in Cards)
            {
                if (card != e.Card && card.CurrentDecision == ReviewAction.Keep)
                    card.CurrentDecision = ReviewAction.Skip;
            }
        }
    }
}
