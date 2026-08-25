using System.Globalization;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed record ScanProgressStage(
    string Name,
    ulong Files,
    string LogicalBytes)
{
    public string FilesText => Files.ToString("N0", CultureInfo.CurrentCulture);

    public string BytesText => DisplayFormatting.Bytes(LogicalBytes);
}

internal static class ScanProgressProjection
{
    internal static IReadOnlyList<ScanProgressStage> Stages(WorkerCandidateFunnelProgress? funnel) =>
        funnel is null
            ? []
            :
            [
                Stage("Discovered", funnel.Discovered),
                Stage("Resolved from metadata", funnel.MetadataResolved),
                Stage("Hash candidates", funnel.HashPipelineCandidates),
                Stage("Partial screened", funnel.PartialScreened),
                Stage("Selected for full hash", funnel.SelectedForFullHash),
                Stage("Full hash satisfied", funnel.FullHashSatisfied),
                Stage("Finalized duplicates", funnel.FinalizedDuplicates),
            ];

    internal static string PhaseElapsed(WorkerScanProgressSnapshot? snapshot) =>
        snapshot is null ? "—" : DurationFromNanos(snapshot.PhaseElapsedNanos);

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
            $"About {DurationFromSeconds(seconds)} remaining · {DisplayFormatting.Bytes(remaining)} "
            + $"at {ScaledDecimalBytes(rate)}/s logical · {DurationFromNanos(window)} window",
        _ => "Unavailable — unsupported ETA state",
    };

    private static ScanProgressStage Stage(string name, WorkerProgressQuantity quantity) =>
        new(name, quantity.Files, quantity.LogicalBytes);

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
