using System.Text.Json;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Infrastructure;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WorkerRunProgressParserTests
{
    [TestMethod]
    public void CompleteTypedPayloadParsesExactVersionsStringsAndUnavailableStates()
    {
        var parsed = Parse(ValidData);

        Assert.AreEqual(1U, parsed.Progress.ProgressContractVersion);
        Assert.AreEqual(2U, parsed.Progress.MetricsContractVersion);
        Assert.AreEqual("100", parsed.Progress.Counters.DiscoveredBytes);
        Assert.AreEqual("mapping_unavailable", parsed.Progress.ActiveDevices.Reason);
        Assert.IsNull(parsed.Progress.RemainingKnownWork);
        Assert.AreEqual("work_not_yet_known", parsed.Progress.Eta.Reason);
    }

    [TestMethod]
    public void JsonKindsRequiredNullableMembersVersionsAndDecimalRangeFailClosed()
    {
        string[] invalid =
        [
            ValidData.Replace("\"sequence\":1", "\"sequence\":\"1\""),
            ValidData.Replace("\"bytesDiscovered\":\"100\"", "\"bytesDiscovered\":100"),
            ValidData.Replace("\"discoveredBytes\":\"100\"", "\"discoveredBytes\":100"),
            ValidData.Replace("\"cacheHitRateBasisPoints\":null,", string.Empty),
            ValidData.Replace("\"remainingKnownWork\":null,", string.Empty),
            ValidData.Replace("\"progressContractVersion\":1", "\"ProgressContractVersion\":1"),
            ValidData.Replace("\"progressContractVersion\":1", "\"progressContractVersion\":9"),
            ValidData.Replace("\"metricsContractVersion\":2", "\"metricsContractVersion\":9"),
            ValidData.Replace("\"100\"", "\"18446744073709551616\""),
            ValidData.Replace("\"100\"", "\"0100\""),
            ValidData.Replace("\"mapping_unavailable\"", "\"invented_reason\""),
            ValidData.Replace("\"work_not_yet_known\"", "\"invented_reason\""),
        ];

        for (var index = 0; index < invalid.Length; index++)
        {
            Assert.ThrowsException<WorkerProtocolException>(
                () => Parse(invalid[index]),
                $"invalid contract mutation {index} was accepted");
        }
    }

    [TestMethod]
    public void ExactMaximumDecimalUnknownMembersAndWrongCaseExtrasRemainAdditiveCompatible()
    {
        var maximum = ValidData.Replace("\"100\"", $"\"{ulong.MaxValue}\"");
        var additive = maximum.Replace(
            "\"progressContractVersion\":1",
            "\"ProgressContractVersion\":99,\"futureField\":{\"value\":true},\"progressContractVersion\":1");

        var parsed = Parse(additive);

        Assert.AreEqual(1U, parsed.Progress.ProgressContractVersion);
        Assert.AreEqual(ulong.MaxValue.ToString(), parsed.BytesDiscovered);
    }

    [TestMethod]
    public void SnakeCaseAvailableEtaAndMultipleDevicePayloadsBindExactly()
    {
        var json = ValidData
            .Replace(
                "\"activeDevices\":{\"state\":\"unavailable\",\"reason\":\"mapping_unavailable\"}",
                "\"activeDevices\":{\"state\":\"multiple\",\"device_keys\":[\"physical:1\",\"physical:2\"]}")
            .Replace(
                "\"eta\":{\"state\":\"unavailable\",\"reason\":\"work_not_yet_known\"}",
                "\"eta\":{\"state\":\"available\",\"stage\":\"hash_pipeline\",\"remaining_logical_bytes\":\"100\",\"logical_bytes_per_second_millis\":\"1000\",\"estimated_seconds\":1,\"window_nanos\":10000000000}");

        var parsed = Parse(json);

        CollectionAssert.AreEqual(
            new[] { "physical:1", "physical:2" },
            parsed.Progress.ActiveDevices.DeviceKeys!.ToArray());
        Assert.AreEqual("100", parsed.Progress.Eta.RemainingLogicalBytes);
        Assert.AreEqual("1000", parsed.Progress.Eta.LogicalBytesPerSecondMillis);
        Assert.AreEqual(1UL, parsed.Progress.Eta.EstimatedSeconds);
    }

    [TestMethod]
    public void FunnelWarningLegacyAndTaggedVariantMismatchesFailClosed()
    {
        string[] invalid =
        [
            ValidData.Replace(
                "\"discovered\":{\"files\":1,\"logicalBytes\":\"100\"}",
                "\"discovered\":{\"files\":2,\"logicalBytes\":\"100\"}"),
            ValidData.Replace(
                "\"cacheHitRateBasisPoints\":null,\"warningCount\":0",
                "\"cacheHitRateBasisPoints\":null,\"warningCount\":1"),
            ValidData.Replace("\"filesDiscovered\":1", "\"filesDiscovered\":2"),
            ValidData.Replace(
                "\"activeDevices\":{\"state\":\"unavailable\",\"reason\":\"mapping_unavailable\"}",
                "\"activeDevices\":{\"state\":\"multiple\",\"device_keys\":[\"same\",\"same\"]}"),
            ValidData.Replace(
                "\"eta\":{\"state\":\"unavailable\",\"reason\":\"work_not_yet_known\"}",
                "\"eta\":{\"state\":\"complete\",\"reason\":\"work_not_yet_known\"}"),
        ];

        for (var index = 0; index < invalid.Length; index++)
        {
            Assert.ThrowsException<WorkerProtocolException>(
                () => Parse(invalid[index]),
                $"invalid semantic mutation {index} was accepted");
        }
    }

    private static WorkerRunProgressEventArgs Parse(string json)
    {
        using var document = JsonDocument.Parse(json);
        return WorkerRunProgressParser.Parse(document.RootElement);
    }

    private const string ValidData = """
        {
          "runId":19,"sequence":1,"status":"running","phase":"discovering",
          "filesDiscovered":1,"bytesDiscovered":"100","filesHashed":0,"warningCount":0,
          "progress":{
            "progressContractVersion":1,"metricsContractVersion":2,"revision":1,
            "monotonicNanos":1000000000,"phase":"discovering","phaseElapsedNanos":1000000000,
            "counters":{
              "discoveredFiles":1,"discoveredBytes":"100","zeroByteFiles":0,
              "hardLinkAliasFiles":0,"hardLinkAliasBytes":"0","sizeBuckets":0,
              "singletonSizeBuckets":0,"singletonSizeFiles":0,"singletonSizeBytes":"0",
              "candidateSizeBuckets":0,"candidateFiles":0,"candidateBytes":"0",
              "duplicateCandidateSizeBuckets":0,"duplicateCandidateFiles":0,
              "duplicateCandidateBytes":"0","metadataResolvedFiles":0,
              "metadataResolvedBytes":"0","partialHashesAttempted":0,
              "partialHashesSucceeded":0,"partialHashesFailed":0,"partialHashBytesRead":"0",
              "partialCollisionBuckets":0,"partialCollisionFiles":0,"partialCollisionBytes":"0",
              "fullHashRequests":0,"fullHashCacheHits":0,"fullHashCacheMisses":0,
              "fullHashCacheErrors":0,"fullHashCacheStores":0,"fullHashContentReadsStarted":0,
              "fullHashContentReadsCompleted":0,"fullHashContentReadsFailed":0,
              "fullHashBytesRead":"0","confirmedDuplicateGroups":0,"confirmedLogicalCopies":0,
              "confirmedPhysicalItems":0,"recoverableBytes":"0","warnings":0,"cancelChecks":0,
              "cancelledWorkItems":0,"telemetrySamplesLost":0,"telemetryFlushErrors":0,
              "unavailableCounters":0
            },
            "logical":{
              "partialScreenedFiles":0,"partialScreenedBytes":"0","fullHashRequestBytes":"0",
              "fullHashSatisfiedFiles":0,"fullHashSatisfiedBytes":"0","fullHashFailedFiles":0,
              "fullHashFailedBytes":"0","hashPipelineResolvedFiles":0,
              "hashPipelineResolvedBytes":"0","confirmedLogicalBytes":"0"
            },
            "funnel":{
              "discovered":{"files":1,"logicalBytes":"100"},
              "metadataResolved":{"files":0,"logicalBytes":"0"},
              "hashPipelineCandidates":{"files":0,"logicalBytes":"0"},
              "partialScreened":{"files":0,"logicalBytes":"0"},
              "selectedForFullHash":{"files":0,"logicalBytes":"0"},
              "fullHashSatisfied":{"files":0,"logicalBytes":"0"},
              "finalizedDuplicates":{"files":0,"logicalBytes":"0"}
            },
            "partialReadRates":{
              "cumulative":{"state":"unavailable","reason":"no_elapsed_time"},
              "recent":{"state":"unavailable","reason":"no_elapsed_time"}
            },
            "fullReadRates":{
              "cumulative":{"state":"unavailable","reason":"no_elapsed_time"},
              "recent":{"state":"unavailable","reason":"no_elapsed_time"}
            },
            "cacheHitRateBasisPoints":null,"warningCount":0,
            "activeDevices":{"state":"unavailable","reason":"mapping_unavailable"},
            "remainingKnownWork":null,
            "eta":{"state":"unavailable","reason":"work_not_yet_known"}
          }
        }
        """;
}
