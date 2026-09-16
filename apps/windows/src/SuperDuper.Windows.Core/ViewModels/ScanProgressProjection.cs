using System.Globalization;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed record ScanProgressStage(
    string Name,
    ulong Files,
    string LogicalBytes,
    string AutomationId)
{
    public string FilesText => Files.ToString("N0", CultureInfo.CurrentCulture);

    public string BytesText => $"{LogicalBytes} B";

    public string AutomationName => $"{Name}: {FilesText} files; {BytesText} logical bytes";
}

public sealed record ScanPhaseWork(string Label, string Detail, double? Percent = null, bool Unknown = true)
{
    public double BarValue => Percent ?? 0;
    public string AutomationName => $"{Label}. {Detail}";
}

internal static class ScanProgressProjection
{
    internal static ScanPhaseWork PhaseWork(string? phase, WorkerScanProgressSnapshot? snapshot,
        WorkerFolderAnalysisProgress? folder)
    {
        // A lifecycle phase can arrive before its detailed snapshot. Never reuse the previous
        // phase's denominator under the new heading.
        if (phase == "discovering")
            return new("Discovering files", snapshot?.Phase == "discovering"
                ? $"{snapshot.Funnel.Discovered.Files:N0} files · {DisplayFormatting.Bytes(snapshot.Funnel.Discovered.LogicalBytes)} discovered · total unknown"
                : "Total work unknown — waiting for discovery counts");
        if (phase == "hashing" && snapshot?.Phase == "candidate_screening")
        {
            const string label = "Hash candidate work resolved";
            // remainingKnownWork is present only after the worker establishes candidate totals.
            // Zero-initialized funnel counters alone do not establish an empty phase.
            if (snapshot.RemainingKnownWork is not { Stage: "hash_pipeline" })
                return new(label, "Candidate total unknown — waiting for the total to be reported");
            var total = ulong.Parse(snapshot.Funnel.HashPipelineCandidates.LogicalBytes, CultureInfo.InvariantCulture);
            var resolved = ulong.Parse(snapshot.Logical.HashPipelineResolvedBytes, CultureInfo.InvariantCulture);
            if (resolved > total) return new(label, "Unavailable — resolved work exceeds the reported candidate denominator");
            if (total == 0) return new(label, "No logical candidate bytes in this phase · percentage not applicable", Unknown: false);
            var percent = (double)((decimal)resolved * 100 / total);
            return new(label, $"{percent:0.#}% · {DisplayFormatting.Bytes(snapshot.Logical.HashPipelineResolvedBytes)} of {DisplayFormatting.Bytes(snapshot.Funnel.HashPipelineCandidates.LogicalBytes)} logical candidate work · not disk bytes read", percent, false);
        }
        if (phase == "analyzing_folders" && snapshot?.Phase == "analyzing_folders" && folder is not null)
        {
            var label = FolderLabel(folder.Substage);
            if (folder.Total == 0) return new(label, "0 of 0 · no work in this substage · percentage not applicable", Unknown: false);
            var percent = (double)((decimal)folder.Completed * 100 / folder.Total);
            return new(label, $"{folder.Completed:N0} of {folder.Total:N0} · {percent:0.#}% of this substage", percent, false);
        }
        return new(phase == "hashing" ? "Hash candidate work resolved" : DisplayFormatting.Phase(phase),
            "Total work unknown — waiting for measurable phase work");
    }

    internal static string FolderLabel(string substage) => substage switch
    {
        "hierarchy" => "Building hierarchy",
        "structural_candidates" => "Finding structural candidates",
        "verification" => "Verifying exact content",
        "persistence" => "Saving folder results",
        _ => "Unavailable folder substage",
    };

    internal static IReadOnlyList<ScanProgressStage> Stages(WorkerCandidateFunnelProgress? funnel) =>
        funnel is null
            ? []
            :
            [
                Stage("Discovered", funnel.Discovered, "ScanStageDiscovered"),
                Stage("Resolved from metadata", funnel.MetadataResolved, "ScanStageMetadataResolved"),
                Stage("Partial screened", funnel.PartialScreened, "ScanStagePartialScreened"),
                Stage("Selected for full hash", funnel.SelectedForFullHash, "ScanStageSelectedFullHash"),
                Stage("Full hash satisfied", funnel.FullHashSatisfied, "ScanStageFullHashSatisfied"),
                Stage("Finalized duplicates", funnel.FinalizedDuplicates, "ScanStageFinalizedDuplicates"),
            ];

    internal static string CandidateContext(WorkerCandidateFunnelProgress? funnel) => funnel switch
    {
        null => "Unavailable — candidate work is not yet known",
        { HashPipelineCandidates: { } candidates } =>
            $"{candidates.Files:N0} files · {DisplayFormatting.Bytes(candidates.LogicalBytes)} candidate denominator",
    };

    internal static string PhaseElapsed(WorkerScanProgressSnapshot? snapshot) =>
        snapshot is null ? "—" : DurationFromNanos(snapshot.PhaseElapsedNanos);

    internal static string FolderAnalysis(WorkerFolderAnalysisProgress? progress) => progress switch
    {
        null => "Waiting for bounded folder-analysis progress",
        { Substage: "hierarchy" } => FolderStage("Building hierarchy", progress),
        { Substage: "structural_candidates" } => FolderStage("Finding structural candidates", progress),
        { Substage: "verification" } => FolderStage("Verifying exact content", progress),
        { Substage: "persistence" } => FolderStage("Saving folder results", progress),
        _ => "Unavailable — unsupported folder-analysis substage",
    };

    internal static string Rate(WorkerProgressRateValue? value) => value switch
    {
        null => "Unavailable — no progress sample",
        { State: "unavailable", Reason: "no_elapsed_time" } => "Unavailable — no elapsed time",
        { State: "available", Rate: { } rate } =>
            $"{Scaled(rate.FilesPerSecondMillis, 1_000)} files/s · "
            + $"{DisplayFormatting.Bytes(rate.PhysicalBytesPerSecond)}/s · "
            + $"{DurationFromNanos(rate.WindowNanos)} window",
        _ => "Unavailable — unsupported progress state",
    };

    internal static string Cache(uint? basisPoints) => basisPoints switch
    {
        null => "Unavailable — no completed cache lookups",
        var value => $"{value / 100m:0.00}% hits",
    };

    internal static string Devices(WorkerActiveDeviceProgress? devices) => devices switch
    {
        null => "Unavailable — device state missing",
        { State: "one", DeviceKey: { } key } => key,
        { State: "multiple", DeviceKeys: { } keys } => $"{keys.Count:N0} active devices",
        { State: "unavailable", Reason: "no_active_io" } => "Unavailable — no active I/O",
        { State: "unavailable", Reason: "mapping_unavailable" } =>
            "Unavailable — scan work is not mapped to a device",
        { State: "unavailable", Reason: "ambiguous" } =>
            "Unavailable — active device mapping is ambiguous",
        _ => "Unavailable — unsupported device state",
    };

    internal static string Remaining(WorkerRemainingKnownWork? remaining) => remaining switch
    {
        null => "Unavailable — work is not yet known",
        { Stage: "hash_pipeline" } =>
            $"{remaining.Files:N0} files · {DisplayFormatting.Bytes(remaining.LogicalBytes)} remaining "
            + "in hash pipeline",
        _ => "Unavailable — unsupported remaining-work stage",
    };

    internal static string EtaSummary(WorkerProgressEta? eta) => eta switch
    {
        null or { State: "unavailable", Reason: "work_not_yet_known" or "window_warming" } => "Still estimating",
        { State: "complete" } => "Complete",
        { State: "unavailable", Reason: "no_recent_progress" or "unstable_rate" } => "No reliable estimate yet",
        { State: "unavailable", Reason: "not_applicable" } => "Not estimated for this step",
        { State: "available", RemainingLogicalBytes: not null, LogicalBytesPerSecondMillis: not null,
            EstimatedSeconds: { } seconds, WindowNanos: not null } =>
            $"About {DurationFromSeconds(seconds)} for file reads",
        _ => "Not available",
    };

    internal static string Eta(WorkerProgressEta? eta) => eta switch
    {
        null => "Unavailable — ETA state missing",
        { State: "complete" } => "Complete",
        { State: "unavailable", Reason: "work_not_yet_known" } =>
            "Unavailable — work is not yet known",
        { State: "unavailable", Reason: "window_warming" } =>
            "Unavailable — collecting a stable 10-second window",
        { State: "unavailable", Reason: "no_recent_progress" } =>
            "Unavailable — no recent candidate progress",
        { State: "unavailable", Reason: "unstable_rate" } =>
            "Unavailable — recent progress rate is unstable",
        { State: "unavailable", Reason: "not_applicable" } =>
            "Unavailable — ETA does not apply to this phase",
        {
            State: "available",
            RemainingLogicalBytes: { } remaining,
            LogicalBytesPerSecondMillis: { } rate,
            EstimatedSeconds: { } seconds,
            WindowNanos: { } window,
        } =>
            $"Hash pipeline: about {DurationFromSeconds(seconds)} remaining · {DisplayFormatting.Bytes(remaining)} "
            + $"at {ScaledDecimalBytes(rate)}/s logical · {DurationFromNanos(window)} window",
        _ => "Unavailable — unsupported ETA state",
    };

    private static ScanProgressStage Stage(
        string name,
        WorkerProgressQuantity quantity,
        string automationId) =>
        new(name, quantity.Files, quantity.LogicalBytes, automationId);

    private static string FolderStage(string label, WorkerFolderAnalysisProgress progress) =>
        $"{label}: {progress.Completed:N0} of {progress.Total:N0}";

    private static string Scaled(ulong value, ulong scale) =>
        ((decimal)value / scale).ToString("0.###", CultureInfo.CurrentCulture);

    private static string ScaledDecimalBytes(string value)
    {
        var millis = ulong.Parse(value, NumberStyles.None, CultureInfo.InvariantCulture);
        var bytes = ((decimal)millis / 1_000m).ToString("0.###", CultureInfo.InvariantCulture);
        return $"{bytes} B";
    }

    private static string DurationFromNanos(ulong nanos) =>
        DurationFromSeconds(nanos / 1_000_000_000m);

    private static string DurationFromSeconds(ulong seconds) =>
        DurationFromSeconds((decimal)seconds);

    private static string DurationFromSeconds(decimal seconds)
    {
        if (seconds >= 86_400)
        {
            return $"{decimal.Floor(seconds / 86_400)}d {decimal.Floor(seconds % 86_400 / 3_600)}h {decimal.Floor(seconds % 3_600 / 60)}m";
        }
        if (seconds >= 3_600)
        {
            return $"{seconds / 3_600m:0.##} h";
        }
        if (seconds >= 60)
        {
            return $"{seconds / 60m:0.##} min";
        }
        return $"{seconds:0.##} s";
    }
}
