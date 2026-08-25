using System.Text.Json;
using System.Text.Json.Serialization;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Infrastructure.Protocol;

namespace SuperDuper.Windows.Infrastructure;

internal static class WorkerRunProgressParser
{
    private static readonly JsonSerializerOptions StrictSerializerOptions = new(
        JsonLineProtocol.SerializerOptions)
    {
        NumberHandling = JsonNumberHandling.Strict,
        PropertyNameCaseInsensitive = false,
        RespectNullableAnnotations = true,
        RespectRequiredConstructorParameters = true,
    };

    private static readonly string[] CounterStrings =
    [
        "discoveredBytes",
        "hardLinkAliasBytes",
        "singletonSizeBytes",
        "candidateBytes",
        "duplicateCandidateBytes",
        "metadataResolvedBytes",
        "partialHashBytesRead",
        "partialCollisionBytes",
        "fullHashBytesRead",
        "recoverableBytes",
    ];

    private static readonly string[] CounterNumbers =
    [
        "discoveredFiles",
        "zeroByteFiles",
        "hardLinkAliasFiles",
        "sizeBuckets",
        "singletonSizeBuckets",
        "singletonSizeFiles",
        "candidateSizeBuckets",
        "candidateFiles",
        "duplicateCandidateSizeBuckets",
        "duplicateCandidateFiles",
        "metadataResolvedFiles",
        "partialHashesAttempted",
        "partialHashesSucceeded",
        "partialHashesFailed",
        "partialCollisionBuckets",
        "partialCollisionFiles",
        "fullHashRequests",
        "fullHashCacheHits",
        "fullHashCacheMisses",
        "fullHashCacheErrors",
        "fullHashCacheStores",
        "fullHashContentReadsStarted",
        "fullHashContentReadsCompleted",
        "fullHashContentReadsFailed",
        "confirmedDuplicateGroups",
        "confirmedLogicalCopies",
        "confirmedPhysicalItems",
        "warnings",
        "cancelChecks",
        "cancelledWorkItems",
        "telemetrySamplesLost",
        "telemetryFlushErrors",
        "unavailableCounters",
    ];

    private static readonly string[] LogicalStrings =
    [
        "partialScreenedBytes",
        "fullHashRequestBytes",
        "fullHashSatisfiedBytes",
        "fullHashFailedBytes",
        "hashPipelineResolvedBytes",
        "confirmedLogicalBytes",
    ];

    private static readonly string[] LogicalNumbers =
    [
        "partialScreenedFiles",
        "fullHashSatisfiedFiles",
        "fullHashFailedFiles",
        "hashPipelineResolvedFiles",
    ];

    private static readonly string[] FunnelStages =
    [
        "discovered",
        "metadataResolved",
        "hashPipelineCandidates",
        "partialScreened",
        "selectedForFullHash",
        "fullHashSatisfied",
        "finalizedDuplicates",
    ];

    internal static WorkerRunProgressEventArgs Parse(JsonElement data)
    {
        try
        {
            ValidateShape(data);
            var progress = data.Deserialize<WorkerRunProgressEventArgs>(
                StrictSerializerOptions)
                ?? throw new WorkerProtocolException("run.progress event data is invalid.");
            if (!WorkerProgressContract.TryValidate(progress, out var error))
            {
                throw new WorkerProtocolException($"run.progress contract is invalid: {error}.");
            }
            return progress;
        }
        catch (WorkerProtocolException)
        {
            throw;
        }
        catch (Exception exception) when (exception is JsonException
            or KeyNotFoundException
            or InvalidOperationException
            or FormatException
            or OverflowException)
        {
            throw new WorkerProtocolException("run.progress event data is invalid.", exception);
        }
    }

    private static void ValidateShape(JsonElement data)
    {
        RequireKind(data, JsonValueKind.Object, "data");
        RequireNumber(data, "runId", "sequence", "filesDiscovered", "filesHashed", "warningCount");
        RequireString(data, "status", "phase", "bytesDiscovered");
        OptionalKind(data, "currentPath", JsonValueKind.String);
        OptionalKind(data, "message", JsonValueKind.String);

        var snapshot = RequireObject(data, "progress");
        RequireNumber(
            snapshot,
            "progressContractVersion",
            "metricsContractVersion",
            "revision",
            "monotonicNanos",
            "phaseElapsedNanos",
            "warningCount");
        RequireString(snapshot, "phase");
        if (!snapshot.TryGetProperty("cacheHitRateBasisPoints", out var cacheHitRate)
            || cacheHitRate.ValueKind is not (JsonValueKind.Number or JsonValueKind.Null))
        {
            throw new WorkerProtocolException(
                "progress.cacheHitRateBasisPoints must be a JSON Number or Null.");
        }

        var counters = RequireObject(snapshot, "counters");
        RequireNumber(counters, CounterNumbers);
        RequireString(counters, CounterStrings);

        var logical = RequireObject(snapshot, "logical");
        RequireNumber(logical, LogicalNumbers);
        RequireString(logical, LogicalStrings);

        var funnel = RequireObject(snapshot, "funnel");
        foreach (var stageName in FunnelStages)
        {
            var stage = RequireObject(funnel, stageName);
            RequireNumber(stage, "files");
            RequireString(stage, "logicalBytes");
        }

        ValidateRates(RequireObject(snapshot, "partialReadRates"), "partialReadRates");
        ValidateRates(RequireObject(snapshot, "fullReadRates"), "fullReadRates");
        ValidateDevices(RequireObject(snapshot, "activeDevices"));

        if (snapshot.TryGetProperty("remainingKnownWork", out var remaining)
            && remaining.ValueKind != JsonValueKind.Null)
        {
            RequireKind(remaining, JsonValueKind.Object, "remainingKnownWork");
            RequireString(remaining, "stage", "logicalBytes");
            RequireNumber(remaining, "files");
        }
        else if (!snapshot.TryGetProperty("remainingKnownWork", out _))
        {
            throw new WorkerProtocolException("progress.remainingKnownWork is missing.");
        }
        ValidateEta(RequireObject(snapshot, "eta"));
    }

    private static void ValidateRates(JsonElement rates, string name)
    {
        ValidateRateValue(RequireObject(rates, "cumulative"), $"{name}.cumulative");
        ValidateRateValue(RequireObject(rates, "recent"), $"{name}.recent");
    }

    private static void ValidateRateValue(JsonElement value, string name)
    {
        RequireString(value, "state");
        switch (value.GetProperty("state").GetString())
        {
            case "available":
                var rate = RequireObject(value, "rate");
                RequireNumber(rate, "filesPerSecondMillis", "windowNanos");
                RequireString(rate, "physicalBytesPerSecond");
                break;
            case "unavailable":
                RequireString(value, "reason");
                break;
            default:
                throw new WorkerProtocolException($"{name}.state is unsupported.");
        }
    }

    private static void ValidateDevices(JsonElement devices)
    {
        RequireString(devices, "state");
        switch (devices.GetProperty("state").GetString())
        {
            case "unavailable":
                RequireString(devices, "reason");
                break;
            case "one":
                RequireString(devices, "device_key");
                break;
            case "multiple":
                var keys = devices.GetProperty("device_keys");
                RequireKind(keys, JsonValueKind.Array, "activeDevices.device_keys");
                foreach (var key in keys.EnumerateArray())
                {
                    RequireKind(key, JsonValueKind.String, "activeDevices.device_keys[]");
                }
                break;
            default:
                throw new WorkerProtocolException("activeDevices.state is unsupported.");
        }
    }

    private static void ValidateEta(JsonElement eta)
    {
        RequireString(eta, "state");
        switch (eta.GetProperty("state").GetString())
        {
            case "available":
                RequireString(
                    eta,
                    "stage",
                    "remaining_logical_bytes",
                    "logical_bytes_per_second_millis");
                RequireNumber(eta, "estimated_seconds", "window_nanos");
                break;
            case "unavailable":
                RequireString(eta, "reason");
                break;
            case "complete":
                break;
            default:
                throw new WorkerProtocolException("eta.state is unsupported.");
        }
    }

    private static JsonElement RequireObject(JsonElement value, string propertyName)
    {
        var property = value.GetProperty(propertyName);
        RequireKind(property, JsonValueKind.Object, propertyName);
        return property;
    }

    private static void RequireNumber(JsonElement value, params string[] propertyNames) =>
        RequireProperties(value, JsonValueKind.Number, propertyNames);

    private static void RequireString(JsonElement value, params string[] propertyNames) =>
        RequireProperties(value, JsonValueKind.String, propertyNames);

    private static void RequireProperties(
        JsonElement value,
        JsonValueKind kind,
        IEnumerable<string> propertyNames)
    {
        foreach (var propertyName in propertyNames)
        {
            RequireKind(value.GetProperty(propertyName), kind, propertyName);
        }
    }

    private static void OptionalKind(
        JsonElement value,
        string propertyName,
        JsonValueKind kind,
        bool allowNull = false)
    {
        if (!value.TryGetProperty(propertyName, out var property))
        {
            return;
        }
        if (allowNull && property.ValueKind == JsonValueKind.Null)
        {
            return;
        }
        RequireKind(property, kind, propertyName);
    }

    private static void RequireKind(JsonElement value, JsonValueKind kind, string name)
    {
        if (value.ValueKind != kind)
        {
            throw new WorkerProtocolException($"{name} must be a JSON {kind}.");
        }
    }
}
