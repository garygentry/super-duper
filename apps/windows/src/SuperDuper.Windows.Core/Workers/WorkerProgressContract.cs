using System.Globalization;
using System.Text;

namespace SuperDuper.Windows.Core.Workers;

public static class WorkerProgressContract
{
    public const uint ProgressContractVersion = 1;
    public const uint MetricsContractVersion = 3;
    public const int MaximumActiveDevices = 64;

    private static readonly HashSet<string> LegacyPhases =
    [
        "discovering",
        "hashing",
        "persisting",
        "analyzing_folders",
        "finalizing",
    ];

    private static readonly Dictionary<string, string> TypedToLegacyPhase = new()
    {
        ["discovering"] = "discovering",
        ["candidate_screening"] = "hashing",
        ["persisting"] = "persisting",
        ["analyzing_folders"] = "analyzing_folders",
        ["finalizing"] = "finalizing",
    };

    private static readonly HashSet<string> EtaUnavailableReasons =
    [
        "work_not_yet_known",
        "window_warming",
        "no_recent_progress",
        "unstable_rate",
        "not_applicable",
    ];

    public static bool TryValidate(WorkerRunProgressEventArgs? progress, out string error)
    {
        if (progress is null)
        {
            error = "progress event is missing";
            return false;
        }
        if (progress.RunId <= 0 || progress.Sequence == 0)
        {
            error = "progress run and sequence must be positive";
            return false;
        }
        if (progress.Status is not ("running" or "cancelling"))
        {
            error = "progress status is unsupported";
            return false;
        }
        if (!LegacyPhases.Contains(progress.Phase))
        {
            error = "legacy progress phase is unsupported";
            return false;
        }
        if (progress.FilesDiscovered < 0 || progress.FilesHashed < 0 || progress.WarningCount < 0)
        {
            error = "legacy progress counters cannot be negative";
            return false;
        }
        if (!TryUnsigned(progress.BytesDiscovered, "bytesDiscovered", out var legacyBytes, out error))
        {
            return false;
        }
        var snapshot = progress.Progress;
        if (snapshot is null)
        {
            error = "typed progress snapshot is missing";
            return false;
        }
        if (snapshot.ProgressContractVersion != ProgressContractVersion
            || snapshot.MetricsContractVersion != MetricsContractVersion)
        {
            error = "progress contract version is unsupported";
            return false;
        }
        if (snapshot.Revision == 0 || snapshot.PhaseElapsedNanos > snapshot.MonotonicNanos)
        {
            error = "progress revision or phase time is invalid";
            return false;
        }
        if (!TypedToLegacyPhase.TryGetValue(snapshot.Phase, out var legacyPhase)
            || legacyPhase != progress.Phase)
        {
            error = "typed and legacy progress phases do not agree";
            return false;
        }
        if (snapshot.CacheHitRateBasisPoints > 10_000)
        {
            error = "cache hit rate exceeds 10,000 basis points";
            return false;
        }
        if (!TryGetCumulativeValues(progress, out _, out error)
            || !ValidateRates(snapshot.PartialReadRates, "partial", out error)
            || !ValidateRates(snapshot.FullReadRates, "full", out error)
            || !ValidateDevices(snapshot.ActiveDevices, out error)
            || !ValidateRemaining(snapshot.RemainingKnownWork, out error)
            || !ValidateEta(snapshot.Eta, out error))
        {
            return false;
        }

        var counters = snapshot.Counters;
        var logical = snapshot.Logical;
        if (legacyBytes != ParseKnown(counters.DiscoveredBytes)
            || counters.ZeroByteFiles > counters.DiscoveredFiles
            || (ulong)progress.FilesDiscovered != counters.DiscoveredFiles - counters.ZeroByteFiles
            || (ulong)progress.FilesHashed != counters.PartialHashesSucceeded
            || (ulong)progress.WarningCount != snapshot.WarningCount
            || snapshot.WarningCount != counters.Warnings)
        {
            error = "legacy and typed progress counters do not agree";
            return false;
        }
        if (!CheckedSumAtMost(counters.ZeroByteFiles, counters.HardLinkAliasFiles, counters.DiscoveredFiles)
            || !CheckedSumAtMost(
                counters.PartialHashesSucceeded,
                counters.PartialHashesFailed,
                counters.PartialHashesAttempted)
            || !CheckedSumAtMost(
                counters.PartialHashCacheHits,
                counters.PartialHashCacheMisses,
                counters.PartialHashCacheErrors,
                counters.PartialHashesAttempted)
            || counters.PartialHashCacheStores > counters.PartialHashesSucceeded
            || !CheckedSumAtMost(
                counters.FullHashCacheHits,
                counters.FullHashCacheMisses,
                counters.FullHashCacheErrors,
                counters.FullHashRequests)
            || !CheckedSumAtMost(
                counters.FullHashContentReadsCompleted,
                counters.FullHashContentReadsFailed,
                counters.FullHashContentReadsStarted)
            || !CheckedSumEquals(
                counters.PartialHashesSucceeded,
                counters.PartialHashesFailed,
                logical.PartialScreenedFiles)
            || !CheckedSumEquals(
                counters.FullHashCacheHits,
                counters.FullHashContentReadsCompleted,
                logical.FullHashSatisfiedFiles)
            || !CheckedSumAtMost(
                logical.FullHashSatisfiedFiles,
                logical.FullHashFailedFiles,
                counters.FullHashRequests))
        {
            error = "typed progress counter invariants failed";
            return false;
        }
        if (!FunnelMatchesSnapshot(snapshot, out error))
        {
            return false;
        }

        error = string.Empty;
        return true;
    }

    public static bool TryGetCumulativeValues(
        WorkerRunProgressEventArgs progress,
        out IReadOnlyList<ulong> values,
        out string error)
    {
        var snapshot = progress.Progress;
        if (snapshot?.Counters is null || snapshot.Logical is null || snapshot.Funnel is null)
        {
            values = [];
            error = "typed progress objects are missing";
            return false;
        }
        var counters = snapshot.Counters;
        var logical = snapshot.Logical;
        var funnel = snapshot.Funnel;
        var result = new List<ulong>(72) { snapshot.MonotonicNanos };
        if (!AddCounters(result, counters, out error)
            || !AddLogical(result, logical, out error)
            || !AddQuantity(result, funnel.Discovered, "funnel.discovered", out error)
            || !AddQuantity(result, funnel.MetadataResolved, "funnel.metadataResolved", out error)
            || !AddQuantity(result, funnel.HashPipelineCandidates, "funnel.hashPipelineCandidates", out error)
            || !AddQuantity(result, funnel.PartialScreened, "funnel.partialScreened", out error)
            || !AddQuantity(result, funnel.SelectedForFullHash, "funnel.selectedForFullHash", out error)
            || !AddQuantity(result, funnel.FullHashSatisfied, "funnel.fullHashSatisfied", out error)
            || !AddQuantity(result, funnel.FinalizedDuplicates, "funnel.finalizedDuplicates", out error))
        {
            values = [];
            return false;
        }
        result.Add(snapshot.WarningCount);
        values = result;
        error = string.Empty;
        return true;
    }

    private static bool AddCounters(
        List<ulong> values,
        WorkerScanProgressCounters counters,
        out string error)
    {
        values.Add(counters.DiscoveredFiles);
        if (!AddUnsigned(values, counters.DiscoveredBytes, "counters.discoveredBytes", out error)) return false;
        values.Add(counters.ZeroByteFiles);
        values.Add(counters.HardLinkAliasFiles);
        if (!AddUnsigned(values, counters.HardLinkAliasBytes, "counters.hardLinkAliasBytes", out error)) return false;
        values.Add(counters.SizeBuckets);
        values.Add(counters.SingletonSizeBuckets);
        values.Add(counters.SingletonSizeFiles);
        if (!AddUnsigned(values, counters.SingletonSizeBytes, "counters.singletonSizeBytes", out error)) return false;
        values.Add(counters.CandidateSizeBuckets);
        values.Add(counters.CandidateFiles);
        if (!AddUnsigned(values, counters.CandidateBytes, "counters.candidateBytes", out error)) return false;
        values.Add(counters.DuplicateCandidateSizeBuckets);
        values.Add(counters.DuplicateCandidateFiles);
        if (!AddUnsigned(values, counters.DuplicateCandidateBytes, "counters.duplicateCandidateBytes", out error)) return false;
        values.Add(counters.MetadataResolvedFiles);
        if (!AddUnsigned(values, counters.MetadataResolvedBytes, "counters.metadataResolvedBytes", out error)) return false;
        values.Add(counters.PartialHashesAttempted);
        values.Add(counters.PartialHashesSucceeded);
        values.Add(counters.PartialHashesFailed);
        if (!AddUnsigned(values, counters.PartialHashBytesRead, "counters.partialHashBytesRead", out error)) return false;
        values.Add(counters.PartialHashCacheHits);
        values.Add(counters.PartialHashCacheMisses);
        values.Add(counters.PartialHashCacheErrors);
        values.Add(counters.PartialHashCacheStores);
        values.Add(counters.PartialCollisionBuckets);
        values.Add(counters.PartialCollisionFiles);
        if (!AddUnsigned(values, counters.PartialCollisionBytes, "counters.partialCollisionBytes", out error)) return false;
        values.Add(counters.FullHashRequests);
        values.Add(counters.FullHashCacheHits);
        values.Add(counters.FullHashCacheMisses);
        values.Add(counters.FullHashCacheErrors);
        values.Add(counters.FullHashCacheStores);
        values.Add(counters.FullHashContentReadsStarted);
        values.Add(counters.FullHashContentReadsCompleted);
        values.Add(counters.FullHashContentReadsFailed);
        if (!AddUnsigned(values, counters.FullHashBytesRead, "counters.fullHashBytesRead", out error)) return false;
        values.Add(counters.ConfirmedDuplicateGroups);
        values.Add(counters.ConfirmedLogicalCopies);
        values.Add(counters.ConfirmedPhysicalItems);
        if (!AddUnsigned(values, counters.RecoverableBytes, "counters.recoverableBytes", out error)) return false;
        values.Add(counters.Warnings);
        values.Add(counters.CancelChecks);
        values.Add(counters.CancelledWorkItems);
        values.Add(counters.TelemetrySamplesLost);
        values.Add(counters.TelemetryFlushErrors);
        values.Add(counters.UnavailableCounters);
        error = string.Empty;
        return true;
    }

    private static bool AddLogical(
        List<ulong> values,
        WorkerProgressLogicalCounters logical,
        out string error)
    {
        values.Add(logical.PartialScreenedFiles);
        if (!AddUnsigned(values, logical.PartialScreenedBytes, "logical.partialScreenedBytes", out error)) return false;
        if (!AddUnsigned(values, logical.FullHashRequestBytes, "logical.fullHashRequestBytes", out error)) return false;
        values.Add(logical.FullHashSatisfiedFiles);
        if (!AddUnsigned(values, logical.FullHashSatisfiedBytes, "logical.fullHashSatisfiedBytes", out error)) return false;
        values.Add(logical.FullHashFailedFiles);
        if (!AddUnsigned(values, logical.FullHashFailedBytes, "logical.fullHashFailedBytes", out error)) return false;
        values.Add(logical.HashPipelineResolvedFiles);
        if (!AddUnsigned(values, logical.HashPipelineResolvedBytes, "logical.hashPipelineResolvedBytes", out error)) return false;
        if (!AddUnsigned(values, logical.ConfirmedLogicalBytes, "logical.confirmedLogicalBytes", out error)) return false;
        error = string.Empty;
        return true;
    }

    private static bool AddQuantity(
        List<ulong> values,
        WorkerProgressQuantity? quantity,
        string name,
        out string error)
    {
        if (quantity is null)
        {
            error = $"{name} is missing";
            return false;
        }
        values.Add(quantity.Files);
        return AddUnsigned(values, quantity.LogicalBytes, $"{name}.logicalBytes", out error);
    }

    private static bool ValidateRates(WorkerProgressRates? rates, string name, out string error)
    {
        error = string.Empty;
        if (rates is null
            || !ValidateRateValue(rates.Cumulative, $"{name}.cumulative", out error)
            || !ValidateRateValue(rates.Recent, $"{name}.recent", out error))
        {
            if (rates is null) error = $"{name} rates are missing";
            return false;
        }
        return true;
    }

    private static bool ValidateRateValue(
        WorkerProgressRateValue? value,
        string name,
        out string error)
    {
        error = string.Empty;
        if (value is null)
        {
            error = $"{name} rate is missing";
            return false;
        }
        if (value.State == "unavailable")
        {
            if (value.Reason != "no_elapsed_time" || value.Rate is not null)
            {
                error = $"{name} unavailable rate is invalid";
                return false;
            }
            error = string.Empty;
            return true;
        }
        if (value.State != "available" || value.Rate is null || value.Reason is not null
            || value.Rate.WindowNanos == 0
            || !TryUnsigned(
                value.Rate.PhysicalBytesPerSecond,
                $"{name}.physicalBytesPerSecond",
                out _,
                out error))
        {
            if (string.IsNullOrEmpty(error)) error = $"{name} available rate is invalid";
            return false;
        }
        error = string.Empty;
        return true;
    }

    private static bool ValidateDevices(WorkerActiveDeviceProgress? devices, out string error)
    {
        if (devices is null)
        {
            error = "active device state is missing";
            return false;
        }
        if (devices.State == "unavailable")
        {
            if (devices.Reason is not ("no_active_io" or "mapping_unavailable" or "ambiguous")
                || devices.DeviceKey is not null
                || devices.DeviceKeys is not null)
            {
                error = "unavailable active device state is invalid";
                return false;
            }
            error = string.Empty;
            return true;
        }
        if (devices.State == "one")
        {
            if (!ValidDeviceKey(devices.DeviceKey) || devices.Reason is not null || devices.DeviceKeys is not null)
            {
                error = "single active device state is invalid";
                return false;
            }
            error = string.Empty;
            return true;
        }
        if (devices.State == "multiple"
            && devices.Reason is null
            && devices.DeviceKey is null
            && devices.DeviceKeys is { Count: >= 2 and <= MaximumActiveDevices } keys
            && keys.All(ValidDeviceKey)
            && keys.Distinct(StringComparer.Ordinal).Count() == keys.Count)
        {
            error = string.Empty;
            return true;
        }
        error = "multiple active device state is invalid";
        return false;
    }

    private static bool ValidateRemaining(WorkerRemainingKnownWork? remaining, out string error)
    {
        error = string.Empty;
        if (remaining is null)
        {
            error = string.Empty;
            return true;
        }
        if (remaining.Stage != "hash_pipeline"
            || !TryUnsigned(remaining.LogicalBytes, "remainingKnownWork.logicalBytes", out _, out error))
        {
            if (string.IsNullOrEmpty(error)) error = "remaining work stage is unsupported";
            return false;
        }
        error = string.Empty;
        return true;
    }

    private static bool ValidateEta(WorkerProgressEta? eta, out string error)
    {
        error = string.Empty;
        if (eta is null)
        {
            error = "ETA state is missing";
            return false;
        }
        if (eta.State == "complete")
        {
            if (eta.Reason is null && eta.Stage is null && eta.RemainingLogicalBytes is null
                && eta.LogicalBytesPerSecondMillis is null && eta.EstimatedSeconds is null
                && eta.WindowNanos is null)
            {
                error = string.Empty;
                return true;
            }
            error = "complete ETA contains variant fields";
            return false;
        }
        if (eta.State == "unavailable")
        {
            if (eta.Reason is not null && EtaUnavailableReasons.Contains(eta.Reason)
                && eta.Stage is null && eta.RemainingLogicalBytes is null
                && eta.LogicalBytesPerSecondMillis is null && eta.EstimatedSeconds is null
                && eta.WindowNanos is null)
            {
                error = string.Empty;
                return true;
            }
            error = "unavailable ETA is invalid";
            return false;
        }
        if (eta.State != "available" || eta.Reason is not null || eta.Stage != "hash_pipeline"
            || eta.EstimatedSeconds is null || eta.WindowNanos is null or 0
            || !TryUnsigned(eta.RemainingLogicalBytes, "eta.remaining_logical_bytes", out _, out error)
            || !TryUnsigned(
                eta.LogicalBytesPerSecondMillis,
                "eta.logical_bytes_per_second_millis",
                out var rate,
                out error)
            || rate == 0)
        {
            if (string.IsNullOrEmpty(error)) error = "available ETA is invalid";
            return false;
        }
        error = string.Empty;
        return true;
    }

    private static bool FunnelMatchesSnapshot(WorkerScanProgressSnapshot snapshot, out string error)
    {
        var counters = snapshot.Counters;
        var logical = snapshot.Logical;
        var funnel = snapshot.Funnel;
        if (!QuantityMatches(funnel.Discovered, counters.DiscoveredFiles, counters.DiscoveredBytes)
            || !QuantityMatches(funnel.MetadataResolved, counters.MetadataResolvedFiles, counters.MetadataResolvedBytes)
            || !QuantityMatches(funnel.HashPipelineCandidates, counters.CandidateFiles, counters.CandidateBytes)
            || !QuantityMatches(funnel.PartialScreened, logical.PartialScreenedFiles, logical.PartialScreenedBytes)
            || !QuantityMatches(funnel.SelectedForFullHash, counters.FullHashRequests, logical.FullHashRequestBytes)
            || !QuantityMatches(funnel.FullHashSatisfied, logical.FullHashSatisfiedFiles, logical.FullHashSatisfiedBytes)
            || !QuantityMatches(funnel.FinalizedDuplicates, counters.ConfirmedLogicalCopies, logical.ConfirmedLogicalBytes))
        {
            error = "progress funnel does not match cumulative source counters";
            return false;
        }
        error = string.Empty;
        return true;
    }

    private static bool QuantityMatches(WorkerProgressQuantity quantity, ulong files, string bytes) =>
        quantity.Files == files && quantity.LogicalBytes == bytes;

    private static bool AddUnsigned(
        List<ulong> values,
        string? value,
        string name,
        out string error)
    {
        if (!TryUnsigned(value, name, out var parsed, out error))
        {
            return false;
        }
        values.Add(parsed);
        return true;
    }

    private static bool TryUnsigned(
        string? value,
        string name,
        out ulong parsed,
        out string error)
    {
        parsed = 0;
        if (string.IsNullOrEmpty(value)
            || value.Any(character => character is < '0' or > '9')
            || (value.Length > 1 && value[0] == '0')
            || !ulong.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture, out parsed))
        {
            error = $"{name} is not an unsigned decimal string";
            return false;
        }
        error = string.Empty;
        return true;
    }

    private static ulong ParseKnown(string value) =>
        ulong.Parse(value, NumberStyles.None, CultureInfo.InvariantCulture);

    private static bool CheckedSumAtMost(ulong left, ulong right, ulong maximum) =>
        left <= maximum && right <= maximum - left;

    private static bool CheckedSumEquals(ulong left, ulong right, ulong expected) =>
        left <= expected && right == expected - left;

    private static bool CheckedSumAtMost(ulong first, ulong second, ulong third, ulong maximum) =>
        first <= maximum && second <= maximum - first && third <= maximum - first - second;

    private static bool ValidDeviceKey(string? key) =>
        !string.IsNullOrWhiteSpace(key) && Encoding.UTF8.GetByteCount(key) <= 256;
}
