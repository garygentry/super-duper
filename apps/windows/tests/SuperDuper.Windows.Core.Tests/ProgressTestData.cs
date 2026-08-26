using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

internal static class ProgressTestData
{
    internal static WorkerRunProgressEventArgs Discovery(
        long runId = 1,
        ulong sequence = 1,
        ulong revision = 1,
        ulong discoveredFiles = 1,
        ulong zeroByteFiles = 0,
        ulong monotonicNanos = 1_000_000_000,
        string status = "running",
        ulong warningCount = 0)
    {
        var nonEmptyFiles = discoveredFiles - zeroByteFiles;
        var discoveredBytes = (nonEmptyFiles * 100).ToString();
        var counters = EmptyCounters() with
        {
            DiscoveredFiles = discoveredFiles,
            DiscoveredBytes = discoveredBytes,
            ZeroByteFiles = zeroByteFiles,
            Warnings = warningCount,
        };
        var logical = EmptyLogical();
        return new WorkerRunProgressEventArgs
        {
            RunId = runId,
            Sequence = sequence,
            Status = status,
            Phase = "discovering",
            FilesDiscovered = checked((long)nonEmptyFiles),
            BytesDiscovered = discoveredBytes,
            FilesHashed = 0,
            WarningCount = checked((long)warningCount),
            Progress = new WorkerScanProgressSnapshot
            {
                ProgressContractVersion = WorkerProgressContract.ProgressContractVersion,
                MetricsContractVersion = WorkerProgressContract.MetricsContractVersion,
                Revision = revision,
                MonotonicNanos = monotonicNanos,
                Phase = "discovering",
                PhaseElapsedNanos = monotonicNanos,
                Counters = counters,
                Logical = logical,
                Funnel = Funnel(counters, logical),
                PartialReadRates = UnavailableRates(),
                FullReadRates = UnavailableRates(),
                CacheHitRateBasisPoints = null,
                WarningCount = warningCount,
                ActiveDevices = new WorkerActiveDeviceProgress
                {
                    State = "unavailable",
                    Reason = "mapping_unavailable",
                },
                RemainingKnownWork = null,
                Eta = new WorkerProgressEta
                {
                    State = "unavailable",
                    Reason = "work_not_yet_known",
                },
            },
        };
    }

    internal static WorkerRunProgressEventArgs Hashing(
        long runId = 1,
        ulong sequence = 1,
        ulong revision = 1,
        string status = "running")
    {
        var counters = EmptyCounters() with
        {
            DiscoveredFiles = 10,
            DiscoveredBytes = "10000",
            SizeBuckets = 3,
            SingletonSizeBuckets = 2,
            SingletonSizeFiles = 2,
            SingletonSizeBytes = "2000",
            CandidateSizeBuckets = 1,
            CandidateFiles = 8,
            CandidateBytes = "8000",
            DuplicateCandidateSizeBuckets = 1,
            DuplicateCandidateFiles = 8,
            DuplicateCandidateBytes = "8000",
            PartialHashesAttempted = 4,
            PartialHashesSucceeded = 4,
            PartialHashBytesRead = "400",
            PartialCollisionBuckets = 1,
            PartialCollisionFiles = 2,
            PartialCollisionBytes = "2000",
            FullHashRequests = 2,
            FullHashCacheHits = 1,
            FullHashCacheMisses = 1,
            FullHashCacheStores = 1,
            FullHashContentReadsStarted = 1,
            FullHashContentReadsCompleted = 1,
            FullHashBytesRead = "1000",
            ConfirmedDuplicateGroups = 1,
            ConfirmedLogicalCopies = 2,
            ConfirmedPhysicalItems = 2,
            RecoverableBytes = "1000",
        };
        var logical = EmptyLogical() with
        {
            PartialScreenedFiles = 4,
            PartialScreenedBytes = "4000",
            FullHashRequestBytes = "2000",
            FullHashSatisfiedFiles = 2,
            FullHashSatisfiedBytes = "2000",
            HashPipelineResolvedFiles = 4,
            HashPipelineResolvedBytes = "4000",
            ConfirmedLogicalBytes = "2000",
        };
        return new WorkerRunProgressEventArgs
        {
            RunId = runId,
            Sequence = sequence,
            Status = status,
            Phase = "hashing",
            FilesDiscovered = 10,
            BytesDiscovered = "10000",
            FilesHashed = 4,
            WarningCount = 0,
            CurrentPath = @"C:\Data\candidate.bin",
            Progress = new WorkerScanProgressSnapshot
            {
                ProgressContractVersion = WorkerProgressContract.ProgressContractVersion,
                MetricsContractVersion = WorkerProgressContract.MetricsContractVersion,
                Revision = revision,
                MonotonicNanos = 12_000_000_000,
                Phase = "candidate_screening",
                PhaseElapsedNanos = 10_000_000_000,
                Counters = counters,
                Logical = logical,
                Funnel = Funnel(counters, logical),
                PartialReadRates = AvailableRates(4_000, "400", 10_000_000_000),
                FullReadRates = AvailableRates(2_000, "100", 10_000_000_000),
                CacheHitRateBasisPoints = 5_000,
                WarningCount = 0,
                ActiveDevices = new WorkerActiveDeviceProgress
                {
                    State = "unavailable",
                    Reason = "mapping_unavailable",
                },
                RemainingKnownWork = new WorkerRemainingKnownWork
                {
                    Stage = "hash_pipeline",
                    Files = 4,
                    LogicalBytes = "4000",
                },
                Eta = new WorkerProgressEta
                {
                    State = "available",
                    Stage = "hash_pipeline",
                    RemainingLogicalBytes = "4000",
                    LogicalBytesPerSecondMillis = "1000000",
                    EstimatedSeconds = 4,
                    WindowNanos = 10_000_000_000,
                },
            },
        };
    }

    private static WorkerCandidateFunnelProgress Funnel(
        WorkerScanProgressCounters counters,
        WorkerProgressLogicalCounters logical) => new()
        {
            Discovered = Quantity(counters.DiscoveredFiles, counters.DiscoveredBytes),
            MetadataResolved = Quantity(counters.MetadataResolvedFiles, counters.MetadataResolvedBytes),
            HashPipelineCandidates = Quantity(counters.CandidateFiles, counters.CandidateBytes),
            PartialScreened = Quantity(logical.PartialScreenedFiles, logical.PartialScreenedBytes),
            SelectedForFullHash = Quantity(counters.FullHashRequests, logical.FullHashRequestBytes),
            FullHashSatisfied = Quantity(logical.FullHashSatisfiedFiles, logical.FullHashSatisfiedBytes),
            FinalizedDuplicates = Quantity(counters.ConfirmedLogicalCopies, logical.ConfirmedLogicalBytes),
        };

    private static WorkerProgressQuantity Quantity(ulong files, string bytes) => new()
    {
        Files = files,
        LogicalBytes = bytes,
    };

    private static WorkerProgressRates UnavailableRates() => new()
    {
        Cumulative = UnavailableRate(),
        Recent = UnavailableRate(),
    };

    private static WorkerProgressRates AvailableRates(
        ulong filesPerSecondMillis,
        string physicalBytesPerSecond,
        ulong windowNanos) => new()
        {
            Cumulative = AvailableRate(filesPerSecondMillis, physicalBytesPerSecond, windowNanos),
            Recent = AvailableRate(filesPerSecondMillis, physicalBytesPerSecond, windowNanos),
        };

    private static WorkerProgressRateValue UnavailableRate() => new()
    {
        State = "unavailable",
        Reason = "no_elapsed_time",
    };

    private static WorkerProgressRateValue AvailableRate(
        ulong filesPerSecondMillis,
        string physicalBytesPerSecond,
        ulong windowNanos) => new()
        {
            State = "available",
            Rate = new WorkerProgressRate
            {
                FilesPerSecondMillis = filesPerSecondMillis,
                PhysicalBytesPerSecond = physicalBytesPerSecond,
                WindowNanos = windowNanos,
            },
        };

    private static WorkerScanProgressCounters EmptyCounters() => new();

    private static WorkerProgressLogicalCounters EmptyLogical() => new();
}
